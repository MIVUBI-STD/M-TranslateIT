use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::engine::audio::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, clear_finalized_outbound_utterance_producer,
    try_take_finalized_incoming_utterance, wait_take_finalized_incoming_utterance,
    wait_take_finalized_outbound_utterance,
};
use crate::engine::audio::live_segment_writer::{
    remove_finalized_meeting_utterance_wav, write_finalized_incoming_utterance_wav,
    write_finalized_outbound_utterance_wav,
};
use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::incoming_deferred::clear_deferred_incoming_queue;
use super::incoming_pipeline::{
    drain_due_deferred_incoming, process_authoritative_finalized_incoming_wav,
    IncomingAudioProcessResult,
};
use super::session_state::{
    elapsed_millis, set_outbound_timing, timing_context_from_utterance, update_incoming_status,
    update_outbound_status,
};
use super::{
    generation_is_live, incoming_session_is_eligible,
    process_authoritative_finalized_outbound_wav,
};

struct MeetingOutboundConsumerRuntime {
    generation: u64,
    session_id: String,
    thread: Option<JoinHandle<()>>,
}

struct MeetingIncomingConsumerRuntime {
    session_id: String,
    thread: Option<JoinHandle<()>>,
}

pub(super) struct MeetingConsumerCleanupResult {
    pub(super) ok: bool,
    pub(super) message: String,
}

static MEETING_OUTBOUND_CONSUMER: OnceLock<Mutex<Option<MeetingOutboundConsumerRuntime>>> =
    OnceLock::new();
static MEETING_INCOMING_CONSUMER: OnceLock<Mutex<Option<MeetingIncomingConsumerRuntime>>> =
    OnceLock::new();

fn outbound_consumer_store() -> &'static Mutex<Option<MeetingOutboundConsumerRuntime>> {
    MEETING_OUTBOUND_CONSUMER.get_or_init(|| Mutex::new(None))
}

fn incoming_consumer_store() -> &'static Mutex<Option<MeetingIncomingConsumerRuntime>> {
    MEETING_INCOMING_CONSUMER.get_or_init(|| Mutex::new(None))
}

pub(super) fn start_meeting_outbound_consumer(generation: u64, session_id: &str) -> Result<(), String> {
    let store = outbound_consumer_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_outbound:consumer_state_lock_failed".to_string())?;
    if guard.is_some() {
        return Err("meeting_outbound:consumer_already_active".to_string());
    }

    let thread_session_id = session_id.to_string();
    let thread_session_for_runtime = thread_session_id.clone();
    let handle = thread::Builder::new()
        .name("translateit-meeting-outbound".to_string())
        .spawn(move || {
            while let Some(utterance) = wait_take_finalized_outbound_utterance(generation) {
                let queue_ms = elapsed_millis(utterance.enqueued_at, Instant::now());
                if utterance.generation != Some(generation)
                    || utterance.lane != "you"
                    || utterance.session_id != thread_session_id
                    || !generation_is_live(generation)
                {
                    continue;
                }

                let audio_prepare_started_at = Instant::now();
                let write = write_finalized_outbound_utterance_wav(&utterance);
                let audio_prepare_ms = elapsed_millis(audio_prepare_started_at, Instant::now());
                let timing = timing_context_from_utterance(&utterance, queue_ms, audio_prepare_ms);
                if !write.ok {
                    update_outbound_status(
                        generation,
                        &thread_session_id,
                        "attention_needed",
                        utterance.sequence,
                        false,
                        false,
                        &write.blocker,
                        "Finalized speech could not be written to its temporary ASR WAV. No AI/output stage consumed it.",
                    );
                    set_outbound_timing(
                        generation,
                        &thread_session_id,
                        utterance.sequence,
                        &timing.metrics,
                    );
                    continue;
                }

                let Some(audio_path) = write.audio_path else {
                    update_outbound_status(
                        generation,
                        &thread_session_id,
                        "attention_needed",
                        utterance.sequence,
                        false,
                        false,
                        "meeting_outbound:finalized_audio_path_missing",
                        "Finalized speech writer returned no temporary audio path. No AI/output stage consumed it.",
                    );
                    set_outbound_timing(
                        generation,
                        &thread_session_id,
                        utterance.sequence,
                        &timing.metrics,
                    );
                    continue;
                };

                if !generation_is_live(generation) {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                    break;
                }

                let _ = process_authoritative_finalized_outbound_wav(
                    generation,
                    &utterance.session_id,
                    utterance.sequence,
                    utterance.utterance_id,
                    audio_path.clone(),
                    timing,
                );
                remove_finalized_meeting_utterance_wav(&audio_path);

                if !runtime_generation_is_authoritative(generation) {
                    break;
                }
            }
        })
        .map_err(|error| format!("meeting_outbound:consumer_spawn_failed:{error}"))?;

    *guard = Some(MeetingOutboundConsumerRuntime {
        generation,
        session_id: thread_session_for_runtime,
        thread: Some(handle),
    });
    Ok(())
}

pub(super) fn stop_meeting_outbound_consumer(generation: u64) -> MeetingConsumerCleanupResult {
    clear_finalized_outbound_utterance_producer();

    let store = outbound_consumer_store();
    let runtime = match store.lock() {
        Ok(mut guard) => {
            if guard
                .as_ref()
                .map(|value| value.generation == generation)
                .unwrap_or(false)
            {
                guard.take()
            } else {
                None
            }
        }
        Err(_) => {
            return MeetingConsumerCleanupResult {
                ok: false,
                message: "Meeting outbound consumer state lock failed during cleanup.".to_string(),
            };
        }
    };

    let Some(mut runtime) = runtime else {
        return MeetingConsumerCleanupResult {
            ok: true,
            message: "No matching Meeting outbound consumer required cleanup.".to_string(),
        };
    };
    let session_id = runtime.session_id.clone();
    let joined = runtime
        .thread
        .take()
        .map(|handle| handle.join().is_ok())
        .unwrap_or(true);
    MeetingConsumerCleanupResult {
        ok: joined,
        message: if joined {
            format!("Meeting outbound consumer stopped for {session_id} generation {generation}.")
        } else {
            format!("Meeting outbound consumer for {session_id} generation {generation} exited unexpectedly during cleanup.")
        },
    }
}

pub(super) fn start_meeting_incoming_consumer(session_id: &str) -> Result<(), String> {
    let store = incoming_consumer_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_incoming:consumer_state_lock_failed".to_string())?;
    if guard.is_some() {
        return Err("meeting_incoming:consumer_already_active".to_string());
    }

    let thread_session_id = session_id.to_string();
    let runtime_session_id = thread_session_id.clone();
    let handle = thread::Builder::new()
        .name("translateit-meeting-incoming".to_string())
        .spawn(move || {
            loop {
                drain_due_deferred_incoming(&thread_session_id);

                let utterance = match try_take_finalized_incoming_utterance(&thread_session_id) {
                    Some(utterance) => utterance,
                    None => {
                        let Some(utterance) =
                            wait_take_finalized_incoming_utterance(&thread_session_id)
                        else {
                            break;
                        };
                        utterance
                    }
                };

                if utterance.session_id != thread_session_id
                    || utterance.lane != "incoming"
                    || utterance.generation.is_some()
                    || !incoming_session_is_eligible(&thread_session_id)
                {
                    continue;
                }

                let write = write_finalized_incoming_utterance_wav(&utterance);
                if !write.ok {
                    update_incoming_status(
                        &thread_session_id,
                        "degraded",
                        true,
                        &write.blocker,
                        "Finalized incoming speech could not be written to its temporary ASR WAV. Outbound remains available.",
                    );
                    continue;
                }
                let Some(audio_path) = write.audio_path else {
                    update_incoming_status(
                        &thread_session_id,
                        "degraded",
                        true,
                        "meeting_incoming:finalized_audio_path_missing",
                        "Finalized incoming speech writer returned no temporary audio path.",
                    );
                    continue;
                };

                if !incoming_session_is_eligible(&thread_session_id) {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                    break;
                }
                let result = process_authoritative_finalized_incoming_wav(
                    &thread_session_id,
                    utterance.sequence,
                    utterance.utterance_id,
                    &audio_path,
                    None,
                );
                if result != IncomingAudioProcessResult::DeferredAsr {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                }
            }
        })
        .map_err(|error| format!("meeting_incoming:consumer_spawn_failed:{error}"))?;

    *guard = Some(MeetingIncomingConsumerRuntime {
        session_id: runtime_session_id,
        thread: Some(handle),
    });
    Ok(())
}

pub(super) fn stop_meeting_incoming_consumer(session_id: &str) -> MeetingConsumerCleanupResult {
    clear_finalized_incoming_utterance_producer();
    clear_deferred_incoming_queue();
    let store = incoming_consumer_store();
    let runtime = match store.lock() {
        Ok(mut guard) => {
            if guard
                .as_ref()
                .map(|value| value.session_id == session_id)
                .unwrap_or(false)
            {
                guard.take()
            } else {
                None
            }
        }
        Err(_) => {
            return MeetingConsumerCleanupResult {
                ok: false,
                message: "Meeting incoming consumer state lock failed during cleanup.".to_string(),
            };
        }
    };

    let Some(mut runtime) = runtime else {
        return MeetingConsumerCleanupResult {
            ok: true,
            message: "No matching Meeting incoming consumer required cleanup.".to_string(),
        };
    };
    let joined = runtime
        .thread
        .take()
        .map(|handle| handle.join().is_ok())
        .unwrap_or(true);
    MeetingConsumerCleanupResult {
        ok: joined,
        message: if joined {
            format!("Meeting incoming consumer stopped for session {session_id}.")
        } else {
            format!("Meeting incoming consumer for session {session_id} exited unexpectedly during cleanup.")
        },
    }
}

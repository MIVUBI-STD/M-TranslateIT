use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::audio::meeting_output::deliver_meeting_output_wav;
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;

use super::super::virtual_mic_route::get_bound_virtual_mic_output_device;
use super::committed_turns::{
    update_committed_turn_delivery_state, update_committed_turn_outbound_timing,
};
use super::outbound_pipeline::remove_temporary_tts_paths;
use super::session_state::{
    elapsed_millis, record_first_playback_timing, set_outbound_playback_activity,
    set_outbound_timing, update_incoming_status, update_outbound_status, OutboundTimingContext,
};
use super::suppression::{
    begin_self_output_suppression, disable_optional_incoming_for_outbound,
};
use super::{generation_is_live, incoming_session_is_eligible};

const PLAYBACK_QUEUE_CAPACITY: usize = 1;
const ENQUEUE_RETRY_DELAY: Duration = Duration::from_millis(10);

pub(super) struct PreparedPlaybackJob {
    pub(super) generation: u64,
    pub(super) session_id: String,
    pub(super) event_sequence: u64,
    pub(super) utterance_id: u64,
    pub(super) requested_tts_path: String,
    pub(super) actual_tts_path: String,
    pub(super) timing: OutboundTimingContext,
    pub(super) prepared_at: Instant,
}

struct MeetingPlaybackRuntime {
    generation: u64,
    session_id: String,
    sender: SyncSender<PreparedPlaybackJob>,
    thread: Option<JoinHandle<()>>,
}

pub(super) struct MeetingPlaybackCleanupResult {
    pub(super) ok: bool,
    pub(super) message: String,
}

static MEETING_PLAYBACK_RUNTIME: OnceLock<Mutex<Option<MeetingPlaybackRuntime>>> = OnceLock::new();

fn playback_runtime_store() -> &'static Mutex<Option<MeetingPlaybackRuntime>> {
    MEETING_PLAYBACK_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn cleanup_job(job: &PreparedPlaybackJob) {
    remove_temporary_tts_paths(&job.requested_tts_path, &job.actual_tts_path);
}

fn mark_interrupted(job: &PreparedPlaybackJob) {
    let _ = update_committed_turn_delivery_state(
        &job.session_id,
        job.generation,
        job.utterance_id,
        "interrupted",
    );
}

fn sequence_is_monotonic(last_sequence: u64, candidate: u64) -> bool {
    candidate > last_sequence
}

fn process_playback_job(job: PreparedPlaybackJob, last_sequence: &mut u64) {
    if !sequence_is_monotonic(*last_sequence, job.event_sequence) {
        cleanup_job(&job);
        let _ = update_committed_turn_delivery_state(
            &job.session_id,
            job.generation,
            job.utterance_id,
            "output_failed",
        );
        update_outbound_status(
            job.generation,
            &job.session_id,
            "attention_needed",
            job.event_sequence,
            false,
            false,
            "meeting_playback:sequence_not_monotonic",
            "Prepared Meeting playback was rejected because output ordering was no longer monotonic.",
        );
        return;
    }
    *last_sequence = job.event_sequence;

    if !generation_is_live(job.generation) {
        cleanup_job(&job);
        mark_interrupted(&job);
        return;
    }

    let mut timing = OutboundTimingContext {
        finalized_at: job.timing.finalized_at,
        metrics: job.timing.metrics.clone(),
    };
    timing.metrics.playback_queue_ms = Some(elapsed_millis(job.prepared_at, Instant::now()));
    set_outbound_timing(
        job.generation,
        &job.session_id,
        job.event_sequence,
        &timing.metrics,
    );
    let _ = update_committed_turn_outbound_timing(
        &job.session_id,
        job.generation,
        job.utterance_id,
        &timing.metrics,
    );

    let bound_output_device = match get_bound_virtual_mic_output_device(job.generation) {
        Ok(device) => device,
        Err(blocker) => {
            cleanup_job(&job);
            let _ = update_committed_turn_delivery_state(
                &job.session_id,
                job.generation,
                job.utterance_id,
                "output_failed",
            );
            update_outbound_status(
                job.generation,
                &job.session_id,
                "attention_needed",
                job.event_sequence,
                false,
                false,
                &blocker,
                "Prepared voice output could not resolve the authoritative Meeting microphone route.",
            );
            return;
        }
    };

    if !generation_is_live(job.generation) {
        cleanup_job(&job);
        mark_interrupted(&job);
        return;
    }

    let suppression_guard = match begin_self_output_suppression(&job.session_id) {
        Some(guard) => Some(guard),
        None => {
            let incoming_cleanup = disable_optional_incoming_for_outbound(&job.session_id);
            update_outbound_status(
                job.generation,
                &job.session_id,
                "queued",
                job.event_sequence,
                false,
                true,
                "",
                &format!(
                    "Optional incoming protection became unavailable and incoming was disabled before required outbound delivery. {incoming_cleanup}"
                ),
            );
            None
        }
    };

    let _ = update_committed_turn_delivery_state(
        &job.session_id,
        job.generation,
        job.utterance_id,
        "speaking",
    );
    set_outbound_playback_activity(
        job.generation,
        &job.session_id,
        job.event_sequence,
        true,
    );

    let delivery_started_at = Instant::now();
    let route = deliver_meeting_output_wav(
        &job.actual_tts_path,
        Some(bound_output_device.as_str()),
        job.generation,
    );
    record_first_playback_timing(
        &mut timing,
        delivery_started_at,
        route.first_playback_at,
        route.first_playback_unix_ms,
    );
    set_outbound_timing(
        job.generation,
        &job.session_id,
        job.event_sequence,
        &timing.metrics,
    );
    let _ = update_committed_turn_outbound_timing(
        &job.session_id,
        job.generation,
        job.utterance_id,
        &timing.metrics,
    );

    set_outbound_playback_activity(
        job.generation,
        &job.session_id,
        job.event_sequence,
        false,
    );
    drop(suppression_guard);

    if incoming_session_is_eligible(&job.session_id)
        && meeting_sound_capture_status().stream_active
    {
        update_incoming_status(
            &job.session_id,
            "listening",
            false,
            "",
            "Incoming Meeting Sound resumed from a fresh speech boundary after TranslateIT TTS playback ended.",
        );
    }

    cleanup_job(&job);

    if !generation_is_live(job.generation) {
        mark_interrupted(&job);
        return;
    }

    if !route.ok || !route.execution_attempted {
        let blocker = if route.blocker.is_empty() {
            "meeting_outbound:meeting_route_delivery_failed".to_string()
        } else {
            route.blocker
        };
        let _ = update_committed_turn_delivery_state(
            &job.session_id,
            job.generation,
            job.utterance_id,
            "output_failed",
        );
        update_outbound_status(
            job.generation,
            &job.session_id,
            "attention_needed",
            job.event_sequence,
            false,
            false,
            &blocker,
            "Translated voice could not be safely delivered to the Meeting microphone route. Playback was not retried.",
        );
        return;
    }

    let _ = update_committed_turn_delivery_state(
        &job.session_id,
        job.generation,
        job.utterance_id,
        "output_complete",
    );
}

pub(super) fn start_meeting_playback_runtime(
    generation: u64,
    session_id: &str,
) -> Result<(), String> {
    let store = playback_runtime_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_playback:runtime_state_lock_failed".to_string())?;
    if guard.is_some() {
        return Err("meeting_playback:runtime_already_active".to_string());
    }

    let (sender, receiver) =
        mpsc::sync_channel::<PreparedPlaybackJob>(PLAYBACK_QUEUE_CAPACITY);
    let thread_session_id = session_id.to_string();
    let runtime_session_id = thread_session_id.clone();
    let handle = thread::Builder::new()
        .name("translateit-meeting-playback".to_string())
        .spawn(move || {
            let mut last_sequence = 0u64;
            while let Ok(job) = receiver.recv() {
                if job.generation != generation || job.session_id != thread_session_id {
                    cleanup_job(&job);
                    mark_interrupted(&job);
                    continue;
                }
                process_playback_job(job, &mut last_sequence);
            }
        })
        .map_err(|error| format!("meeting_playback:runtime_spawn_failed:{error}"))?;

    *guard = Some(MeetingPlaybackRuntime {
        generation,
        session_id: runtime_session_id,
        sender,
        thread: Some(handle),
    });
    Ok(())
}

pub(super) fn enqueue_meeting_playback(
    mut job: PreparedPlaybackJob,
) -> Result<(), String> {
    let sender = {
        let guard = playback_runtime_store()
            .lock()
            .map_err(|_| "meeting_playback:runtime_state_lock_failed".to_string())?;
        let runtime = guard
            .as_ref()
            .filter(|runtime| {
                runtime.generation == job.generation && runtime.session_id == job.session_id
            })
            .ok_or_else(|| "meeting_playback:runtime_not_active".to_string())?;
        runtime.sender.clone()
    };

    loop {
        if !generation_is_live(job.generation) {
            cleanup_job(&job);
            return Err("meeting_playback:generation_not_authoritative".to_string());
        }
        match sender.try_send(job) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                job = returned;
                thread::sleep(ENQUEUE_RETRY_DELAY);
            }
            Err(TrySendError::Disconnected(returned)) => {
                cleanup_job(&returned);
                return Err("meeting_playback:runtime_disconnected".to_string());
            }
        }
    }
}

pub(super) fn stop_meeting_playback_runtime(
    generation: u64,
) -> MeetingPlaybackCleanupResult {
    let runtime = match playback_runtime_store().lock() {
        Ok(mut guard) => {
            if guard
                .as_ref()
                .map(|runtime| runtime.generation == generation)
                .unwrap_or(false)
            {
                guard.take()
            } else {
                None
            }
        }
        Err(_) => {
            return MeetingPlaybackCleanupResult {
                ok: false,
                message: "Meeting playback runtime state lock failed during cleanup.".to_string(),
            };
        }
    };

    let Some(mut runtime) = runtime else {
        return MeetingPlaybackCleanupResult {
            ok: true,
            message: "No matching Meeting playback runtime required cleanup.".to_string(),
        };
    };

    drop(runtime.sender);
    let joined = runtime
        .thread
        .take()
        .map(|handle| handle.join().is_ok())
        .unwrap_or(true);
    MeetingPlaybackCleanupResult {
        ok: joined,
        message: if joined {
            format!(
                "Meeting playback runtime stopped for {} generation {}.",
                runtime.session_id, generation
            )
        } else {
            format!(
                "Meeting playback runtime for {} generation {} exited unexpectedly during cleanup.",
                runtime.session_id, generation
            )
        },
    }
}


#[cfg(test)]
mod tests {
    use super::{sequence_is_monotonic, PLAYBACK_QUEUE_CAPACITY};

    #[test]
    fn bounded_playback_keeps_exactly_one_pending_slot() {
        assert_eq!(PLAYBACK_QUEUE_CAPACITY, 1);
    }

    #[test]
    fn playback_sequence_accepts_gaps_but_rejects_duplicate_or_older_output() {
        assert!(sequence_is_monotonic(10, 11));
        assert!(sequence_is_monotonic(10, 12));
        assert!(!sequence_is_monotonic(10, 10));
        assert!(!sequence_is_monotonic(10, 9));
    }
}

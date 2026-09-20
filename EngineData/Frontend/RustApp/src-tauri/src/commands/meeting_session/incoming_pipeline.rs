use serde_json::json;

use crate::engine::audio::live_segment_writer::remove_finalized_meeting_utterance_wav;
use crate::engine::runtime_settings::load_settings;

use super::committed_turns::commit_meeting_turn;
use super::incoming_deferred::{
    classify_incoming_asr_response, deferred_enqueue_unix_ms, enqueue_deferred_incoming,
    incoming_deferred_for_required_outbound, requeue_deferred_incoming_front,
    take_due_deferred_incoming, DeferredIncomingJob, DeferredIncomingStage,
    IncomingAsrDisposition, MAX_DEFERRED_INCOMING,
};
use super::{incoming_session_is_eligible, worker_blocker, worker_text};
use super::session_state::update_incoming_status;
use super::super::helper_bridge::send_helper_worker_task;
use super::super::helper_bridge_runtime::unix_ms;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum IncomingAudioProcessResult {
    Complete,
    DeferredAsr,
    DeferredTranslation,
}

pub(super) fn process_authoritative_finalized_incoming_wav(
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    audio_path: &str,
    deferred_enqueued_unix_ms: Option<u64>,
) -> IncomingAudioProcessResult {
    let retrying_deferred = deferred_enqueued_unix_ms.is_some();
    if !incoming_session_is_eligible(session_id) {
        return IncomingAudioProcessResult::Complete;
    }
    let asr_hotwords = load_settings().asr_hotwords();

    update_incoming_status(
        session_id,
        "transcribing",
        false,
        "",
        "Finalized English Meeting Sound is being transcribed locally.",
    );
    let asr = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": audio_path,
            "language": "en",
            "beam_size": 1,
            "vad_filter": true,
            "hotwords": asr_hotwords,
            "meeting_session_id": session_id,
            "meeting_lane": "incoming",
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    if !incoming_session_is_eligible(session_id) {
        return IncomingAudioProcessResult::Complete;
    }

    let transcript = match classify_incoming_asr_response(&asr) {
        IncomingAsrDisposition::Deferred => {
            let enqueued_unix_ms =
                deferred_enqueue_unix_ms(deferred_enqueued_unix_ms, unix_ms() as u64);
            let held = if retrying_deferred {
                requeue_deferred_incoming_front(DeferredIncomingJob {
                    session_id: session_id.to_string(),
                    event_sequence,
                    utterance_id,
                    stage: DeferredIncomingStage::NeedsAsr {
                        audio_path: audio_path.to_string(),
                    },
                    enqueued_unix_ms,
                })
            } else {
                enqueue_deferred_incoming(DeferredIncomingJob {
                    session_id: session_id.to_string(),
                    event_sequence,
                    utterance_id,
                    stage: DeferredIncomingStage::NeedsAsr {
                        audio_path: audio_path.to_string(),
                    },
                    enqueued_unix_ms,
                })
            };
            if held == 0 {
                update_incoming_status(
                    session_id,
                    "degraded",
                    true,
                    "meeting_incoming:deferred_queue_unavailable",
                    "Incoming English ASR yielded to required outbound work, but the deferred queue was unavailable. Outbound remains available.",
                );
                return IncomingAudioProcessResult::Complete;
            }
            update_incoming_status(
                session_id,
                "listening",
                false,
                "",
                &format!(
                    "Held {held}/{MAX_DEFERRED_INCOMING} before ASR while your outbound translation finishes. This Meeting Sound segment will resume first when the helper pipeline is available."
                ),
            );
            return IncomingAudioProcessResult::DeferredAsr;
        }
        IncomingAsrDisposition::EmptyTranscript => {
            update_incoming_status(
                session_id,
                "listening",
                false,
                "",
                "Finalized Meeting Sound did not produce stable English speech. Incoming remains listening.",
            );
            return IncomingAudioProcessResult::Complete;
        }
        IncomingAsrDisposition::Failed(blocker) => {
            update_incoming_status(
                session_id,
                "listening",
                true,
                &blocker,
                "Incoming English ASR failed for the latest finalized Meeting Sound event. Outbound remains available.",
            );
            return IncomingAudioProcessResult::Complete;
        }
        IncomingAsrDisposition::Transcript(transcript) => transcript,
    };

    let held = translate_and_commit_incoming_transcript(
        session_id,
        event_sequence,
        utterance_id,
        &transcript,
        !retrying_deferred,
    );
    if held && retrying_deferred {
        let held_count = requeue_deferred_incoming_front(DeferredIncomingJob {
            session_id: session_id.to_string(),
            event_sequence,
            utterance_id,
            stage: DeferredIncomingStage::NeedsTranslation {
                transcript: transcript.clone(),
            },
            enqueued_unix_ms: deferred_enqueue_unix_ms(
                deferred_enqueued_unix_ms,
                unix_ms() as u64,
            ),
        });
        if held_count == 0 {
            update_incoming_status(
                session_id,
                "degraded",
                true,
                "meeting_incoming:deferred_queue_unavailable",
                "Incoming translation yielded to required outbound work, but the deferred queue was unavailable. Outbound remains available.",
            );
            return IncomingAudioProcessResult::Complete;
        }
        update_incoming_status(
            session_id,
            "listening",
            false,
            "",
            &format!(
                "Held {held_count}/{MAX_DEFERRED_INCOMING} after ASR while your outbound translation finishes. The original incoming order is preserved."
            ),
        );
        return IncomingAudioProcessResult::DeferredTranslation;
    }
    if held {
        IncomingAudioProcessResult::DeferredTranslation
    } else {
        IncomingAudioProcessResult::Complete
    }
}

fn translate_and_commit_incoming_transcript(
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    transcript: &str,
    allow_defer: bool,
) -> bool {
    update_incoming_status(
        session_id,
        "translating",
        false,
        "",
        "Final English Meeting Sound transcript is being translated to Indonesian.",
    );
    let terminology = load_settings().terminology;
    let translation = send_helper_worker_task(
        "translate",
        json!({
            "text": transcript,
            "source_language": "en",
            "target_language": "id",
            "max_new_tokens": 96,
            "terminology": terminology,
            "meeting_session_id": session_id,
            "meeting_lane": "incoming",
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    if !incoming_session_is_eligible(session_id) {
        return false;
    }
    if incoming_deferred_for_required_outbound(&translation) {
        if allow_defer {
            let held = enqueue_deferred_incoming(DeferredIncomingJob {
                session_id: session_id.to_string(),
                event_sequence,
                utterance_id,
                stage: DeferredIncomingStage::NeedsTranslation {
                    transcript: transcript.to_string(),
                },
                enqueued_unix_ms: unix_ms() as u64,
            });
            if held == 0 {
                update_incoming_status(
                    session_id,
                    "degraded",
                    true,
                    "meeting_incoming:deferred_queue_unavailable",
                    "Incoming translation yielded to required outbound work, but the deferred queue was unavailable. Outbound remains available.",
                );
                return false;
            }
            update_incoming_status(
                session_id,
                "listening",
                false,
                "",
                &format!(
                    "Held {held}/{MAX_DEFERRED_INCOMING} while your outbound translation finishes. It will run right after."
                ),
            );
        }
        return true;
    }
    let translated_text = worker_text(&translation, "translated_text");
    if !translation.ok || translated_text.is_none() {
        let blocker = worker_blocker(&translation, "translation:empty_output");
        update_incoming_status(
            session_id,
            "degraded",
            true,
            &blocker,
            "Incoming English -> Indonesian translation failed for the latest event. Outbound Meeting translation remains unaffected.",
        );
        return false;
    }

    let translated_text = translated_text.unwrap_or_default();
    let committed = commit_meeting_turn(
        session_id,
        event_sequence,
        None,
        utterance_id,
        "incoming",
        &transcript,
        &translated_text,
        None,
        None,
    );
    update_incoming_status(
        session_id,
        if committed { "listening" } else { "degraded" },
        !committed,
        if committed {
            ""
        } else {
            "meeting_incoming:commit_rejected"
        },
        if committed {
            "Incoming Indonesian translation was committed in finalized speech/event order. Listening for current Meeting Sound."
        } else {
            "Incoming translation finished but could not be committed to the canonical Meeting conversation store."
        },
    );
    false
}

pub(super) fn drain_due_deferred_incoming(session_id: &str) {
    for _ in 0..MAX_DEFERRED_INCOMING {
        let Some(job) = take_due_deferred_incoming(session_id, unix_ms() as u64) else {
            break;
        };
        let DeferredIncomingJob {
            event_sequence,
            utterance_id,
            stage,
            enqueued_unix_ms,
            ..
        } = job;
        match stage {
            DeferredIncomingStage::NeedsAsr { audio_path } => {
                let result = process_authoritative_finalized_incoming_wav(
                    session_id,
                    event_sequence,
                    utterance_id,
                    &audio_path,
                    Some(enqueued_unix_ms),
                );
                match result {
                    IncomingAudioProcessResult::DeferredAsr => break,
                    IncomingAudioProcessResult::DeferredTranslation => {
                        remove_finalized_meeting_utterance_wav(&audio_path);
                        break;
                    }
                    IncomingAudioProcessResult::Complete => {
                        remove_finalized_meeting_utterance_wav(&audio_path);
                    }
                }
            }
            DeferredIncomingStage::NeedsTranslation { transcript } => {
                let held = translate_and_commit_incoming_transcript(
                    session_id,
                    event_sequence,
                    utterance_id,
                    &transcript,
                    false,
                );
                if held {
                    // Outbound still has priority; keep the original job at the
                    // front so ordering and the original age budget are preserved.
                    let held_count = requeue_deferred_incoming_front(DeferredIncomingJob {
                        session_id: session_id.to_string(),
                        event_sequence,
                        utterance_id,
                        stage: DeferredIncomingStage::NeedsTranslation { transcript },
                        enqueued_unix_ms,
                    });
                    if held_count == 0 {
                        update_incoming_status(
                            session_id,
                            "degraded",
                            true,
                            "meeting_incoming:deferred_queue_unavailable",
                            "Incoming translation yielded again, but the deferred queue was unavailable. Outbound remains available.",
                        );
                    }
                    break;
                }
            }
        }
    }
}

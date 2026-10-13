use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::engine::audio::finalized_utterance::{
    clear_finalized_incoming_utterance_producer_for_session,
    reset_finalized_incoming_speech_boundary,
};
use crate::engine::audio::meeting_sound_capture::stop_meeting_sound_capture_runtime_for_session;

use super::incoming_deferred::clear_deferred_incoming_queue_for_session;
use super::session_state::update_incoming_status;

struct MeetingSelfOutputSuppression {
    session_id: String,
    active: Arc<AtomicBool>,
}

pub(super) struct SelfOutputSuppressionGuard {
    active: Arc<AtomicBool>,
    session_id: String,
}

impl Drop for SelfOutputSuppressionGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        // The old guard must not reset the current producer of another session.
        let _ = reset_finalized_incoming_speech_boundary(&self.session_id);
    }
}

static MEETING_SELF_OUTPUT_SUPPRESSION: OnceLock<Mutex<Option<MeetingSelfOutputSuppression>>> =
    OnceLock::new();

fn suppression_store() -> &'static Mutex<Option<MeetingSelfOutputSuppression>> {
    MEETING_SELF_OUTPUT_SUPPRESSION.get_or_init(|| Mutex::new(None))
}

pub(super) fn reset_self_output_suppression(session_id: &str) -> Arc<AtomicBool> {
    let active = Arc::new(AtomicBool::new(false));
    if let Ok(mut guard) = suppression_store().lock() {
        *guard = Some(MeetingSelfOutputSuppression {
            session_id: session_id.to_string(),
            active: Arc::clone(&active),
        });
    }
    active
}

pub(super) fn suppression_handle_for_session(session_id: &str) -> Option<Arc<AtomicBool>> {
    suppression_store()
        .lock()
        .ok()
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|value| (value.session_id.clone(), Arc::clone(&value.active)))
        })
        .filter(|(stored_session, _)| stored_session == session_id)
        .map(|(_, active)| active)
}

pub(super) fn begin_self_output_suppression(session_id: &str) -> Option<SelfOutputSuppressionGuard> {
    let active = suppression_handle_for_session(session_id)?;
    let _ = reset_finalized_incoming_speech_boundary(session_id);
    active.store(true, Ordering::Release);
    update_incoming_status(
        session_id,
        "suppressed",
        false,
        "",
        "Incoming Meeting Sound is temporarily suppressed while TranslateIT's own English TTS is routed to the Meeting Microphone.",
    );
    Some(SelfOutputSuppressionGuard { active, session_id: session_id.to_string() })
}

pub(super) fn disable_optional_incoming_for_outbound(session_id: &str) -> String {
    // Revoke incoming eligibility before clearing its producer or deferred queue.
    // An in-flight ASR/translation must not requeue work after cleanup starts.
    // Required outbound playback remains independent of this optional lane.
    update_incoming_status(
        session_id,
        "disabled",
        true,
        "meeting_incoming:self_output_suppression_unavailable",
        "Incoming Meeting Sound was disabled because TranslateIT could not establish self-output suppression. Required outbound translation continues through the Meeting Microphone.",
    );
    // A delayed suppression failure from the prior Meeting must never
    // discard the current Meeting's producer, queue or sound-capture stream.
    let producer_cleared = clear_finalized_incoming_utterance_producer_for_session(session_id);
    let deferred_cleanup = clear_deferred_incoming_queue_for_session(session_id);
    let capture_stop = stop_meeting_sound_capture_runtime_for_session(session_id);
    let mut result = capture_stop.message;
    if !producer_cleared {
        result.push_str(" Incoming producer cleanup could not be verified.");
    }
    if let Err(error) = deferred_cleanup {
        result.push_str(&format!(" Deferred incoming cleanup: {error}"));
    }
    result
}

pub(super) fn clear_self_output_suppression_for_session(session_id: &str) -> bool {
    let cleared = match suppression_store().lock() {
        Ok(mut guard) => {
            if let Some(value) = guard.as_ref() {
                if value.session_id == session_id {
                    value.active.store(false, Ordering::Release);
                    *guard = None;
                }
            }
            true
        }
        Err(_) => false,
    };
    let _ = reset_finalized_incoming_speech_boundary(session_id);
    cleared
}

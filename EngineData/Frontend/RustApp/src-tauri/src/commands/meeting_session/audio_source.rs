use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use crate::engine::audio::finalized_utterance::reset_finalized_outbound_speech_boundary;
use crate::engine::audio::meeting_output::{
    cancel_meeting_output_for_generation, start_original_voice_output, stop_original_voice_output,
};
use crate::engine::audio::original_voice_pcm;
use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};
use super::super::helper_bridge::{
    get_helper_bridge_status, prepare_required_outbound_ai_runtime,
    required_outbound_voice_actor_token, start_helper_bridge,
};
use super::consumer_runtime::ensure_meeting_outbound_consumer;
use super::playback_runtime::ensure_meeting_playback_runtime;
use super::super::virtual_mic_route::get_bound_virtual_mic_output_device;
static SOURCE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[derive(Debug, Clone, Serialize)]
pub struct MeetingAudioSourceActionResult {
    pub ok: bool,
    pub source: String,
    pub message: String,
}
fn response(ok: bool, generation: Option<u64>, message: &str) -> MeetingAudioSourceActionResult {
    MeetingAudioSourceActionResult {
        ok,
        source: if generation.is_some_and(original_voice_pcm::is_original_active) {
            "original".to_string()
        } else { "translateit".to_string() },
        message: message.to_string(),
    }
}
/// Switch exactly one source for the already-live Meeting generation.
pub(super) fn set_source(requested: &str) -> MeetingAudioSourceActionResult {
    if !matches!(requested, "original" | "translateit") {
        return response(false, None, "Unknown microphone source.");
    }
    let Ok(_transition) = SOURCE_CHANGE_LOCK.get_or_init(|| Mutex::new(())).lock() else {
        return response(false, None, "Microphone source control is temporarily unavailable.");
    };
    let current = latest_runtime_session_state();
    let Some(session) = current.snapshot.filter(|item| {
        item.owner_id == APPLICATION_MEETING_OWNER_ID && item.authority_active && item.phase == "live"
    }) else {
        return response(false, None, "Start the Meeting before changing microphone source.");
    };
    let generation = session.generation;
    let session_id = session.session_id;
    let original = original_voice_pcm::is_original_active(generation);
    if (requested == "original") == original {
        return response(true, Some(generation), "Microphone source is already selected.");
    }
    if requested == "translateit" {
        let mut helper = get_helper_bridge_status();
        if matches!(helper.state.as_str(), "stopped" | "not_started") {
            let started = start_helper_bridge();
            if !started.ok {
                return response(false, Some(generation),
                    "Translation runtime could not start. Original Microphone stays active.");
            }
            helper = get_helper_bridge_status();
        }
        if helper.state != "ready" || !helper.provider_ready {
            return response(false, Some(generation),
                "Translation runtime is unavailable. Original Microphone stays active.");
        }
        if required_outbound_voice_actor_token(generation).is_none()
            && prepare_required_outbound_ai_runtime(generation).is_err()
        {
            return response(false, Some(generation),
                "AI translation validation failed. Original Microphone stays active.");
        }
        if required_outbound_voice_actor_token(generation).is_none() {
            return response(false, Some(generation),
                "A usable translation voice is not ready. Original Microphone stays active.");
        }
        if ensure_meeting_playback_runtime(generation, &session_id).is_err()
            || ensure_meeting_outbound_consumer(generation, &session_id).is_err()
        {
            return response(false, Some(generation),
                "Translation output could not initialize. Original Microphone stays active.");
        }
        if reset_finalized_outbound_speech_boundary(&session_id, generation).is_none() {
            return response(false, Some(generation), "Could not reset the speech boundary.");
        }
        if !stop_original_voice_output(generation) {
            return response(false, Some(generation),
                "Original output cleanup is incomplete. Stop Meeting to recover.");
        }
        return response(true, Some(generation), "TranslateIT Audio selected.");
    }
    let Ok(epoch) = original_voice_pcm::next_original_epoch(generation) else {
        return response(false, Some(generation), "Audio source state is unavailable.");
    };
    let output_device = match get_bound_virtual_mic_output_device(generation) {
        Ok(device) => device,
        Err(_) => return response(false, Some(generation), "Virtual microphone route is unavailable."),
    };
    if reset_finalized_outbound_speech_boundary(&session_id, generation).is_none() {
        return response(false, Some(generation), "Could not reset the speech boundary.");
    }
    // Epoch cutover invalidates old ASR/MT/TTS and playback callbacks.
    if original_voice_pcm::begin(generation, epoch).is_err() {
        return response(false, Some(generation), "Original microphone buffer is unavailable.");
    }
    if !cancel_meeting_output_for_generation(generation) {
        original_voice_pcm::stop(generation);
        return response(false, Some(generation), "Could not cancel previous translated output.");
    }
    match start_original_voice_output(&output_device, generation, epoch) {
        Ok(()) => response(true, Some(generation), "Original Microphone selected."),
        Err(_) => {
            original_voice_pcm::stop(generation);
            response(false, Some(generation),
                "Original output could not start. Check virtual audio or stop the Meeting.")
        }
    }
}

use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::engine::audio::finalized_utterance::reset_finalized_outbound_speech_boundary;
use crate::engine::audio::meeting_output::{
    cancel_meeting_output_for_generation, start_original_voice_output, stop_original_voice_output,
};
use crate::engine::audio::original_voice_pcm;
use crate::engine::runtime_state::{
    latest_runtime_session_state, mark_runtime_session_cleanup_incomplete,
    runtime_generation_is_authoritative, APPLICATION_MEETING_OWNER_ID,
};
use super::super::helper_bridge::{
    get_helper_bridge_status, prepare_required_outbound_ai_runtime,
    required_outbound_voice_actor_token, start_helper_bridge,
};
use super::consumer_runtime::ensure_meeting_outbound_consumer;
use super::playback_runtime::ensure_meeting_playback_runtime;
use super::super::virtual_mic_route::get_bound_virtual_mic_output_device;
static SOURCE_CHANGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static PREFER_ORIGINAL: AtomicBool = AtomicBool::new(false);

pub(super) fn preferred_source() -> &'static str {
    if PREFER_ORIGINAL.load(Ordering::Acquire) { "original" } else { "translateit" }
}

pub(super) fn remember_preferred_source(source: &str) {
    if matches!(source, "original" | "translateit") {
        PREFER_ORIGINAL.store(source == "original", Ordering::Release);
    }
}

/// Keep the pre-Start choice in the same native owner as the Live source.
/// This is ephemeral intent; it does not change Windows audio devices.
pub(super) fn set_preferred_source(source: &str) -> MeetingAudioSourceActionResult {
    if !matches!(source, "original" | "translateit") {
        return MeetingAudioSourceActionResult {
            ok: false, source: preferred_source().to_string(),
            message: "Unknown microphone source.".to_string(),
        };
    }
    let Ok(_lock) = SOURCE_CHANGE_LOCK.get_or_init(|| Mutex::new(())).lock() else {
        return MeetingAudioSourceActionResult {
            ok: false, source: preferred_source().to_string(),
            message: "Meeting source selection is temporarily unavailable.".to_string(),
        };
    };
    let state = latest_runtime_session_state();
    if state.has_active_session || state.snapshot.is_some()
        || super::lifecycle::transient_stop_cleanup_pending()
    {
        return MeetingAudioSourceActionResult {
            ok: false, source: preferred_source().to_string(),
            message: "You can choose the next source only before Meeting starts.".to_string(),
        };
    }
    remember_preferred_source(source);
    MeetingAudioSourceActionResult {
        ok: true, source: preferred_source().to_string(),
        message: if source == "original" {
            "Original Voice selected for the next Meeting."
        } else {
            "TranslateIT Voice selected for the next Meeting."
        }.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingAudioSourceActionResult {
    pub ok: bool,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingMuteActionResult {
    pub ok: bool,
    pub muted: bool,
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
// AI preparation may overlap Stop. Recheck the session before admitting native
// output or worker consumers; source switching never owns a second Meeting.
fn still_live(generation: u64, session_id: &str) -> bool {
    runtime_generation_is_authoritative(generation)
        && latest_runtime_session_state().snapshot.is_some_and(|session| {
            session.generation == generation
                && session.session_id == session_id
                && session.owner_id == APPLICATION_MEETING_OWNER_ID
                && session.authority_active && session.phase == "live"
        })
}

// Uncertain native output teardown must never silently enable the other source.
// Revoke authority and expose canonical Stop as the only recovery action.
fn require_stop(generation: u64, reason: &str) -> MeetingAudioSourceActionResult {
    original_voice_pcm::stop(generation);
    let _ = mark_runtime_session_cleanup_incomplete(generation, true, reason);
    response(false, Some(generation),
        "Microphone switching could not safely finish. Audio is blocked; select Stop Microphone to recover.")
}

/// Mute is an output gate, never a third microphone source. The same atomic
/// source gate covers Original and translated playback, independent of AI.
pub(super) fn set_muted(muted: bool) -> MeetingMuteActionResult {
    let reply = |ok: bool, generation: Option<u64>, message: &str| MeetingMuteActionResult {
        ok,
        muted: generation.is_some_and(original_voice_pcm::is_output_muted),
        message: message.to_string(),
    };
    let Ok(_lock) = SOURCE_CHANGE_LOCK.get_or_init(|| Mutex::new(())).lock() else {
        return reply(false, None, "Microphone control is temporarily unavailable.");
    };
    let current = latest_runtime_session_state();
    let Some(session) = current.snapshot.filter(|item|
        item.owner_id == APPLICATION_MEETING_OWNER_ID
            && item.authority_active && item.phase == "live") else {
        return reply(false, None, "Start the Meeting before using Mute.");
    };
    let generation = session.generation;
    if let Err(error) = original_voice_pcm::set_output_muted(generation, muted) {
        return reply(false, Some(generation), error);
    }
    // Discard any speech/pre-roll from the muted interval in the existing VAD.
    // Original mode can safely return None if its VAD producer is unavailable.
    let _ = reset_finalized_outbound_speech_boundary(&session.session_id, generation);
    if muted {
        // Output callback is already silent via MUTED. Cancellation additionally
        // interrupts the current translated WAV without affecting captions.
        if !cancel_meeting_output_for_generation(generation) {
            let _ = mark_runtime_session_cleanup_incomplete(
                generation, true, "Mute could not confirm native TTS cancellation.");
            return reply(false, Some(generation),
                "Mute was applied, but audio cleanup is uncertain. Use Stop to recover.");
        }
    }
    reply(true, Some(generation), if muted {
        "Microphone muted. Both Original and TranslateIT voice outputs are silent."
    } else {
        "Microphone unmuted. Only newly captured audio will be sent."
    })
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
        if !still_live(generation, &session_id) {
            return response(false, Some(generation),
                "Meeting stopped during translation preparation.");
        }
        if ensure_meeting_playback_runtime(generation, &session_id).is_err()
            || ensure_meeting_outbound_consumer(generation, &session_id).is_err()
        {
            return response(false, Some(generation),
                "Translation output could not initialize. Original Microphone stays active.");
        }
        if !still_live(generation, &session_id)
            || reset_finalized_outbound_speech_boundary(&session_id, generation).is_none()
        {
            return response(false, Some(generation), "Could not reset the speech boundary.");
        }
        if !stop_original_voice_output(generation) {
            return require_stop(generation, "Original output cleanup failed during source switching.");
        }
        if !still_live(generation, &session_id) {
            return response(false, Some(generation), "Meeting stopped during source switching.");
        }
        remember_preferred_source("translateit");
        return response(true, Some(generation), "TranslateIT Audio selected.");
    }
    let Ok(epoch) = original_voice_pcm::next_original_epoch(generation) else {
        return response(false, Some(generation), "Audio source state is unavailable.");
    };
    let output_device = match get_bound_virtual_mic_output_device(generation) {
        Ok(device) => device,
        Err(_) => return response(false, Some(generation), "Virtual microphone route is unavailable."),
    };
    if !still_live(generation, &session_id)
        || reset_finalized_outbound_speech_boundary(&session_id, generation).is_none()
    {
        return response(false, Some(generation), "Could not reset the speech boundary.");
    }
    // Epoch cutover invalidates old ASR/MT/TTS and playback callbacks.
    if original_voice_pcm::begin(generation, epoch).is_err() {
        return response(false, Some(generation), "Original microphone buffer is unavailable.");
    }
    if !cancel_meeting_output_for_generation(generation) {
        return require_stop(generation, "Translated output cancellation was not confirmed.");
    }
    if !still_live(generation, &session_id) {
        original_voice_pcm::stop(generation);
        return response(false, Some(generation), "Meeting stopped during source switching.");
    }
    match start_original_voice_output(&output_device, generation, epoch) {
        Ok(()) if still_live(generation, &session_id) => {
            remember_preferred_source("original");
            response(true, Some(generation), "Original Microphone selected.")
        },
        Ok(()) => {
            original_voice_pcm::stop(generation);
            response(false, Some(generation), "Meeting stopped during source switching.")
        }
        Err(_) => require_stop(generation, "Original output could not start safely after switching."),
    }
}

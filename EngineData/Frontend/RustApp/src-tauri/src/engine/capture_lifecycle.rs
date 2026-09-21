use crate::engine::audio::guided_take::active_guided_take_line_id;
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::runtime_state::{
    begin_mic_test_session, clear_runtime_session_if_generation,
    latest_runtime_session_state,
    mark_runtime_session_cleanup_incomplete, revoke_runtime_session_authority,
};
use crate::engine::state::{CommandResult, LifecycleState};

use crate::engine::runtime_state::MIC_TEST_OWNER_ID;

pub fn start_capture() -> CommandResult {
    if active_guided_take_line_id().is_some() {
        return CommandResult::blocked(
            LifecycleState::ConversionPending,
            "Mic Test cannot start while VoiceLab is recording a guided line.",
        );
    }

    let session = begin_mic_test_session();
    let Some(snapshot) = session.snapshot.as_ref() else {
        return CommandResult::blocked(
            LifecycleState::Error,
            "Microphone test cannot verify runtime ownership right now. No capture resource was opened.",
        );
    };
    if !session.blocker.is_empty() || snapshot.owner_id != MIC_TEST_OWNER_ID {
        return CommandResult::blocked(
            LifecycleState::ConversionPending,
            "Microphone test cannot start while Meeting or another capture session owns runtime resources.",
        );
    }

    let generation = snapshot.generation;
    let capture = start_live_capture_runtime(session);
    if capture.ok {
        CommandResult::ok(
            LifecycleState::Listening,
            "Microphone test recording started. This capture-only path does not run ASR, translation, or TTS.",
        )
    } else {
        let _ = clear_runtime_session_if_generation(generation);
        CommandResult::blocked(LifecycleState::Error, capture.message)
    }
}

pub fn stop_capture() -> CommandResult {
    if active_guided_take_line_id().is_some() {
        return CommandResult::blocked(
            LifecycleState::ConversionPending,
            "Mic Test Stop cannot control a VoiceLab guided recording. Stop the take from VoiceLab instead.",
        );
    }

    let current = latest_runtime_session_state();
    if current.has_active_session && current.snapshot.is_none() {
        return CommandResult::blocked(
            LifecycleState::Error,
            "Microphone test cannot verify runtime ownership right now. Capture state was left unchanged.",
        );
    }
    if let Some(snapshot) = current.snapshot.as_ref() {
        if snapshot.owner_id != MIC_TEST_OWNER_ID {
            return CommandResult::blocked(
                LifecycleState::ConversionPending,
                "Microphone test Stop cannot control an active Meeting session. Stop Translation from the Meeting workspace instead.",
            );
        }
    }

    let generation = current
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.generation);
    if let Some(generation) = generation {
        let revoked = revoke_runtime_session_authority(
            generation,
            "Mic Test Stop accepted. Runtime generation authority was revoked before microphone cleanup.",
        );
        if revoked.snapshot.is_none()
            || revoked
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.authority_active)
                .unwrap_or(true)
        {
            return CommandResult::blocked(
                LifecycleState::Error,
                "Microphone test could not revoke runtime ownership safely. Capture cleanup was not claimed complete.",
            );
        }
    }

    let capture = stop_live_capture_runtime();
    if !capture.ok {
        if let Some(generation) = generation {
            let _ = mark_runtime_session_cleanup_incomplete(
                generation,
                true,
                "Mic Test output authority is revoked, but microphone capture cleanup did not complete. Retry Stop Mic Test.",
            );
        }
        return CommandResult::blocked(LifecycleState::Error, capture.message);
    }

    if let Some(generation) = generation {
        let cleared = clear_runtime_session_if_generation(generation);
        if cleared.has_active_session
            || cleared.snapshot.is_some()
            || cleared.blocker != "runtime_session:cleared"
        {
            return CommandResult::blocked(
                LifecycleState::Error,
                "Microphone capture stopped, but TranslateIT could not confirm that Mic Test ownership was cleared. Keep the app open and retry.",
            );
        }
    }

    CommandResult::ok(
        LifecycleState::Stopped,
        "Microphone test recording stopped and microphone ownership was released.",
    )
}

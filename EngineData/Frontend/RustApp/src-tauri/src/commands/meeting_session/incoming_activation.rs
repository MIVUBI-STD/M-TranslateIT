use std::thread;

use crate::engine::audio::meeting_sound_capture::{
    start_meeting_sound_capture_runtime, stop_meeting_sound_capture_runtime,
};

use super::consumer_runtime::{
    start_meeting_incoming_consumer, stop_meeting_incoming_consumer,
};
use super::session_state::update_incoming_status;
use super::suppression::suppression_handle_for_session;
use super::incoming_session_is_eligible;

fn start_optional_incoming_lane(session_id: &str) -> String {
    if !incoming_session_is_eligible(session_id) {
        return "Incoming activation skipped because the Meeting is no longer Live.".to_string();
    }

    let Some(suppression) = suppression_handle_for_session(session_id) else {
        update_incoming_status(
            session_id,
            "degraded",
            true,
            "meeting_incoming:suppression_state_unavailable",
            "Incoming Meeting Sound was not started because self-output suppression state was unavailable. Outbound remains Live.",
        );
        return "Incoming unavailable: suppression state could not be established.".to_string();
    };

    let capture = start_meeting_sound_capture_runtime(session_id, suppression);
    if !incoming_session_is_eligible(session_id) {
        if capture.ok {
            let _ = stop_meeting_sound_capture_runtime();
        }
        return "Incoming activation ended because the Meeting stopped while optional capture was opening."
            .to_string();
    }
    if !capture.ok {
        update_incoming_status(
            session_id,
            "degraded",
            true,
            if capture.status.blocker.is_empty() {
                "meeting_incoming:capture_unavailable"
            } else {
                &capture.status.blocker
            },
            &capture.message,
        );
        return format!("Incoming degraded: {}", capture.message);
    }

    if let Err(error) = start_meeting_incoming_consumer(session_id) {
        let _ = stop_meeting_sound_capture_runtime();
        update_incoming_status(
            session_id,
            "degraded",
            true,
            &error,
            "Meeting Sound capture opened, but the incoming consumer could not start. Outbound remains Live.",
        );
        return format!("Incoming degraded: {error}");
    }

    if !incoming_session_is_eligible(session_id) {
        let _ = stop_meeting_incoming_consumer(session_id);
        let _ = stop_meeting_sound_capture_runtime();
        return "Incoming activation ended because the Meeting stopped before optional capture became active."
            .to_string();
    }

    update_incoming_status(
        session_id,
        "listening",
        false,
        "",
        "Incoming Meeting Sound is listening for finalized English speech while this Meeting is Live.",
    );
    "Incoming Meeting Sound lane started.".to_string()
}

pub(super) fn schedule_optional_incoming_lane(session_id: &str) -> String {
    update_incoming_status(
        session_id,
        "starting",
        false,
        "",
        "Required outbound translation is Live. Optional incoming Meeting Sound is starting independently.",
    );
    let thread_session_id = session_id.to_string();
    match thread::Builder::new()
        .name("translateit-meeting-incoming-start".to_string())
        .spawn(move || {
            let _ = start_optional_incoming_lane(&thread_session_id);
        }) {
        Ok(_) => "Optional incoming Meeting Sound is starting independently.".to_string(),
        Err(error) => {
            update_incoming_status(
                session_id,
                "degraded",
                true,
                "meeting_incoming:activation_spawn_failed",
                "Optional incoming Meeting Sound could not start its activation task. Required outbound remains Live.",
            );
            format!("Incoming degraded: activation task could not start: {error}")
        }
    }
}

use super::contract::IntentOutcome;
use super::super::{helper_bridge, meeting_session, mic_test, runtime};

fn outcome(ok: bool, state: String, message: String) -> IntentOutcome {
    IntentOutcome { ok, state, message }
}

pub fn dispatch(intent: &str) -> IntentOutcome {
    match intent {
        "start_meeting" => {
            let result = runtime::start_meeting_translation();
            outcome(result.ok, result.state, result.message)
        }
        "stop_meeting" => {
            let result = meeting_session::stop_meeting_translation();
            outcome(result.ok, result.state, result.message)
        }
        "start_mic_test" => {
            let result = mic_test::start_capture();
            outcome(result.ok, result.state.to_string(), result.message)
        }
        "stop_mic_test" => {
            let result = mic_test::stop_capture();
            outcome(result.ok, result.state.to_string(), result.message)
        }
        "fix_setup" => fix_setup(),
        "refresh" => outcome(
            true,
            "refreshed".to_string(),
            "Application runtime snapshot refreshed.".to_string(),
        ),
        _ => outcome(
            false,
            "unsupported_intent".to_string(),
            format!("Unsupported product intent: {intent}"),
        ),
    }
}

fn fix_setup() -> IntentOutcome {
    let helper = helper_bridge::get_helper_bridge_status();
    if matches!(helper.state.as_str(), "not_started" | "stopped") {
        let started = helper_bridge::start_helper_bridge();
        if !started.ok {
            return outcome(false, started.state, started.message);
        }
    } else if helper.state != "ready" {
        return outcome(
            false,
            "helper_unavailable".to_string(),
            "Setup still needs attention because the local helper is unavailable.".to_string(),
        );
    }

    let readiness = runtime::verify_required_outbound_ai_readiness();
    outcome(readiness.ok, readiness.state, readiness.message)
}

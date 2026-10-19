use super::contract::IntentOutcome;
use super::super::{helper_bridge, meeting_session, mic_test, runtime};

fn outcome(ok: bool, state: String, message: String) -> IntentOutcome {
    IntentOutcome { ok, state, message }
}

fn helper_state_allows_start(state: &str) -> bool {
    matches!(state, "not_started" | "stopped")
}

pub fn dispatch(intent: &str) -> IntentOutcome {
    match intent {
        "start_meeting" => {
            let result = runtime::start_meeting_translation();
            outcome(result.ok, result.state, result.message)
        }
        "start_original_meeting" => {
            let result = runtime::start_meeting_original();
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
        "check_readiness" => {
            let readiness = runtime::verify_required_outbound_ai_readiness();
            outcome(readiness.ok, readiness.state, readiness.message)
        }
        "ensure_runtime_ready" => ensure_runtime_ready(),
        "retry_blocked_worker" => retry_blocked_worker(),
        _ => outcome(
            false,
            "unsupported_intent".to_string(),
            format!("Unsupported product intent: {intent}"),
        ),
    }
}

fn ensure_runtime_ready() -> IntentOutcome {
    let helper = helper_bridge::get_helper_bridge_status();
    if helper.state == "ready" {
        return outcome(
            true,
            "ready".to_string(),
            "Application runtime is ready.".to_string(),
        );
    }
    if helper_state_allows_start(&helper.state) {
        let started = helper_bridge::start_helper_bridge();
        return outcome(started.ok, started.state, started.message);
    }
    outcome(
        false,
        helper.state,
        "Application runtime is not ready yet.".to_string(),
    )
}

// Explicit user recovery only. Ordinary snapshots and ensure_runtime_ready never
// restart a blocked worker; active capture or GPU training cannot be interrupted.
fn retry_blocked_worker() -> IntentOutcome {
    let helper = helper_bridge::get_helper_bridge_status();
    if helper.state != "blocked" {
        return outcome(
            false,
            helper.state,
            "Retry Worker is available only when the worker is blocked.".to_string(),
        );
    }
    let snapshot = super::current_application_snapshot();
    if snapshot.summaries.meeting.has_session
        || snapshot.resources.audio_locked
        || snapshot.active_owner.is_some()
        || snapshot.summaries.voice.build_active
    {
        return outcome(
            false,
            "resource_busy".to_string(),
            "Cannot retry the blocked worker while audio, Meeting or voice training is active."
                .to_string(),
        );
    }
    let started = helper_bridge::start_helper_bridge();
    outcome(started.ok, started.state, started.message)
}

fn fix_setup() -> IntentOutcome {
    let helper = helper_bridge::get_helper_bridge_status();
    if helper_state_allows_start(&helper.state) {
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


#[cfg(test)]
mod tests {
    use super::helper_state_allows_start;

    #[test]
    fn helper_can_start_from_initial_or_cleanly_stopped_state() {
        assert!(helper_state_allows_start("not_started"));
        assert!(helper_state_allows_start("stopped"));
        assert!(!helper_state_allows_start("ready"));
        assert!(!helper_state_allows_start("error"));
        assert!(!helper_state_allows_start("blocked"));
    }
}

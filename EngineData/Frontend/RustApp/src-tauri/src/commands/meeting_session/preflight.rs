use crate::engine::runtime_state::{
    latest_runtime_session_state, RuntimeSessionStateReport,
};

use super::super::audio::get_input_status;
use super::super::helper_bridge::get_helper_bridge_status;
use super::super::virtual_mic_route::get_virtual_mic_route_selection;
use super::session_state::{
    current_incoming_status, current_outbound_status, current_start_preflight,
};
use super::{
    MeetingSessionActionResult, MeetingSessionPreflightStatus, MeetingSessionStatus,
    APPLICATION_MEETING_OWNER_ID,
};

#[cfg(test)]
fn helper_start_recoverable(state: &str, last_error: Option<&str>) -> bool {
    state == "stopped"
        && last_error == Some("helper_bridge:meeting_session_hard_cancelled")
}

pub(super) fn build_preflight() -> MeetingSessionPreflightStatus {
    let input = get_input_status();
    let helper = get_helper_bridge_status();
    let route = get_virtual_mic_route_selection();

    let microphone_ready = input.prepared;
    let helper_ready = helper.state == "ready";
    let provider_ready = helper.provider_ready;
    let functional_outbound_ready = helper.functional_outbound_ready;
    let functional_outbound_verified_unix_ms = helper.functional_outbound_verified_unix_ms;
    // `models_ready` remains the inexpensive required outbound capability view. C4
    // keeps functional truth separate so routine status stays cheap and Start can run
    // the bounded self-test only when needed.
    let models_ready = helper_ready && provider_ready;
    let meeting_route_ready = route.route_ready;

    let mut start_blockers = Vec::new();
    if !microphone_ready {
        start_blockers.push("meeting_session:microphone_not_ready".to_string());
    }
    if !models_ready {
        start_blockers.push("meeting_session:local_runtime_not_ready".to_string());
    }
    if !meeting_route_ready {
        start_blockers.push(if route.blocker.is_empty() {
            "meeting_session:meeting_microphone_route_not_ready".to_string()
        } else {
            route.blocker.clone()
        });
    }
    start_blockers.sort();
    start_blockers.dedup();

    let start_eligible = start_blockers.is_empty();
    let ready_for_start = start_eligible && functional_outbound_ready;
    let mut blockers = start_blockers;
    if start_eligible && !functional_outbound_ready {
        blockers.push("meeting_session:functional_outbound_not_verified".to_string());
    }

    MeetingSessionPreflightStatus {
        ready_for_start,
        start_eligible,
        functional_outbound_ready,
        functional_outbound_verified_unix_ms,
        microphone_ready,
        models_ready,
        helper_ready,
        provider_ready,
        meeting_route_ready,
        blockers,
        summary: if ready_for_start {
            "Required outbound Meeting capabilities are functionally verified for the current local worker and current preflight prerequisites are ready. Incoming Meeting Sound remains optional/degradable."
                .to_string()
        } else if start_eligible {
            "Required Meeting setup is available. A bounded local translation check must complete before Translation can become Live."
                .to_string()
        } else {
            "Start Translation remains blocked until all required current outbound Meeting prerequisites are available."
                .to_string()
        },
        runtime_claim: "meeting_start_preflight_source_contract_not_windows_runtime_proof"
            .to_string(),
    }
}

pub(super) fn status_from_report(
    report: RuntimeSessionStateReport,
    preflight: MeetingSessionPreflightStatus,
) -> MeetingSessionStatus {
    let snapshot = report.snapshot.as_ref();
    MeetingSessionStatus {
        lifecycle: snapshot
            .map(|value| value.phase.clone())
            .unwrap_or_else(|| "idle".to_string()),
        has_session: report.has_active_session,
        authority_active: snapshot
            .map(|value| value.authority_active)
            .unwrap_or(false),
        session_id: snapshot.map(|value| value.session_id.clone()),
        generation: snapshot.map(|value| value.generation),
        started_unix_ms: snapshot.map(|value| value.started_unix_ms),
        active_age_ms: report.active_age_ms,
        capture_active: snapshot
            .map(|value| value.live_capture_stream_active)
            .unwrap_or(false),
        owner_id: snapshot.map(|value| value.owner_id.clone()),
        blocker: report.blocker,
        note: report.note,
        preflight,
        outbound: current_outbound_status(),
        incoming: current_incoming_status(),
        runtime_claim: "application_meeting_session_source_contract_not_windows_runtime_proof"
            .to_string(),
    }
}

fn preflight_for_report(report: &RuntimeSessionStateReport) -> MeetingSessionPreflightStatus {
    if let Some(snapshot) = report.snapshot.as_ref() {
        if snapshot.owner_id == APPLICATION_MEETING_OWNER_ID {
            if let Some(cached) = current_start_preflight(snapshot.generation) {
                return cached;
            }
        }
    }
    build_preflight()
}

pub(super) fn current_status() -> MeetingSessionStatus {
    let report = latest_runtime_session_state();
    let preflight = preflight_for_report(&report);
    status_from_report(report, preflight)
}

pub(super) fn blocked_result(state: &str, message: String) -> MeetingSessionActionResult {
    MeetingSessionActionResult {
        ok: false,
        state: state.to_string(),
        message,
        status: current_status(),
    }
}


#[cfg(test)]
mod recovery_eligibility_tests {
    use super::helper_start_recoverable;

    #[test]
    fn only_intentional_meeting_hard_cancel_is_start_recoverable() {
        assert!(helper_start_recoverable(
            "stopped",
            Some("helper_bridge:meeting_session_hard_cancelled"),
        ));
        assert!(!helper_start_recoverable(
            "stopped",
            Some("helper_bridge:worker_exited"),
        ));
        assert!(!helper_start_recoverable(
            "stopped",
            Some("helper_bridge:translate_read_failed:deadline"),
        ));
        assert!(!helper_start_recoverable("error", None));
        assert!(!helper_start_recoverable("ready", None));
    }
}

use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use super::helper_bridge::get_helper_bridge_status;
use super::helper_bridge_runtime::{
    WORKER_INFERENCE_RESPONSE_DEADLINE_MS, WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS,
};
use super::meeting_session::{get_meeting_session_status, MeetingSessionStatus};

const WATCHDOG_GRACE_MS: u128 = 15_000;
const DELIVERY_STALL_THRESHOLD_MS: u128 = 120_000;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeWatchdogStatus {
    pub state: String,
    pub healthy: bool,
    pub action_required: bool,
    pub component: String,
    pub stage: String,
    pub age_ms: u128,
    pub threshold_ms: u128,
    pub blocker: String,
    pub note: String,
    pub updated_unix_ms: u128,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn stage_threshold(stage: &str) -> Option<u128> {
    match stage {
        "transcribing" | "translating" => {
            Some(WORKER_INFERENCE_RESPONSE_DEADLINE_MS + WATCHDOG_GRACE_MS)
        }
        "synthesizing" => Some(WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS + WATCHDOG_GRACE_MS),
        "delivering" => Some(DELIVERY_STALL_THRESHOLD_MS),
        _ => None,
    }
}

fn helper_task_threshold(task: &str) -> Option<u128> {
    match task {
        "transcribe" | "translate" => {
            Some(WORKER_INFERENCE_RESPONSE_DEADLINE_MS + WATCHDOG_GRACE_MS)
        }
        "voice_actor_synthesize" => {
            Some(WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS + WATCHDOG_GRACE_MS)
        }
        _ => None,
    }
}

fn healthy_status(state: &str, note: &str) -> RuntimeWatchdogStatus {
    RuntimeWatchdogStatus {
        state: state.to_string(),
        healthy: true,
        action_required: false,
        component: String::new(),
        stage: String::new(),
        age_ms: 0,
        threshold_ms: 0,
        blocker: String::new(),
        note: note.to_string(),
        updated_unix_ms: unix_ms(),
    }
}

fn stalled_status(
    component: &str,
    stage: &str,
    age_ms: u128,
    threshold_ms: u128,
    blocker: &str,
    note: &str,
) -> RuntimeWatchdogStatus {
    RuntimeWatchdogStatus {
        state: "suspected_stall".to_string(),
        healthy: false,
        action_required: true,
        component: component.to_string(),
        stage: stage.to_string(),
        age_ms,
        threshold_ms,
        blocker: blocker.to_string(),
        note: note.to_string(),
        updated_unix_ms: unix_ms(),
    }
}

fn evaluate_meeting_watchdog(
    meeting: &MeetingSessionStatus,
    helper_task: Option<&str>,
    helper_session_id: Option<&str>,
    helper_updated_unix_ms: u128,
    now: u128,
) -> RuntimeWatchdogStatus {
    if !meeting.has_session {
        return healthy_status("idle", "No Meeting session is active.");
    }
    if meeting.lifecycle != "live" {
        return healthy_status(
            "observing",
            "Meeting is transitioning. Watchdog does not interfere with Starting or Stopping.",
        );
    }
    if !meeting.capture_active {
        return stalled_status(
            "microphone",
            "capture",
            0,
            0,
            "runtime_watchdog:live_capture_missing",
            "Meeting is Live but the required microphone capture is not active. Stop Translation and repair the audio setup before restarting.",
        );
    }

    if let Some(threshold) = stage_threshold(&meeting.outbound.stage) {
        let age = now.saturating_sub(meeting.outbound.updated_unix_ms);
        if age > threshold {
            return stalled_status(
                "meeting_outbound",
                &meeting.outbound.stage,
                age,
                threshold,
                "runtime_watchdog:outbound_progress_stalled",
                "The required outbound Meeting pipeline has not reported progress within its bounded stage deadline plus watchdog grace. Stop Translation before attempting recovery.",
            );
        }
    }

    if let Some(threshold) = stage_threshold(&meeting.incoming.stage) {
        let age = now.saturating_sub(meeting.incoming.updated_unix_ms);
        if age > threshold {
            return RuntimeWatchdogStatus {
                state: "degraded_optional_incoming".to_string(),
                healthy: false,
                action_required: false,
                component: "meeting_incoming".to_string(),
                stage: meeting.incoming.stage.clone(),
                age_ms: age,
                threshold_ms: threshold,
                blocker: "runtime_watchdog:incoming_progress_stalled".to_string(),
                note: "Optional incoming Meeting Sound appears stalled. Required outbound translation remains authoritative; use Diagnostics or restart Translation if incoming captions are needed.".to_string(),
                updated_unix_ms: now,
            };
        }
    }

    if helper_session_id == meeting.session_id.as_deref() {
        if let Some(task) = helper_task {
            if let Some(threshold) = helper_task_threshold(task) {
                let age = now.saturating_sub(helper_updated_unix_ms);
                if age > threshold {
                    return stalled_status(
                        "helper_worker",
                        task,
                        age,
                        threshold,
                        "runtime_watchdog:helper_meeting_task_stalled",
                        "The local AI helper still reports an active Meeting task beyond its bounded deadline plus grace. Stop Translation before attempting helper recovery.",
                    );
                }
            }
        }
    }

    if !meeting.outbound.last_stage_ok && !meeting.outbound.blocker.is_empty() {
        return RuntimeWatchdogStatus {
            state: "degraded".to_string(),
            healthy: false,
            action_required: true,
            component: "meeting_outbound".to_string(),
            stage: meeting.outbound.stage.clone(),
            age_ms: now.saturating_sub(meeting.outbound.updated_unix_ms),
            threshold_ms: stage_threshold(&meeting.outbound.stage).unwrap_or(0),
            blocker: meeting.outbound.blocker.clone(),
            note: "The latest required outbound stage reported a failure. Use the existing Meeting recovery flow instead of an automatic watchdog restart.".to_string(),
            updated_unix_ms: now,
        };
    }

    healthy_status(
        "healthy",
        "No stalled required Meeting stage is currently detected.",
    )
}

#[tauri::command]
pub fn get_runtime_watchdog_status() -> RuntimeWatchdogStatus {
    let meeting = get_meeting_session_status();
    let helper = get_helper_bridge_status();
    evaluate_meeting_watchdog(
        &meeting,
        helper.active_task.as_deref(),
        helper.active_meeting_session_id.as_deref(),
        helper.updated_unix_ms,
        unix_ms(),
    )
}

#[cfg(test)]
mod tests {
    use super::evaluate_meeting_watchdog;
    use crate::commands::meeting_session::{
        MeetingIncomingRuntimeStatus, MeetingOutboundRuntimeStatus, MeetingSessionPreflightStatus,
        MeetingSessionStatus,
    };

    fn live_status(stage: &str, updated: u128) -> MeetingSessionStatus {
        MeetingSessionStatus {
            lifecycle: "live".to_string(),
            has_session: true,
            authority_active: true,
            session_id: Some("session-a".to_string()),
            generation: Some(1),
            started_unix_ms: Some(1),
            active_age_ms: Some(1),
            capture_active: true,
            owner_id: Some("translateit_application_meeting".to_string()),
            blocker: String::new(),
            note: String::new(),
            preflight: MeetingSessionPreflightStatus {
                ready_for_start: true,
                start_eligible: true,
                functional_outbound_ready: true,
                functional_outbound_verified_unix_ms: Some(1),
                microphone_ready: true,
                models_ready: true,
                helper_ready: true,
                provider_ready: true,
                meeting_route_ready: true,
                blockers: Vec::new(),
                summary: String::new(),
                runtime_claim: String::new(),
            },
            outbound: MeetingOutboundRuntimeStatus {
                generation: Some(1),
                session_id: Some("session-a".to_string()),
                stage: stage.to_string(),
                utterance_sequence: 1,
                output_active: false,
                last_stage_ok: true,
                timing: None,
                overflow_dropped_utterance_count: 0,
                evicted_pending_utterance_count: 0,
                blocker: String::new(),
                note: String::new(),
                updated_unix_ms: updated,
                runtime_claim: String::new(),
            },
            incoming: MeetingIncomingRuntimeStatus {
                session_id: Some("session-a".to_string()),
                stage: "listening".to_string(),
                capture_active: true,
                suppressed: false,
                degraded: false,
                blocker: String::new(),
                note: String::new(),
                updated_unix_ms: updated,
                runtime_claim: String::new(),
            },
            runtime_claim: String::new(),
        }
    }

    #[test]
    fn silence_while_listening_is_never_a_stall() {
        let meeting = live_status("listening", 1);
        let result = evaluate_meeting_watchdog(&meeting, None, None, 1, 500_000);
        assert_eq!(result.state, "healthy");
        assert!(result.healthy);
    }

    #[test]
    fn active_inference_stage_stalls_only_after_deadline_plus_grace() {
        let meeting = live_status("translating", 1_000);
        let healthy = evaluate_meeting_watchdog(&meeting, None, None, 1_000, 100_000);
        assert!(healthy.healthy);

        let stalled = evaluate_meeting_watchdog(&meeting, None, None, 1_000, 120_000);
        assert_eq!(stalled.state, "suspected_stall");
        assert!(stalled.action_required);
    }

    #[test]
    fn live_without_required_capture_is_immediate_recovery_condition() {
        let mut meeting = live_status("listening", 1_000);
        meeting.capture_active = false;
        let result = evaluate_meeting_watchdog(&meeting, None, None, 1_000, 2_000);
        assert_eq!(result.blocker, "runtime_watchdog:live_capture_missing");
        assert!(result.action_required);
    }

    #[test]
    fn helper_stall_is_generation_session_scoped() {
        let meeting = live_status("listening", 1_000);
        let ignored = evaluate_meeting_watchdog(
            &meeting,
            Some("translate"),
            Some("other-session"),
            1_000,
            200_000,
        );
        assert!(ignored.healthy);

        let stalled = evaluate_meeting_watchdog(
            &meeting,
            Some("translate"),
            Some("session-a"),
            1_000,
            200_000,
        );
        assert_eq!(stalled.component, "helper_worker");
        assert!(stalled.action_required);
    }
}

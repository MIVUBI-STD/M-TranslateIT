use crate::engine::runtime_state::latest_runtime_session_state;

use super::helper_bridge;
use super::helper_bridge_runtime::HelperBridgeActionResult;
use super::meeting_session::{self, MeetingSessionActionResult};
use super::runtime_inventory::{self, ModelInventoryReport};
use super::virtual_mic_route::{
    clear_prepared_virtual_mic_route_selection, prepare_current_virtual_mic_route_for_meeting,
};
use super::voice_lab::voice_lab_build_blocks_meeting as my_voice_build_blocks_meeting;

#[tauri::command]
pub fn start_helper_bridge() -> HelperBridgeActionResult {
    let runtime_state = latest_runtime_session_state();
    if runtime_state.has_active_session {
        let helper = helper_bridge::get_helper_bridge_status();
        return HelperBridgeActionResult {
            ok: false,
            state: "active_runtime_session".to_string(),
            message: "Stop Translation or Mic Test before restarting the local translation runtime. The active session was left unchanged."
                .to_string(),
            generation_token: helper.generation_token,
            runtime_claim: "public_helper_restart_deferred_until_runtime_session_stop".to_string(),
        };
    }

    helper_bridge::start_helper_bridge()
}

#[tauri::command]
pub fn verify_required_outbound_ai_readiness() -> HelperBridgeActionResult {
    let runtime_state = latest_runtime_session_state();
    if runtime_state.has_active_session {
        let helper = helper_bridge::get_helper_bridge_status();
        return HelperBridgeActionResult {
            ok: false,
            state: "active_runtime_session".to_string(),
            message:
                "Finish the current Meeting or Mic Test before running the local translation check."
                    .to_string(),
            generation_token: helper.generation_token,
            runtime_claim: "functional_readiness_check_deferred_until_runtime_session_stop"
                .to_string(),
        };
    }

    let mut helper = helper_bridge::get_helper_bridge_status();
    if matches!(helper.state.as_str(), "not_started" | "stopped") {
        let started = helper_bridge::start_helper_bridge();
        if !started.ok {
            return started;
        }
        helper = helper_bridge::get_helper_bridge_status();
    }
    if helper.state != "ready" {
        return HelperBridgeActionResult {
            ok: false,
            state: helper.state,
            message: "The local translator is not available for the final readiness check."
                .to_string(),
            generation_token: helper.generation_token,
            runtime_claim: "functional_readiness_check_helper_not_ready".to_string(),
        };
    }

    if helper.functional_outbound_ready {
        return HelperBridgeActionResult {
            ok: true,
            state: "ready".to_string(),
            message: "The current helper generation already has verified outbound readiness."
                .to_string(),
            generation_token: helper.generation_token,
            runtime_claim:
                "functional_outbound_readiness_reused_current_helper_generation".to_string(),
        };
    }

    if let Err(stage) = helper_bridge::verify_required_outbound_ai_runtime() {
        let current = helper_bridge::get_helper_bridge_status();
        return HelperBridgeActionResult {
            ok: false,
            state: "blocked".to_string(),
            message: format!(
                "The final local translation check could not complete at {stage}. Check Diagnostics and try again."
            ),
            generation_token: current.generation_token,
            runtime_claim: "functional_readiness_check_failed".to_string(),
        };
    }

    let current = helper_bridge::get_helper_bridge_status();
    HelperBridgeActionResult {
        ok: current.functional_outbound_ready,
        state: if current.functional_outbound_ready {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        message: if current.functional_outbound_ready {
            "The final local translation check passed.".to_string()
        } else {
            "The final local translation check did not produce verified readiness.".to_string()
        },
        generation_token: current.generation_token,
        runtime_claim: if current.functional_outbound_ready {
            "functional_outbound_ready_current_helper_generation".to_string()
        } else {
            "functional_outbound_readiness_unverified".to_string()
        },
    }
}

#[tauri::command]
pub fn start_meeting_translation() -> MeetingSessionActionResult {
    if my_voice_build_blocks_meeting() {
        return MeetingSessionActionResult {
            ok: false,
            state: "voice_lab_build_active".to_string(),
            message: "Finish or cancel My Voice creation before starting Translation. The current My Voice build was left unchanged."
                .to_string(),
            status: meeting_session::get_meeting_session_status(),
        };
    }

    // Duplicate Start, runtime-owner conflicts, and unverifiable authority remain
    // owned by the canonical Meeting command. Only a verified-empty state prepares
    // a fresh route pair.
    if latest_runtime_session_state().has_active_session {
        return meeting_session::start_meeting_translation();
    }

    clear_prepared_virtual_mic_route_selection();
    if let Err(blocker) = prepare_current_virtual_mic_route_for_meeting() {
        return MeetingSessionActionResult {
            ok: false,
            state: "meeting_route_pair_prepare_failed".to_string(),
            message: format!(
                "Start Translation needs one matched Windows virtual-audio route pair before it can start. Route detail: {blocker}"
            ),
            status: meeting_session::get_meeting_session_status(),
        };
    }

    let result = meeting_session::start_meeting_translation();
    if !result.ok {
        clear_prepared_virtual_mic_route_selection();
    }
    result
}

#[tauri::command]
pub fn verify_models() -> ModelInventoryReport {
    runtime_inventory::verify_models()
}

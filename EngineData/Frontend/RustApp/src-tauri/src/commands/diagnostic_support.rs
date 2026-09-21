use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::logging::sanitize_diagnostic_text;
use crate::engine::paths::ProjectPaths;

use super::device_loss_guard::get_device_loss_guard_status;
use super::helper_bridge::get_helper_bridge_status;
use super::meeting_session::get_meeting_session_status;
use super::runtime_watchdog::get_runtime_watchdog_status;
use super::startup_recovery::get_startup_recovery_status;
use super::virtual_mic_route::get_virtual_mic_route_selection;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticSupportBundleResult {
    pub ok: bool,
    pub file_path: Option<String>,
    pub message: String,
    pub blocker: String,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn sanitized(value: &str) -> String {
    sanitize_diagnostic_text(value)
}

#[tauri::command]
pub fn export_diagnostic_support_bundle() -> DiagnosticSupportBundleResult {
    let paths = ProjectPaths::discover();
    let meeting = get_meeting_session_status();
    let helper = get_helper_bridge_status();
    let watchdog = get_runtime_watchdog_status();
    let devices = get_device_loss_guard_status();
    let recovery = get_startup_recovery_status();
    let route = get_virtual_mic_route_selection();

    let payload = json!({
        "schema": "translateit.support_bundle.v1",
        "created_unix_ms": unix_ms(),
        "app": {
            "version": env!("CARGO_PKG_VERSION"),
            "path_mode": paths.path_mode,
            "packaged_context_initialized": paths.packaged_context_initialized,
            "development_root_verified": paths.development_root_verified,
        },
        "startup_recovery": {
            "previous_unclean_shutdown": recovery.previous_unclean_shutdown,
            "another_instance_detected": recovery.another_instance_detected,
            "cleanup_attempted": recovery.cleanup_attempted,
            "cleanup_ok": recovery.cleanup_ok,
            "removed_files": recovery.removed_files,
            "blocker": sanitized(&recovery.blocker),
            "note": sanitized(&recovery.note),
            "checked_unix_ms": recovery.checked_unix_ms,
        },
        "watchdog": {
            "state": watchdog.state,
            "healthy": watchdog.healthy,
            "action_required": watchdog.action_required,
            "component": watchdog.component,
            "stage": watchdog.stage,
            "age_ms": watchdog.age_ms,
            "threshold_ms": watchdog.threshold_ms,
            "blocker": sanitized(&watchdog.blocker),
            "note": sanitized(&watchdog.note),
            "updated_unix_ms": watchdog.updated_unix_ms,
        },
        "device_loss": {
            "state": devices.state,
            "healthy": devices.healthy,
            "action_required": devices.action_required,
            "required_device_lost": devices.required_device_lost,
            "optional_device_lost": devices.optional_device_lost,
            "component": devices.component,
            "blocker": sanitized(&devices.blocker),
            "note": sanitized(&devices.note),
            "updated_unix_ms": devices.updated_unix_ms,
        },
        "meeting": {
            "lifecycle": meeting.lifecycle,
            "has_session": meeting.has_session,
            "authority_active": meeting.authority_active,
            "active_age_ms": meeting.active_age_ms,
            "capture_active": meeting.capture_active,
            "blocker": sanitized(&meeting.blocker),
            "note": sanitized(&meeting.note),
            "outbound": {
                "stage": meeting.outbound.stage,
                "output_active": meeting.outbound.output_active,
                "last_stage_ok": meeting.outbound.last_stage_ok,
                "overflow_dropped_utterance_count": meeting.outbound.overflow_dropped_utterance_count,
                "evicted_pending_utterance_count": meeting.outbound.evicted_pending_utterance_count,
                "blocker": sanitized(&meeting.outbound.blocker),
                "updated_unix_ms": meeting.outbound.updated_unix_ms,
            },
            "incoming": {
                "stage": meeting.incoming.stage,
                "capture_active": meeting.incoming.capture_active,
                "suppressed": meeting.incoming.suppressed,
                "degraded": meeting.incoming.degraded,
                "blocker": sanitized(&meeting.incoming.blocker),
                "updated_unix_ms": meeting.incoming.updated_unix_ms,
            },
        },
        "helper": {
            "state": helper.state,
            "cuda_ready": helper.cuda_ready,
            "provider_ready": helper.provider_ready,
            "functional_outbound_ready": helper.functional_outbound_ready,
            "degraded_mode": helper.degraded_mode,
            "active_task": helper.active_task,
            "active_meeting_lane": helper.active_meeting_lane,
            "generation_token": helper.generation_token,
            "last_error": helper.last_error.as_deref().map(sanitized),
            "message": sanitized(&helper.message),
            "updated_unix_ms": helper.updated_unix_ms,
        },
        "virtual_route": {
            "route_ready": route.route_ready,
            "output_device_found": route.output_device_found,
            "input_device_found": route.input_device_found,
            "blocker": sanitized(&route.blocker),
            "runtime_claim": route.runtime_claim,
            "updated_unix_ms": route.updated_unix_ms,
        },
        "privacy": {
            "contains_transcript": false,
            "contains_audio": false,
            "contains_voice_reference": false,
            "contains_absolute_paths": false,
            "contains_device_names": false,
            "contains_session_id": false,
        }
    });

    let directory = PathBuf::from(&paths.user_saved_dir).join("Diagnostics");
    if fs::create_dir_all(&directory).is_err() {
        return DiagnosticSupportBundleResult {
            ok: false,
            file_path: None,
            message: "Diagnostics folder could not be created.".to_string(),
            blocker: "diagnostic_bundle:create_dir_failed".to_string(),
        };
    }
    let path = directory.join(format!("translateit_support_{}.json", unix_ms()));
    let body = match serde_json::to_vec_pretty(&payload) {
        Ok(body) => body,
        Err(_) => {
            return DiagnosticSupportBundleResult {
                ok: false,
                file_path: None,
                message: "Diagnostic support data could not be encoded.".to_string(),
                blocker: "diagnostic_bundle:encode_failed".to_string(),
            }
        }
    };
    if fs::write(&path, body).is_err() {
        return DiagnosticSupportBundleResult {
            ok: false,
            file_path: None,
            message: "Diagnostic support bundle could not be written.".to_string(),
            blocker: "diagnostic_bundle:write_failed".to_string(),
        };
    }

    DiagnosticSupportBundleResult {
        ok: true,
        file_path: Some(path.to_string_lossy().replace('\\', "/")),
        message: "Redacted diagnostic support bundle exported. It does not include transcript, audio, voice reference, device names, session identifiers, or absolute runtime paths.".to_string(),
        blocker: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::sanitized;

    #[test]
    fn bundle_text_uses_runtime_redaction() {
        let clean = sanitized(r"failed C:\Users\alice\trace.log alice@example.com token=secret");
        assert!(!clean.contains("alice@example.com"));
        assert!(!clean.contains("secret"));
        assert!(!clean.contains("C:\\Users\\alice"));
    }
}

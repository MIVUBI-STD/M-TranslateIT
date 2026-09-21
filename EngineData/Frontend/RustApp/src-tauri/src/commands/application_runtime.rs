use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Emitter;

use super::{audio, helper_bridge, meeting_session, mic_test, runtime, settings};

static APPLICATION_REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationProblem {
    pub code: String,
    pub domain: String,
    pub severity: String,
    pub recoverable: bool,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationCapabilities {
    pub meeting_translation: bool,
    pub mic_test: bool,
    pub text_translation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationSnapshot {
    pub revision: u64,
    pub lifecycle: String,
    pub active_owner: Option<String>,
    pub settings: Value,
    pub meeting: Value,
    pub helper: Value,
    pub worker: Value,
    pub input: Value,
    pub capabilities: ApplicationCapabilities,
    pub problems: Vec<ApplicationProblem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationIntentResult {
    pub ok: bool,
    pub intent: String,
    pub state: String,
    pub message: String,
    pub snapshot: ApplicationSnapshot,
}

fn json_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn problem(
    code: impl Into<String>,
    domain: &str,
    severity: &str,
    recoverable: bool,
    action: &str,
    message: impl Into<String>,
) -> ApplicationProblem {
    ApplicationProblem {
        code: code.into(),
        domain: domain.to_string(),
        severity: severity.to_string(),
        recoverable,
        action: action.to_string(),
        message: message.into(),
    }
}

fn derive_lifecycle(meeting: &Value, helper: &Value, input: &Value) -> String {
    let meeting_lifecycle = json_string(meeting, "lifecycle");
    if json_bool(meeting, "has_session") {
        return match meeting_lifecycle.as_str() {
            "live" | "listening" => "meeting_live".to_string(),
            "starting" => "meeting_starting".to_string(),
            "stopping" => "meeting_stopping".to_string(),
            "cleanup_incomplete" => "recovering".to_string(),
            _ => format!("meeting_{meeting_lifecycle}"),
        };
    }

    let helper_state = json_string(helper, "state");
    let input_ready = json_bool(input, "ready") || json_bool(input, "prepared");
    if helper_state == "ready" && input_ready {
        "ready".to_string()
    } else if helper_state == "frontend_bridge_error" || helper_state == "error" {
        "degraded".to_string()
    } else {
        "setup_required".to_string()
    }
}

fn collect_problems(meeting: &Value, helper: &Value, input: &Value) -> Vec<ApplicationProblem> {
    let mut problems = Vec::new();

    let meeting_blocker = json_string(meeting, "blocker");
    if !meeting_blocker.is_empty() {
        problems.push(problem(
            meeting_blocker,
            "meeting",
            "blocking",
            true,
            "check_meeting",
            json_string(meeting, "note"),
        ));
    }

    if let Some(blockers) = meeting
        .get("preflight")
        .and_then(|value| value.get("blockers"))
        .and_then(Value::as_array)
    {
        for blocker in blockers {
            let code = blocker.as_str().unwrap_or_default().trim();
            if !code.is_empty() && !problems.iter().any(|item| item.code == code) {
                problems.push(problem(
                    code,
                    "meeting",
                    "blocking",
                    true,
                    "fix_setup",
                    "Meeting translation still needs attention before it can start.",
                ));
            }
        }
    }

    let helper_state = json_string(helper, "state");
    if !helper_state.is_empty() && helper_state != "ready" {
        problems.push(problem(
            format!("helper:{helper_state}"),
            "worker",
            if helper_state == "error" { "blocking" } else { "warning" },
            true,
            "restart_helper",
            json_string(helper, "message"),
        ));
    }

    let input_blocker = json_string(input, "blocker");
    if !input_blocker.is_empty() {
        problems.push(problem(
            input_blocker,
            "audio",
            "blocking",
            true,
            "select_microphone",
            json_string(input, "note"),
        ));
    }

    problems
}

pub fn current_application_snapshot() -> ApplicationSnapshot {
    let settings_value = serde_json::to_value(settings::load_runtime_settings()).unwrap_or_else(|_| json!({}));
    let meeting_status = meeting_session::get_meeting_session_status();
    let meeting_value = serde_json::to_value(&meeting_status).unwrap_or_else(|_| json!({}));
    let helper_status = helper_bridge::get_helper_bridge_status();
    let helper_value = serde_json::to_value(&helper_status).unwrap_or_else(|_| json!({}));
    let input_status = audio::get_input_status();
    let input_value = serde_json::to_value(&input_status).unwrap_or_else(|_| json!({}));

    let helper_ready = json_string(&helper_value, "state") == "ready";
    let worker_value = if helper_ready {
        serde_json::to_value(helper_bridge::helper_bridge_worker_status()).unwrap_or_else(|_| json!({}))
    } else {
        Value::Null
    };

    let meeting_has_session = meeting_status.has_session;
    let application_owned = meeting_status.owner_id.as_deref() == Some("translateit_application_meeting");
    let meeting_ready = meeting_status.preflight.ready_for_start || meeting_has_session;
    let input_ready = json_bool(&input_value, "ready") || json_bool(&input_value, "prepared");
    let lifecycle = derive_lifecycle(&meeting_value, &helper_value, &input_value);
    let problems = collect_problems(&meeting_value, &helper_value, &input_value);

    ApplicationSnapshot {
        revision: APPLICATION_REVISION.fetch_add(1, Ordering::AcqRel) + 1,
        lifecycle,
        active_owner: meeting_status.owner_id.clone(),
        settings: settings_value,
        meeting: meeting_value,
        helper: helper_value,
        worker: worker_value,
        input: input_value,
        capabilities: ApplicationCapabilities {
            meeting_translation: meeting_ready && application_owned || (!meeting_has_session && meeting_ready),
            mic_test: !meeting_has_session && input_ready,
            text_translation: helper_ready,
        },
        problems,
    }
}

fn emit_application_snapshot(app: &tauri::AppHandle, reason: &str, snapshot: &ApplicationSnapshot) {
    let _ = app.emit(
        "translateit://application-runtime",
        json!({
            "reason": reason,
            "snapshot": snapshot,
        }),
    );
}

#[tauri::command]
pub fn get_application_snapshot() -> ApplicationSnapshot {
    current_application_snapshot()
}

#[tauri::command]
pub fn dispatch_product_intent(app: tauri::AppHandle, intent: String) -> ApplicationIntentResult {
    let normalized = intent.trim().to_ascii_lowercase();

    let (ok, state, message) = match normalized.as_str() {
        "start_meeting" => {
            let result = runtime::start_meeting_translation();
            (result.ok, result.state, result.message)
        }
        "stop_meeting" => {
            let result = meeting_session::stop_meeting_translation();
            (result.ok, result.state, result.message)
        }
        "start_mic_test" => {
            let result = mic_test::start_capture();
            (result.ok, result.state.to_string(), result.message)
        }
        "stop_mic_test" => {
            let result = mic_test::stop_capture();
            (result.ok, result.state.to_string(), result.message)
        }
        "fix_setup" => {
            let helper = helper_bridge::get_helper_bridge_status();
            if matches!(helper.state.as_str(), "not_started" | "stopped") {
                let started = helper_bridge::start_helper_bridge();
                if !started.ok {
                    (false, started.state, started.message)
                } else {
                    let readiness = runtime::verify_required_outbound_ai_readiness();
                    (readiness.ok, readiness.state, readiness.message)
                }
            } else if helper.state == "ready" {
                let readiness = runtime::verify_required_outbound_ai_readiness();
                (readiness.ok, readiness.state, readiness.message)
            } else {
                (
                    false,
                    "helper_unavailable".to_string(),
                    "Setup still needs attention because the local helper is unavailable.".to_string(),
                )
            }
        }
        "refresh" => (
            true,
            "refreshed".to_string(),
            "Application runtime snapshot refreshed.".to_string(),
        ),
        _ => (
            false,
            "unsupported_intent".to_string(),
            format!("Unsupported product intent: {normalized}"),
        ),
    };

    let snapshot = current_application_snapshot();
    emit_application_snapshot(&app, &normalized, &snapshot);

    ApplicationIntentResult {
        ok,
        intent: normalized,
        state,
        message,
        snapshot,
    }
}

#[cfg(test)]
mod tests {
    use super::derive_lifecycle;
    use serde_json::json;
    
    #[test]
    fn lifecycle_prefers_active_meeting_truth() {
        let meeting = json!({"has_session": true, "lifecycle": "live"});
        let helper = json!({"state": "ready"});
        let input = json!({"ready": true});
        assert_eq!(derive_lifecycle(&meeting, &helper, &input), "meeting_live");
    }

    #[test]
    fn lifecycle_reports_ready_only_when_core_dependencies_are_ready() {
        let meeting = json!({"has_session": false, "lifecycle": "idle"});
        let helper = json!({"state": "ready"});
        let input = json!({"ready": true});
        assert_eq!(derive_lifecycle(&meeting, &helper, &input), "ready");
    }
}

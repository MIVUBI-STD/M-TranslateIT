use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::engine::runtime_state::{
    latest_runtime_session_state, runtime_generation_is_authoritative,
};

use super::super::helper_bridge_runtime::HelperTaskPriority;
use super::clean_helper_text;

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

static MEETING_OUTBOUND_PIPELINE_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn live_outbound_generation_is_authoritative(generation: u64) -> bool {
    if !runtime_generation_is_authoritative(generation) {
        return false;
    }
    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| {
            snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
                && snapshot.generation == generation
                && snapshot.authority_active
                && snapshot.phase == "live"
        })
        .unwrap_or(false)
}

pub(super) fn live_outbound_stage_retry_safe(task: &str) -> bool {
    matches!(task, "transcribe" | "translate")
}

pub(super) fn meeting_generation(payload: &Value) -> Option<u64> {
    payload.get("meeting_generation").and_then(Value::as_u64)
}

pub(super) fn meeting_session_id(payload: &Value) -> Option<String> {
    payload
        .get("meeting_session_id")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 96))
        .filter(|value| !value.is_empty())
}

pub(super) fn meeting_lane(payload: &Value) -> Option<String> {
    payload
        .get("meeting_lane")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 32).to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "you" | "incoming"))
}

pub(super) fn meeting_start_prepare(payload: &Value) -> bool {
    payload
        .get("meeting_start_prepare")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn meeting_outbound_pipeline_active() -> bool {
    MEETING_OUTBOUND_PIPELINE_GENERATION.load(Ordering::Acquire) != 0
}

pub(super) fn mark_meeting_outbound_pipeline(generation: u64) {
    if generation != 0 {
        MEETING_OUTBOUND_PIPELINE_GENERATION.store(generation, Ordering::Release);
    }
}

pub(super) fn clear_meeting_outbound_pipeline(generation: u64) {
    let _ = MEETING_OUTBOUND_PIPELINE_GENERATION.compare_exchange(
        generation,
        0,
        Ordering::AcqRel,
        Ordering::Acquire,
    );
}

pub(super) fn clear_any_meeting_outbound_pipeline() {
    MEETING_OUTBOUND_PIPELINE_GENERATION.store(0, Ordering::Release);
}

pub(super) fn incoming_session_is_eligible(session_id: &str) -> bool {
    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| {
            snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
                && snapshot.session_id == session_id
                && snapshot.authority_active
                && snapshot.phase == "live"
        })
        .unwrap_or(false)
}

pub(super) fn task_priority(task: &str, payload: &Value) -> HelperTaskPriority {
    if meeting_generation(payload).is_some() || meeting_start_prepare(payload) {
        HelperTaskPriority::MeetingOutbound
    } else if meeting_lane(payload).as_deref() == Some("incoming")
        && meeting_session_id(payload).is_some()
    {
        HelperTaskPriority::MeetingIncoming
    } else if task == "translate" {
        HelperTaskPriority::Text
    } else {
        HelperTaskPriority::Diagnostic
    }
}

pub(super) fn inject_request_metadata(
    payload: &mut Value,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
) {
    if !payload.is_object() {
        *payload = json!({});
    }
    if let Some(object) = payload.as_object_mut() {
        object.insert("command".to_string(), json!(task));
        object.insert("request_id".to_string(), json!(request_id));
        object.insert("scheduler_priority".to_string(), json!(priority.label()));
    }
}

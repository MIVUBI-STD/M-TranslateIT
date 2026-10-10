use serde_json::{json, Value};

use crate::engine::runtime_state::{
    begin_application_meeting_session, clear_runtime_session_if_generation,
    clear_runtime_session_state, commit_application_meeting_session_live,
};

use super::helper_bridge::{
    cancel_helper_bridge_meeting_session, send_helper_worker_task, HelperBridgeWorkerResponse,
};
use super::helper_bridge_runtime::{
    clear_active_request, runtime, set_blocked, stop_child, validate_worker_response_contract,
    HelperBridgeRuntime,
};

fn reset_bridge_runtime() {
    if let Ok(mut bridge) = runtime().lock() {
        stop_child(&mut bridge);
        *bridge = HelperBridgeRuntime::default();
    }
}

fn response_blocker(response: &HelperBridgeWorkerResponse) -> String {
    serde_json::from_str::<Value>(&response.worker_response_json)
        .ok()
        .and_then(|value| {
            value
                .get("blocker")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

#[test]
fn bridge_priority_and_stale_guards_follow_production_payload_contract() {
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();

    let text = send_helper_worker_task(
        "translate",
        json!({
            "text": "halo",
            "source_language": "id",
            "target_language": "en"
        }),
    );
    assert_eq!(text.scheduler_priority, "text");
    assert_eq!(response_blocker(&text), "helper_bridge:not_running");

    reset_bridge_runtime();
    let diagnostic = send_helper_worker_task("status", json!({}));
    assert_eq!(diagnostic.scheduler_priority, "diagnostic");
    assert_eq!(response_blocker(&diagnostic), "helper_bridge:not_running");

    reset_bridge_runtime();
    let incomplete_incoming = send_helper_worker_task(
        "translate",
        json!({
            "text": "hello",
            "meeting_lane": "incoming"
        }),
    );
    assert_eq!(
        incomplete_incoming.scheduler_priority, "text",
        "incoming lane without a Meeting session id must not gain MeetingIncoming priority"
    );

    reset_bridge_runtime();
    let start_prepare = send_helper_worker_task(
        "status",
        json!({
            "meeting_start_prepare": true
        }),
    );
    assert_eq!(start_prepare.scheduler_priority, "meeting_outbound");
    assert_eq!(response_blocker(&start_prepare), "helper_bridge:not_running");

    let meeting = begin_application_meeting_session();
    assert!(meeting.blocker.is_empty());
    let snapshot = meeting.snapshot.as_ref().expect("Meeting authority claim");
    let generation = snapshot.generation;
    let session_id = snapshot.session_id.clone();

    reset_bridge_runtime();
    let outbound = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": "unused-test.wav",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "you",
            "meeting_generation": generation
        }),
    );
    assert_eq!(outbound.scheduler_priority, "meeting_outbound");
    assert_eq!(response_blocker(&outbound), "helper_bridge:not_running");

    let live = commit_application_meeting_session_live(
        generation,
        false,
        "bridge contract test commits Meeting Live without opening capture",
    );
    assert!(live.blocker.is_empty());

    reset_bridge_runtime();
    let incoming = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": "unused-test.wav",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "INCOMING"
        }),
    );
    assert_eq!(incoming.scheduler_priority, "meeting_incoming");
    assert_eq!(response_blocker(&incoming), "helper_bridge:not_running");

    let cleared = clear_runtime_session_if_generation(generation);
    assert_eq!(cleared.blocker, "runtime_session:cleared");

    reset_bridge_runtime();
    let stale_outbound = send_helper_worker_task(
        "translate",
        json!({
            "text": "halo",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "you",
            "meeting_generation": generation
        }),
    );
    assert_eq!(stale_outbound.scheduler_priority, "meeting_outbound");
    assert_eq!(stale_outbound.state, "stale_generation");
    assert_eq!(
        response_blocker(&stale_outbound),
        "helper_scheduler:meeting_generation_not_authoritative"
    );
    assert_eq!(
        stale_outbound.runtime_claim,
        "helper_scheduler_request_not_executed"
    );

    reset_bridge_runtime();
    let stale_incoming = send_helper_worker_task(
        "translate",
        json!({
            "text": "hello",
            "meeting_session_id": session_id,
            "meeting_lane": "incoming"
        }),
    );
    assert_eq!(stale_incoming.scheduler_priority, "meeting_incoming");
    assert_eq!(stale_incoming.state, "stale_session");
    assert_eq!(
        response_blocker(&stale_incoming),
        "helper_scheduler:incoming_meeting_session_not_eligible"
    );
    assert_eq!(
        stale_incoming.runtime_claim,
        "helper_scheduler_request_not_executed"
    );

    reset_bridge_runtime();
    let _ = clear_runtime_session_state();
}

#[test]
fn request_cleanup_preserves_replacement_and_clears_exact_identity() {
    let mut bridge = HelperBridgeRuntime::default();
    bridge.active_task = Some("translate".to_string());
    bridge.active_request_id = Some("request-new".to_string());
    bridge.active_meeting_generation = Some(44);
    bridge.active_meeting_session_id = Some("session-new".to_string());
    bridge.active_meeting_lane = Some("you".to_string());

    clear_active_request(&mut bridge, "request-old");
    assert_eq!(bridge.active_task.as_deref(), Some("translate"));
    assert_eq!(bridge.active_request_id.as_deref(), Some("request-new"));
    assert_eq!(bridge.active_meeting_generation, Some(44));
    assert_eq!(
        bridge.active_meeting_session_id.as_deref(),
        Some("session-new")
    );
    assert_eq!(bridge.active_meeting_lane.as_deref(), Some("you"));

    clear_active_request(&mut bridge, "request-new");
    assert!(bridge.active_task.is_none());
    assert!(bridge.active_request_id.is_none());
    assert!(bridge.active_meeting_generation.is_none());
    assert!(bridge.active_meeting_session_id.is_none());
    assert!(bridge.active_meeting_lane.is_none());
}

#[test]
fn helper_hard_cancel_is_session_scoped_and_cleans_matching_metadata() {
    reset_bridge_runtime();
    {
        let mut bridge = runtime().lock().expect("helper bridge test lock");
        bridge.state = "ready".to_string();
        bridge.generation_token = 41;
        bridge.active_task = Some("translate".to_string());
        bridge.active_request_id = Some("request-live".to_string());
        bridge.active_meeting_generation = Some(77);
        bridge.active_meeting_session_id = Some("session-live".to_string());
        bridge.active_meeting_lane = Some("you".to_string());
    }

    let stale_cancel = cancel_helper_bridge_meeting_session("session-stale");
    assert!(stale_cancel.ok);
    {
        let bridge = runtime().lock().expect("helper bridge test lock");
        assert_eq!(bridge.generation_token, 41);
        assert_eq!(bridge.active_task.as_deref(), Some("translate"));
        assert_eq!(bridge.active_request_id.as_deref(), Some("request-live"));
        assert_eq!(bridge.active_meeting_generation, Some(77));
        assert_eq!(
            bridge.active_meeting_session_id.as_deref(),
            Some("session-live")
        );
        assert_eq!(bridge.active_meeting_lane.as_deref(), Some("you"));
    }

    let matching_cancel = cancel_helper_bridge_meeting_session("session-live");
    assert!(matching_cancel.ok);
    assert_eq!(matching_cancel.state, "stopped");
    {
        let bridge = runtime().lock().expect("helper bridge test lock");
        assert_eq!(bridge.generation_token, 42);
        assert!(bridge.active_task.is_none());
        assert!(bridge.active_request_id.is_none());
        assert!(bridge.active_meeting_generation.is_none());
        assert!(bridge.active_meeting_session_id.is_none());
        assert!(bridge.active_meeting_lane.is_none());
        assert_eq!(
            bridge.last_error.as_deref(),
            Some("helper_bridge:meeting_session_hard_cancelled")
        );
    }
    reset_bridge_runtime();
}

#[test]
fn blocked_bridge_state_clears_all_active_request_metadata() {
    let mut bridge = HelperBridgeRuntime::default();
    bridge.active_task = Some("transcribe".to_string());
    bridge.active_request_id = Some("request-active".to_string());
    bridge.active_meeting_generation = Some(9);
    bridge.active_meeting_session_id = Some("session-active".to_string());
    bridge.active_meeting_lane = Some("incoming".to_string());

    let blocked = set_blocked(
        &mut bridge,
        "test helper bridge blocked state",
        "helper_bridge:test_blocked",
    );
    assert!(!blocked.ok);
    assert_eq!(blocked.state, "blocked");
    assert!(bridge.active_task.is_none());
    assert!(bridge.active_request_id.is_none());
    assert!(bridge.active_meeting_generation.is_none());
    assert!(bridge.active_meeting_session_id.is_none());
    assert!(bridge.active_meeting_lane.is_none());
    assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:test_blocked"));
}

#[test]
fn worker_response_identity_and_stage_match_the_requested_task() {
    assert!(validate_worker_response_contract(
        "ping",
        "helper-17",
        &json!({"ok": true, "stage": "ping", "request_id": "helper-17"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "status",
        "helper-18",
        &json!({"ok": true, "stage": "local_realtime_worker_preflight", "request_id": "helper-18"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "translate",
        "helper-19",
        &json!({"ok": false, "stage": "worker_error", "request_id": "helper-19"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "status",
        "helper-20",
        &json!({"ok": false, "stage": "status", "request_id": "helper-20"}),
    ).is_ok());
}

#[test]
fn worker_response_mismatched_identity_and_stage_fail_closed() {
    for response in [
        json!({"ok": true, "stage": "ping"}),
        json!({"ok": true, "stage": "ping", "request_id": "helper-old"}),
    ] {
        assert_eq!(
            validate_worker_response_contract("ping", "helper-current", &response),
            Err("worker:response_request_id_mismatch".to_string()),
        );
    }
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"ok": true, "stage": "translate", "request_id": "helper-current"}),
        ),
        Err("worker:response_stage_mismatch".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"ok": true, "stage": "worker_error", "request_id": "helper-current"}),
        ),
        Err("worker:response_stage_mismatch".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"stage": "ping", "request_id": "helper-current"}),
        ),
        Err("worker:response_ok_must_be_bool".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!(["not", "an", "object"]),
        ),
        Err("worker:response_must_be_object".to_string()),
    );
}

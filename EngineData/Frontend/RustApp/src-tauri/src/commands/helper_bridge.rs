use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::commands::diagnostic_trace::{
    trace_command_end, trace_command_error, trace_command_start,
};
use crate::engine::runtime_state::{
    latest_runtime_session_state, runtime_generation_is_authoritative,
};

use super::bridge_paths::{
    helper_stderr_log_path, resolve_worker_python_command, slash_path,
    worker_python_unavailable_message, worker_root, worker_script,
};
use super::helper_bridge_runtime::{
    acquire_helper_task_permit, action_result, apply_worker_response, apply_worker_status,
    clear_active_request, read_worker_response_direct_with_deadline, runtime, set_blocked,
    spawn_stderr_logger, status_from_runtime, stop_child, unix_ms,
    worker_response_deadline_for_priority, worker_response_deadline_ms,
    write_worker_request_with_deadline, HelperBridgeActionResult, HelperBridgeStatus,
    HelperTaskPriority,
};

mod functional_readiness;

use functional_readiness::{
    decorate_functional_readiness_status, failed_required_outbound_task_invalidates_cache,
    functional_asr_output, functional_translation_output, functional_voice_actor_output_path,
    invalidate_required_outbound_ai_readiness, remember_required_outbound_functional_readiness,
    worker_response_value,
};
pub use functional_readiness::required_outbound_voice_actor_token;

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

static MEETING_OUTBOUND_PIPELINE_GENERATION: AtomicU64 = AtomicU64::new(0);

const REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE: &str = "selamat pagi";
const REQUIRED_OUTBOUND_FUNCTIONAL_VOICE_OUTPUT: &str =
    "UserData/CacheData/helper_functional_readiness/required_outbound_myvoice.wav";
const REQUIRED_OUTBOUND_DIAGNOSTIC_VOICE_OUTPUT: &str =
    "UserData/CacheData/helper_functional_readiness/diagnostic_myvoice.wav";

#[derive(Debug, Clone, Serialize)]
pub struct HelperBridgeWorkerResponse {
    pub ok: bool,
    pub state: String,
    pub task: String,
    pub request_id: String,
    pub scheduler_priority: String,
    pub message: String,
    pub generation_token: u64,
    pub runtime_claim: String,
    pub worker_response_json: String,
}

fn clean_helper_text(value: &str, max_chars: usize) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            *character != '\0'
                && !('\u{0001}'..='\u{0008}').contains(character)
                && !('\u{000b}'..='\u{001f}').contains(character)
                && *character != '\u{007f}'
        })
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

fn worker_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|text| clean_helper_text(text, 500))
        .filter(|text| !text.is_empty())
}

fn worker_message(task: &str, response: &Value) -> String {
    worker_text(response, "note")
        .or_else(|| worker_text(response, "blocker"))
        .or_else(|| worker_text(response, "stage"))
        .unwrap_or_else(|| format!("Helper worker task {task} completed."))
}

fn helper_transport_failure(response: &HelperBridgeWorkerResponse) -> bool {
    if response.ok {
        return false;
    }
    let value =
        serde_json::from_str::<Value>(&response.worker_response_json).unwrap_or_else(|_| json!({}));
    let blocker = worker_text(&value, "blocker").unwrap_or_default();
    blocker.starts_with("helper_bridge:")
        && (blocker.contains("_write_failed:") || blocker.contains("_read_failed:"))
}

fn live_outbound_generation_is_authoritative(generation: u64) -> bool {
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

fn live_outbound_stage_retry_safe(task: &str) -> bool {
    matches!(task, "transcribe" | "translate")
}

fn meeting_generation(payload: &Value) -> Option<u64> {
    payload.get("meeting_generation").and_then(Value::as_u64)
}

fn meeting_session_id(payload: &Value) -> Option<String> {
    payload
        .get("meeting_session_id")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 96))
        .filter(|value| !value.is_empty())
}

fn meeting_lane(payload: &Value) -> Option<String> {
    payload
        .get("meeting_lane")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 32).to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "you" | "incoming"))
}

fn meeting_start_prepare(payload: &Value) -> bool {
    payload
        .get("meeting_start_prepare")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn meeting_outbound_pipeline_active() -> bool {
    MEETING_OUTBOUND_PIPELINE_GENERATION.load(Ordering::Acquire) != 0
}

fn mark_meeting_outbound_pipeline(generation: u64) {
    if generation != 0 {
        MEETING_OUTBOUND_PIPELINE_GENERATION.store(generation, Ordering::Release);
    }
}

fn clear_meeting_outbound_pipeline(generation: u64) {
    let _ = MEETING_OUTBOUND_PIPELINE_GENERATION.compare_exchange(
        generation,
        0,
        Ordering::AcqRel,
        Ordering::Acquire,
    );
}

fn clear_any_meeting_outbound_pipeline() {
    MEETING_OUTBOUND_PIPELINE_GENERATION.store(0, Ordering::Release);
}

fn incoming_session_is_eligible(session_id: &str) -> bool {
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

fn task_priority(task: &str, payload: &Value) -> HelperTaskPriority {
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

fn inject_request_metadata(
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

fn response_with_runtime(
    ok: bool,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: String,
    worker_response: Value,
    runtime: &super::helper_bridge_runtime::HelperBridgeRuntime,
) -> HelperBridgeWorkerResponse {
    HelperBridgeWorkerResponse {
        ok,
        state: runtime.state.clone(),
        task: task.to_string(),
        request_id: request_id.to_string(),
        scheduler_priority: priority.label().to_string(),
        message,
        generation_token: runtime.generation_token,
        runtime_claim: status_from_runtime(runtime).runtime_claim,
        worker_response_json: worker_response.to_string(),
    }
}

fn standalone_blocked_response(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    state: &str,
    blocker: &str,
    message: &str,
) -> HelperBridgeWorkerResponse {
    HelperBridgeWorkerResponse {
        ok: false,
        state: state.to_string(),
        task: task.to_string(),
        request_id: request_id.to_string(),
        scheduler_priority: priority.label().to_string(),
        message: message.to_string(),
        generation_token: 0,
        runtime_claim: "helper_scheduler_request_not_executed".to_string(),
        worker_response_json: json!({
            "ok": false,
            "stage": task,
            "request_id": request_id,
            "scheduler_priority": priority.label(),
            "blocker": blocker,
            "note": message,
        })
        .to_string(),
    }
}

fn incoming_deferred_response(task: &str, request_id: &str) -> HelperBridgeWorkerResponse {
    standalone_blocked_response(
        task,
        request_id,
        HelperTaskPriority::MeetingIncoming,
        "deferred",
        "helper_scheduler:incoming_deferred_for_outbound",
        "Optional incoming Meeting work yielded before execution because required outbound translation currently owns the helper pipeline.",
    )
}

fn blocked_response_from_runtime(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: &str,
    runtime: &super::helper_bridge_runtime::HelperBridgeRuntime,
) -> HelperBridgeWorkerResponse {
    response_with_runtime(
        false,
        task,
        request_id,
        priority,
        message.to_string(),
        json!({
            "ok": false,
            "stage": task,
            "request_id": request_id,
            "scheduler_priority": priority.label(),
            "blocker": runtime
                .last_error
                .clone()
                .unwrap_or_else(|| "helper_bridge:not_ready".to_string()),
            "note": message,
        }),
        runtime,
    )
}

fn stale_meeting_request(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    generation: Option<u64>,
    session_id: Option<&str>,
    lane: Option<&str>,
) -> Option<HelperBridgeWorkerResponse> {
    if generation
        .map(|value| !runtime_generation_is_authoritative(value))
        .unwrap_or(false)
    {
        return Some(standalone_blocked_response(
            task,
            request_id,
            priority,
            "stale_generation",
            "helper_scheduler:meeting_generation_not_authoritative",
            "Queued outbound Meeting work was discarded because its generation no longer owns output authority.",
        ));
    }

    if lane == Some("incoming")
        && session_id
            .map(|value| !incoming_session_is_eligible(value))
            .unwrap_or(true)
    {
        return Some(standalone_blocked_response(
            task,
            request_id,
            priority,
            "stale_session",
            "helper_scheduler:incoming_meeting_session_not_eligible",
            "Queued incoming Meeting work was discarded because its application Meeting session is no longer eligible for incoming promotion.",
        ));
    }
    None
}

fn recover_incoming_transport_failure_before_permit_release(
    session_id: Option<&str>,
    mut response: HelperBridgeWorkerResponse,
) -> HelperBridgeWorkerResponse {
    if !helper_transport_failure(&response) {
        return response;
    }
    if session_id
        .filter(|value| incoming_session_is_eligible(value))
        .is_none()
    {
        return response;
    }

    // This runs while the failed MeetingIncoming request still owns the scheduler
    // permit. Restoring the one shared worker before that permit is released prevents
    // a concurrently waiting required outbound request from observing the worker as
    // stopped. The stale incoming event itself is never retried.
    let recovery = start_helper_bridge_internal(false);
    response.state = recovery.state.clone();
    response.generation_token = recovery.generation_token;
    if recovery.ok {
        response.runtime_claim =
            "meeting_incoming_transport_recovered_same_worker_event_not_retried".to_string();
        response.message = format!(
            "{} The same helper worker was restored before releasing incoming scheduler ownership; this stale incoming event was not retried.",
            response.message
        );
    } else {
        response.runtime_claim = "meeting_incoming_transport_recovery_failed".to_string();
        response.message = format!(
            "{} The helper worker could not be restored after the optional incoming transport failure: {}",
            response.message, recovery.message
        );
    }
    response
}

fn send_worker_task_inner(task: &str, mut payload: Value) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let meeting_generation = meeting_generation(&payload);
    let meeting_session_id = meeting_session_id(&payload);
    let meeting_lane = meeting_lane(&payload);
    let permit = match acquire_helper_task_permit(priority) {
        Ok(permit) => permit,
        Err(error) => {
            return standalone_blocked_response(
                task,
                "scheduler-unavailable",
                priority,
                "error",
                &error,
                "Helper scheduler could not admit the request.",
            )
        }
    };
    let request_id = permit.request_id().to_string();
    let response_deadline_ms = worker_response_deadline_for_priority(task, priority);

    // An incoming request may have entered the scheduler before outbound claimed the
    // pipeline. Re-check after permit acquisition so queued optional work cannot slip
    // between required outbound ASR -> translation -> TTS stages.
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, &request_id);
    }

    if let Some(response) = stale_meeting_request(
        task,
        &request_id,
        priority,
        meeting_generation,
        meeting_session_id.as_deref(),
        meeting_lane.as_deref(),
    ) {
        return response;
    }

    inject_request_metadata(&mut payload, task, &request_id, priority);

    let (mut stdin, stdout, bridge_generation) = match runtime().lock() {
        Ok(mut runtime) => {
            if runtime.child.is_none() || runtime.stdin.is_none() || runtime.stdout.is_none() {
                runtime.state = "stopped".to_string();
                runtime.message = "Helper worker is not running. Start Helper first.".to_string();
                runtime.last_error = Some("helper_bridge:not_running".to_string());
                runtime.updated_unix_ms = unix_ms();
                return blocked_response_from_runtime(
                    task,
                    &request_id,
                    priority,
                    "Helper worker is not running. Start Helper first.",
                    &runtime,
                );
            }

            let stdin = runtime.stdin.take().expect("stdin checked above");
            let stdout = runtime.stdout.take().expect("stdout checked above");
            let bridge_generation = runtime.generation_token;
            runtime.active_task = Some(task.to_string());
            runtime.active_request_id = Some(request_id.clone());
            runtime.active_meeting_generation = meeting_generation;
            runtime.active_meeting_session_id = meeting_session_id.clone();
            runtime.active_meeting_lane = meeting_lane.clone();
            runtime.updated_unix_ms = unix_ms();
            (stdin, stdout, bridge_generation)
        }
        Err(_) => {
            return standalone_blocked_response(
                task,
                &request_id,
                priority,
                "error",
                "helper_bridge:lock_poisoned",
                "Helper bridge state lock is poisoned.",
            )
        }
    };

    if let Err(error) =
        write_worker_request_with_deadline(&mut stdin, &payload, response_deadline_ms)
    {
        let response = match runtime().lock() {
            Ok(mut runtime) => {
                if runtime.generation_token == bridge_generation {
                    stop_child(&mut runtime);
                    runtime.generation_token = runtime.generation_token.saturating_add(1);
                    runtime.state = "stopped".to_string();
                    runtime.message =
                        format!("Failed to write {task} request to Python helper worker: {error}");
                    runtime.last_error = Some(format!("helper_bridge:{task}_write_failed:{error}"));
                    clear_active_request(&mut runtime, &request_id);
                    runtime.provider_ready = false;
                    runtime.cuda_ready = false;
                    runtime.updated_unix_ms = unix_ms();
                }
                let message = runtime.message.clone();
                blocked_response_from_runtime(task, &request_id, priority, &message, &runtime)
            }
            Err(_) => standalone_blocked_response(
                task,
                &request_id,
                priority,
                "error",
                "helper_bridge:lock_poisoned_after_write_failure",
                "Helper request write failed and bridge state could not be recovered.",
            ),
        };
        return if priority == HelperTaskPriority::MeetingIncoming {
            recover_incoming_transport_failure_before_permit_release(
                meeting_session_id.as_deref(),
                response,
            )
        } else {
            response
        };
    }

    let (mut worker_response, stdout) = match read_worker_response_direct_with_deadline(
        stdout,
        response_deadline_ms,
    ) {
        Ok(value) => value,
        Err(error) => {
            let response = match runtime().lock() {
                Ok(mut runtime) => {
                    if runtime.generation_token == bridge_generation {
                        stop_child(&mut runtime);
                        runtime.generation_token = runtime.generation_token.saturating_add(1);
                        runtime.state = "stopped".to_string();
                        runtime.message = format!(
                                "Failed to read {task} response from Python helper worker before deadline: {error}"
                            );
                        runtime.last_error =
                            Some(format!("helper_bridge:{task}_read_failed:{error}"));
                        clear_active_request(&mut runtime, &request_id);
                        runtime.provider_ready = false;
                        runtime.cuda_ready = false;
                        runtime.updated_unix_ms = unix_ms();
                    }
                    let message = runtime.message.clone();
                    blocked_response_from_runtime(task, &request_id, priority, &message, &runtime)
                }
                Err(_) => standalone_blocked_response(
                    task,
                    &request_id,
                    priority,
                    "error",
                    "helper_bridge:lock_poisoned_after_read_failure",
                    "Helper response failed and bridge state could not be recovered safely.",
                ),
            };
            return if priority == HelperTaskPriority::MeetingIncoming {
                recover_incoming_transport_failure_before_permit_release(
                    meeting_session_id.as_deref(),
                    response,
                )
            } else {
                response
            };
        }
    };

    if let Some(object) = worker_response.as_object_mut() {
        object.insert("request_id".to_string(), json!(request_id));
        object.insert("scheduler_priority".to_string(), json!(priority.label()));
    }

    match runtime().lock() {
        Ok(mut runtime) => {
            let bridge_still_owns_process =
                runtime.generation_token == bridge_generation && runtime.child.is_some();
            if !bridge_still_owns_process {
                clear_active_request(&mut runtime, &request_id);
                return blocked_response_from_runtime(
                    task,
                    &request_id,
                    priority,
                    "Helper result was discarded because the worker process was cancelled or replaced while the request was running.",
                    &runtime,
                );
            }

            runtime.stdin = Some(stdin);
            runtime.stdout = Some(stdout);

            if let Some(response) = stale_meeting_request(
                task,
                &request_id,
                priority,
                meeting_generation,
                meeting_session_id.as_deref(),
                meeting_lane.as_deref(),
            ) {
                clear_active_request(&mut runtime, &request_id);
                runtime.message = response.message.clone();
                runtime.updated_unix_ms = unix_ms();
                return response_with_runtime(
                    false,
                    task,
                    &request_id,
                    priority,
                    response.message,
                    serde_json::from_str(&response.worker_response_json)
                        .unwrap_or_else(|_| json!({})),
                    &runtime,
                );
            }

            let ok = apply_worker_response(&mut runtime, &worker_response);
            clear_active_request(&mut runtime, &request_id);
            runtime.updated_unix_ms = unix_ms();
            let message = worker_message(task, &worker_response);
            response_with_runtime(
                ok,
                task,
                &request_id,
                priority,
                message,
                worker_response,
                &runtime,
            )
        }
        Err(_) => standalone_blocked_response(
            task,
            &request_id,
            priority,
            "error",
            "helper_bridge:lock_poisoned_after_worker_response",
            "Worker response was received, but bridge state could not be updated safely.",
        ),
    }
}

fn recover_live_meeting_helper_transport(
    generation: u64,
    retain_outbound_pipeline: bool,
) -> HelperBridgeActionResult {
    if !live_outbound_generation_is_authoritative(generation) {
        return HelperBridgeActionResult {
            ok: false,
            state: "stale_generation".to_string(),
            message: "Live Meeting helper recovery was skipped because the outbound generation no longer owns Live authority."
                .to_string(),
            generation_token: get_helper_bridge_status().generation_token,
            runtime_claim: "meeting_live_helper_recovery_skipped_stale_generation".to_string(),
        };
    }

    let _permit = match acquire_helper_task_permit(HelperTaskPriority::MeetingOutbound) {
        Ok(permit) => permit,
        Err(error) => {
            return HelperBridgeActionResult {
                ok: false,
                state: "error".to_string(),
                message: format!(
                    "Live Meeting helper recovery could not acquire outbound scheduler priority: {error}"
                ),
                generation_token: get_helper_bridge_status().generation_token,
                runtime_claim: "meeting_live_helper_recovery_scheduler_unavailable".to_string(),
            }
        }
    };

    if !live_outbound_generation_is_authoritative(generation) {
        return HelperBridgeActionResult {
            ok: false,
            state: "stale_generation".to_string(),
            message: "Live Meeting helper recovery stopped before restart because the outbound generation was revoked while waiting for the scheduler."
                .to_string(),
            generation_token: get_helper_bridge_status().generation_token,
            runtime_claim: "meeting_live_helper_recovery_skipped_after_scheduler_wait".to_string(),
        };
    }

    let mut recovery = start_helper_bridge_internal(false);
    if recovery.ok && live_outbound_generation_is_authoritative(generation) {
        if retain_outbound_pipeline {
            mark_meeting_outbound_pipeline(generation);
        }
        recovery.message = "The same canonical helper worker was restarted after a Live Meeting transport failure."
            .to_string();
        recovery.runtime_claim = "meeting_live_helper_transport_recovered_same_worker".to_string();
        return recovery;
    }

    clear_meeting_outbound_pipeline(generation);
    if recovery.ok {
        recovery.ok = false;
        recovery.state = "stale_generation".to_string();
        recovery.message = "The helper worker restarted, but the Meeting generation was revoked before the failed stage could be retried."
            .to_string();
        recovery.runtime_claim =
            "meeting_live_helper_recovery_completed_after_generation_revoke".to_string();
    }
    recovery
}

fn send_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let outbound_generation = if priority == HelperTaskPriority::MeetingOutbound
        && meeting_lane(&payload).as_deref() == Some("you")
    {
        meeting_generation(&payload)
    } else {
        None
    };
    let retry_payload = payload.clone();

    // Reject new optional incoming stages immediately while a required outbound
    // utterance owns the helper pipeline. The post-permit check in the inner path
    // also catches incoming work that was already queued before this claim existed.
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, "incoming-deferred-before-scheduler");
    }

    if let Some(generation) = outbound_generation {
        mark_meeting_outbound_pipeline(generation);
    }

    let mut response = send_worker_task_inner(task, payload);

    if let Some(generation) = outbound_generation {
        if helper_transport_failure(&response)
            && live_outbound_generation_is_authoritative(generation)
        {
            let retry_current_stage = live_outbound_stage_retry_safe(task);
            let recovery = recover_live_meeting_helper_transport(generation, retry_current_stage);
            if recovery.ok && live_outbound_generation_is_authoritative(generation) {
                if retry_current_stage {
                    // ASR and translation have no Meeting playback side effect, so the
                    // same finalized input may be attempted exactly once after a
                    // transport-only worker restart. The retry itself is not recursive.
                    response = send_worker_task_inner(task, retry_payload);
                } else {
                    // Synthesis can have uncertain child-process/file state after a
                    // transport failure. Restore the worker for the next utterance but
                    // do not synthesize the current phrase again automatically.
                    response.state = recovery.state;
                    response.generation_token = recovery.generation_token;
                    response.runtime_claim =
                        "meeting_live_helper_recovered_current_stage_not_retried".to_string();
                    response.message = format!(
                        "{} The helper worker was restored for subsequent Meeting utterances; this synthesis stage was not retried.",
                        response.message
                    );
                }
            }
        }

        if task == "voice_actor_synthesize"
            || !response.ok
            || !runtime_generation_is_authoritative(generation)
        {
            clear_meeting_outbound_pipeline(generation);
        }
    }

    if failed_required_outbound_task_invalidates_cache(task, priority, &response) {
        invalidate_required_outbound_ai_readiness();
    }
    response
}

pub fn send_helper_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task(task, payload)
}

fn run_required_outbound_ai_probe(
    meeting_generation: Option<u64>,
    output_path: &str,
) -> Result<(u64, String), &'static str> {
    if meeting_generation
        .map(|generation| generation == 0 || !runtime_generation_is_authoritative(generation))
        .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("Meeting authority");
    }
    let meeting_start_prepare = meeting_generation.is_some();

    // Refresh cheap capability truth first so a newly approved/rebuilt My Voice is
    // visible to explicit setup checks and to the generation-bound Meeting probe.
    let refreshed = send_worker_task(
        "status",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !refreshed.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let helper = get_helper_bridge_status();
    if helper.state != "ready" || !helper.provider_ready || helper.generation_token == 0 {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let generation_token = helper.generation_token;

    let asr = send_worker_task(
        "asr_preload",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !asr.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    let translation = send_worker_task(
        "translate",
        json!({
            "text": REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE,
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 24,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(translated_fixture) = functional_translation_output(&translation) else {
        invalidate_required_outbound_ai_readiness();
        return Err("Indonesian to English translation");
    };

    let actor_preflight = send_worker_task(
        "voice_actor_preflight",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !actor_preflight.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let Some(actor_token) = worker_text(&worker_response_value(&actor_preflight), "actor_token") else {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let voice = send_worker_task(
        "voice_actor_synthesize",
        json!({
            "text": translated_fixture,
            "output_path": output_path,
            "expected_actor_token": actor_token.clone(),
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(functional_voice_path) = functional_voice_actor_output_path(&voice) else {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let asr_inference = send_worker_task(
        "transcribe",
        json!({
            "audio_path": functional_voice_path,
            "language": "en",
            "beam_size": 1,
            "vad_filter": false,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let functional_voice_path = worker_response_value(&voice)
        .get("output_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(path) = functional_voice_path.as_deref() {
        let _ = fs::remove_file(path);
    }
    if !functional_asr_output(&asr_inference) {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    let status = send_worker_task(
        "status",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !status.ok
        || worker_text(&worker_response_value(&status), "voice_actor_token").as_deref()
            != Some(actor_token.as_str())
    {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let current = get_helper_bridge_status();
    if current.state != "ready"
        || !current.provider_ready
        || current.generation_token != generation_token
        || meeting_generation
            .map(|generation| !runtime_generation_is_authoritative(generation))
            .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }

    Ok((generation_token, actor_token))
}

pub fn verify_required_outbound_ai_runtime() -> Result<(), &'static str> {
    let (generation_token, actor_token) = run_required_outbound_ai_probe(
        None,
        REQUIRED_OUTBOUND_DIAGNOSTIC_VOICE_OUTPUT,
    )?;
    remember_required_outbound_functional_readiness(generation_token, 0, actor_token);
    Ok(())
}

pub fn prepare_required_outbound_ai_runtime(meeting_generation: u64) -> Result<(), &'static str> {
    let (generation_token, actor_token) = run_required_outbound_ai_probe(
        Some(meeting_generation),
        REQUIRED_OUTBOUND_FUNCTIONAL_VOICE_OUTPUT,
    )?;
    if !runtime_generation_is_authoritative(meeting_generation) {
        invalidate_required_outbound_ai_readiness();
        return Err("Meeting authority");
    }
    remember_required_outbound_functional_readiness(
        generation_token,
        meeting_generation,
        actor_token,
    );
    Ok(())
}

#[tauri::command]
pub fn get_helper_bridge_status() -> HelperBridgeStatus {
    let started = trace_command_start("get_helper_bridge_status", "reading helper bridge status");
    match runtime().lock() {
        Ok(mut runtime) => {
            let child_exited = runtime
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
                .is_some();
            if child_exited {
                invalidate_required_outbound_ai_readiness();
                stop_child(&mut runtime);
                runtime.state = "stopped".to_string();
                runtime.message = "Helper worker process exited.".to_string();
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.active_task = None;
                runtime.active_request_id = None;
                runtime.active_meeting_generation = None;
                runtime.active_meeting_session_id = None;
                runtime.active_meeting_lane = None;
                runtime.updated_unix_ms = unix_ms();
            }
            let result = decorate_functional_readiness_status(status_from_runtime(&runtime));
            trace_command_end(
                "get_helper_bridge_status",
                started,
                format!("state={}", result.state),
            );
            result
        }
        Err(_) => {
            let result = HelperBridgeStatus {
                state: "error".to_string(),
                message: "Helper bridge status lock is poisoned.".to_string(),
                cuda_ready: false,
                provider_ready: false,
                functional_outbound_ready: false,
                functional_outbound_verified_unix_ms: None,
                degraded_mode: false,
                active_task: None,
                active_request_id: None,
                active_meeting_generation: None,
                active_meeting_session_id: None,
                active_meeting_lane: None,
                generation_token: 0,
                last_error: Some("helper_bridge:lock_poisoned".to_string()),
                stderr_log_path: None,
                updated_unix_ms: unix_ms(),
                runtime_claim: "bridge_state_error".to_string(),
            };
            trace_command_error(
                "get_helper_bridge_status",
                started,
                format!("state={}", result.state),
            );
            result
        }
    }
}

fn start_helper_bridge_internal(clear_outbound_pipeline: bool) -> HelperBridgeActionResult {
    if clear_outbound_pipeline {
        clear_any_meeting_outbound_pipeline();
    }
    match runtime().lock() {
        Ok(mut runtime) => {
            runtime.generation_token = runtime.generation_token.saturating_add(1);
            invalidate_required_outbound_ai_readiness();
            stop_child(&mut runtime);
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;

            let worker = worker_script();
            if !worker.is_file() {
                return set_blocked(&mut runtime, "Missing realtime worker script. Restore EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py before starting the helper bridge.", "helper_bridge:worker_script_missing");
            }

            let Some(python) = resolve_worker_python_command() else {
                let message = worker_python_unavailable_message();
                return set_blocked(
                    &mut runtime,
                    &message,
                    "helper_bridge:python_runtime_missing",
                );
            };

            runtime.state = "starting".to_string();
            runtime.message = format!("Starting Python helper worker using {}.", python.source);
            runtime.updated_unix_ms = unix_ms();

            let mut command = Command::new(&python.program);
            command.args(&python.bootstrap_args);
            command.arg(&worker);
            command.current_dir(worker_root());
            command.stdin(Stdio::piped());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    return set_blocked(
                        &mut runtime,
                        &format!(
                            "Failed to spawn Python helper worker using {}: {error}",
                            python.source
                        ),
                        "helper_bridge:spawn_failed",
                    )
                }
            };

            let stderr_log_path = helper_stderr_log_path();
            if let Some(stderr) = child.stderr.take() {
                runtime.stderr_logger = Some(spawn_stderr_logger(stderr, stderr_log_path.clone()));
                runtime.stderr_log_path = Some(slash_path(&stderr_log_path));
            }

            let mut stdin = match child.stdin.take() {
                Some(stdin) => stdin,
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    stop_child(&mut runtime);
                    return set_blocked(
                        &mut runtime,
                        "Python helper stdin was not available after spawn.",
                        "helper_bridge:stdin_missing",
                    );
                }
            };
            let stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    stop_child(&mut runtime);
                    return set_blocked(
                        &mut runtime,
                        "Python helper stdout was not available after spawn.",
                        "helper_bridge:stdout_missing",
                    );
                }
            };
            let stdout = BufReader::new(stdout);
            runtime.child = Some(child);

            let ping_deadline_ms = worker_response_deadline_ms("ping");
            if let Err(error) = write_worker_request_with_deadline(
                &mut stdin,
                &json!({ "command": "ping" }),
                ping_deadline_ms,
            ) {
                stop_child(&mut runtime);
                return set_blocked(
                    &mut runtime,
                    &format!("Failed to send ping to helper worker: {error}"),
                    "helper_bridge:ping_write_failed",
                );
            }
            let (ping, mut stdout) =
                match read_worker_response_direct_with_deadline(stdout, ping_deadline_ms) {
                    Ok((value, stdout)) => (value, stdout),
                    Err(error) => {
                        stop_child(&mut runtime);
                        return set_blocked(
                            &mut runtime,
                            &format!(
                            "Failed to read helper worker ping response before deadline: {error}"
                        ),
                            "helper_bridge:ping_read_failed",
                        );
                    }
                };
            if ping.get("ok").and_then(Value::as_bool) != Some(true) {
                stop_child(&mut runtime);
                return set_blocked(
                    &mut runtime,
                    "Helper worker ping returned a non-ready response.",
                    "helper_bridge:ping_not_ok",
                );
            }

            let status_deadline_ms = worker_response_deadline_ms("status");
            let status = if write_worker_request_with_deadline(
                &mut stdin,
                &json!({ "command": "status" }),
                status_deadline_ms,
            )
            .is_ok()
            {
                match read_worker_response_direct_with_deadline(stdout, status_deadline_ms) {
                    Ok((value, next_stdout)) => {
                        stdout = next_stdout;
                        Some(value)
                    }
                    Err(error) => {
                        stop_child(&mut runtime);
                        return set_blocked(
                            &mut runtime,
                            &format!("Failed to read helper worker status response before deadline: {error}"),
                            "helper_bridge:status_read_failed",
                        );
                    }
                }
            } else {
                None
            };

            runtime.stdin = Some(stdin);
            runtime.stdout = Some(stdout);
            runtime.state = "ready".to_string();
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;
            runtime.last_error = None;
            runtime.updated_unix_ms = unix_ms();
            if let Some(status) = status {
                apply_worker_status(&mut runtime, &status);
            } else {
                runtime.message = format!("Python helper worker is running via {} and ping verified. Worker capability status was not available yet.", python.source);
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
            }
            action_result(true, &runtime)
        }
        Err(_) => HelperBridgeActionResult {
            ok: false,
            state: "error".to_string(),
            message: "Helper bridge start failed because state lock is poisoned.".to_string(),
            generation_token: 0,
            runtime_claim: "bridge_state_error".to_string(),
        },
    }
}

pub fn start_helper_bridge() -> HelperBridgeActionResult {
    start_helper_bridge_internal(true)
}

pub fn cancel_helper_bridge_meeting_session(session_id: &str) -> HelperBridgeActionResult {
    clear_any_meeting_outbound_pipeline();
    let session_id = session_id.trim();
    match runtime().lock() {
        Ok(mut runtime) => {
            if !session_id.is_empty()
                && runtime.active_meeting_session_id.as_deref() == Some(session_id)
            {
                runtime.generation_token = runtime.generation_token.saturating_add(1);
                invalidate_required_outbound_ai_readiness();
                stop_child(&mut runtime);
                runtime.state = "stopped".to_string();
                runtime.message = format!(
                    "In-flight helper inference for Meeting session {session_id} was hard-cancelled during full Meeting Stop."
                );
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
                runtime.active_task = None;
                runtime.active_request_id = None;
                runtime.active_meeting_generation = None;
                runtime.active_meeting_session_id = None;
                runtime.active_meeting_lane = None;
                runtime.last_error =
                    Some("helper_bridge:meeting_session_hard_cancelled".to_string());
            } else {
                runtime.message = format!(
                    "No in-flight helper task belongs to Meeting session {session_id}. Queued incoming/outbound work will be rejected by session/generation guards."
                );
            }
            runtime.updated_unix_ms = unix_ms();
            action_result(true, &runtime)
        }
        Err(_) => HelperBridgeActionResult {
            ok: false,
            state: "error".to_string(),
            message: "Meeting helper session cancellation failed because state lock is poisoned."
                .to_string(),
            generation_token: 0,
            runtime_claim: "bridge_state_error".to_string(),
        },
    }
}

#[tauri::command]
pub fn helper_bridge_worker_status() -> HelperBridgeWorkerResponse {
    send_worker_task("status", json!({}))
}

#[cfg(test)]
mod c4_functional_readiness_tests {
    use super::{
        failed_required_outbound_task_invalidates_cache, functional_asr_output,
        functional_translation_output, helper_transport_failure, live_outbound_stage_retry_safe,
        HelperBridgeWorkerResponse, HelperTaskPriority,
    };

    fn response(ok: bool, body: &str) -> HelperBridgeWorkerResponse {
        HelperBridgeWorkerResponse {
            ok,
            state: "ready".to_string(),
            task: "test".to_string(),
            request_id: "test-request".to_string(),
            scheduler_priority: "meeting_outbound".to_string(),
            message: "test".to_string(),
            generation_token: 7,
            runtime_claim: "test".to_string(),
            worker_response_json: body.to_string(),
        }
    }

    #[test]
    fn functional_translation_requires_id_en_complete_eos_output() {
        let complete = response(
            true,
            r#"{"ok":true,"direction_pair":"id->en","complete":true,"finished_with_eos":true,"translated_text":"good morning"}"#,
        );
        assert_eq!(
            functional_translation_output(&complete).as_deref(),
            Some("good morning")
        );

        let incomplete = response(
            true,
            r#"{"ok":true,"direction_pair":"id->en","complete":false,"finished_with_eos":false,"translated_text":"partial"}"#,
        );
        assert!(functional_translation_output(&incomplete).is_none());
    }

    #[test]
    fn functional_asr_requires_real_nonempty_transcribe_output() {
        let complete = response(
            true,
            r#"{"ok":true,"stage":"transcribe","transcript_text":"good morning"}"#,
        );
        assert!(functional_asr_output(&complete));

        let empty = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:empty_transcript"}"#,
        );
        assert!(!functional_asr_output(&empty));
    }

    #[test]
    fn normal_empty_asr_does_not_invalidate_functional_capability() {
        let empty = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:empty_transcript"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            &empty
        ));

        let hard_tts = response(
            false,
            r#"{"ok":false,"stage":"voice_actor_synthesize","blocker":"voice_actor:actor_changed_since_meeting_start"}"#,
        );
        assert!(failed_required_outbound_task_invalidates_cache(
            "voice_actor_synthesize",
            HelperTaskPriority::MeetingOutbound,
            &hard_tts
        ));
    }

    #[test]
    fn optional_incoming_failures_cannot_invalidate_required_outbound_capability() {
        let hard_asr = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingIncoming,
            &hard_asr
        ));
        assert!(failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            &hard_asr
        ));

        let hard_translation = response(
            false,
            r#"{"ok":false,"stage":"translate","blocker":"translation:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "translate",
            HelperTaskPriority::MeetingIncoming,
            &hard_translation
        ));
        assert!(failed_required_outbound_task_invalidates_cache(
            "translate",
            HelperTaskPriority::MeetingOutbound,
            &hard_translation
        ));

        let optional_preload = response(
            false,
            r#"{"ok":false,"stage":"translation_preload","direction_pair":"en->id","blocker":"translation:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "translation_preload",
            HelperTaskPriority::Diagnostic,
            &optional_preload
        ));
        let outbound_preload = response(
            false,
            r#"{"ok":false,"stage":"translation_preload","direction_pair":"id->en","blocker":"translation:provider_failed"}"#,
        );
        assert!(failed_required_outbound_task_invalidates_cache(
            "translation_preload",
            HelperTaskPriority::Diagnostic,
            &outbound_preload
        ));
    }

    #[test]
    fn transport_failure_and_retry_policy_remain_bounded() {
        let write_failure = response(
            false,
            r#"{"ok":false,"blocker":"helper_bridge:translate_write_failed:broken_pipe"}"#,
        );
        assert!(helper_transport_failure(&write_failure));
        let read_failure = response(
            false,
            r#"{"ok":false,"blocker":"helper_bridge:transcribe_read_failed:response_deadline"}"#,
        );
        assert!(helper_transport_failure(&read_failure));
        let model_failure = response(
            false,
            r#"{"ok":false,"blocker":"translation:provider_failed"}"#,
        );
        assert!(!helper_transport_failure(&model_failure));

        assert!(live_outbound_stage_retry_safe("transcribe"));
        assert!(live_outbound_stage_retry_safe("translate"));
        assert!(!live_outbound_stage_retry_safe("voice_actor_synthesize"));
        assert!(!live_outbound_stage_retry_safe("status"));
    }

}

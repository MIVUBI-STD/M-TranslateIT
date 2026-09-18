use serde_json::{json, Value};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    failed_required_outbound_task_invalidates_cache, invalidate_required_outbound_ai_readiness,
};
use super::request_policy::{
    clear_meeting_outbound_pipeline, incoming_session_is_eligible, inject_request_metadata,
    live_outbound_generation_is_authoritative, live_outbound_stage_retry_safe,
    mark_meeting_outbound_pipeline, meeting_generation, meeting_lane,
    meeting_outbound_pipeline_active, meeting_session_id, task_priority,
};
use super::{
    get_helper_bridge_status, prepare_required_outbound_ai_runtime, start_helper_bridge_internal,
    worker_message, worker_text, HelperBridgeWorkerResponse,
};
use super::super::helper_bridge_runtime::{
    acquire_helper_task_permit, apply_worker_response, clear_active_request,
    read_worker_response_direct_with_deadline, runtime, status_from_runtime, stop_child, unix_ms,
    worker_response_deadline_for_priority, write_worker_request_with_deadline,
    HelperBridgeActionResult, HelperBridgeRuntime, HelperTaskPriority,
};

pub(super) fn helper_transport_failure(response: &HelperBridgeWorkerResponse) -> bool {
    if response.ok {
        return false;
    }
    let value =
        serde_json::from_str::<Value>(&response.worker_response_json).unwrap_or_else(|_| json!({}));
    let blocker = worker_text(&value, "blocker").unwrap_or_default();
    blocker.starts_with("helper_bridge:")
        && (blocker.contains("_write_failed:") || blocker.contains("_read_failed:"))
}

fn response_with_runtime(
    ok: bool,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: String,
    worker_response: Value,
    runtime: &HelperBridgeRuntime,
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
    runtime: &HelperBridgeRuntime,
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

fn retain_payload_for_live_retry(
    task: &str,
    priority: HelperTaskPriority,
    outbound_generation: Option<u64>,
) -> bool {
    priority == HelperTaskPriority::MeetingOutbound
        && outbound_generation.is_some()
        && live_outbound_stage_retry_safe(task)
}

pub(super) fn send_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let outbound_generation = if priority == HelperTaskPriority::MeetingOutbound
        && meeting_lane(&payload).as_deref() == Some("you")
    {
        meeting_generation(&payload)
    } else {
        None
    };
    let mut retry_payload = retain_payload_for_live_retry(task, priority, outbound_generation)
        .then(|| payload.clone());

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
                // Restarting the worker intentionally invalidates functional readiness
                // and the generation-bound actor token. Re-prove the exact required
                // outbound path before retrying a side-effect-safe stage or claiming
                // the recovered worker is usable for later utterances.
                match prepare_required_outbound_ai_runtime(generation) {
                    Ok(()) if retry_current_stage => {
                        // Only retry-safe Live outbound stages retain a payload clone.
                        // ASR and translation have no Meeting playback side effect, so
                        // the same finalized input may be attempted exactly once after
                        // transport recovery + generation-bound functional re-proof.
                        // The retry itself is not recursive.
                        if let Some(retry_payload) = retry_payload.take() {
                            response = send_worker_task_inner(task, retry_payload);
                        } else {
                            clear_meeting_outbound_pipeline(generation);
                            response.ok = false;
                            response.state = "error".to_string();
                            response.runtime_claim =
                                "meeting_live_helper_retry_payload_contract_missing".to_string();
                            response.message =
                                "Live Meeting helper recovery completed, but the retry-safe payload contract was unavailable."
                                    .to_string();
                        }
                    }
                    Ok(()) => {
                        // Synthesis can have uncertain child-process/file state after a
                        // transport failure. The worker and actor binding are restored
                        // for subsequent utterances, but this phrase is not synthesized
                        // again automatically.
                        response.state = recovery.state;
                        response.generation_token = recovery.generation_token;
                        response.runtime_claim =
                            "meeting_live_helper_recovered_current_stage_not_retried".to_string();
                        response.message = format!(
                            "{} The helper worker and Meeting voice authority were restored for subsequent utterances; this synthesis stage was not retried.",
                            response.message
                        );
                    }
                    Err(stage) => {
                        clear_meeting_outbound_pipeline(generation);
                        let current = get_helper_bridge_status();
                        response.state = current.state;
                        response.generation_token = current.generation_token;
                        response.runtime_claim =
                            "meeting_live_helper_recovery_functional_reproof_failed".to_string();
                        response.message = format!(
                            "{} The helper process restarted, but required outbound functional re-proof failed at {stage}. Stop and start Translation before producing more voice output.",
                            response.message
                        );
                    }
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

#[cfg(test)]
mod request_payload_retention_tests {
    use super::retain_payload_for_live_retry;
    use super::super::super::helper_bridge_runtime::HelperTaskPriority;

    #[test]
    fn only_retry_safe_live_outbound_stages_retain_payload_clone() {
        assert!(retain_payload_for_live_retry(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(retain_payload_for_live_retry(
            "translate",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));

        assert!(!retain_payload_for_live_retry(
            "voice_actor_synthesize",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(!retain_payload_for_live_retry(
            "status",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(!retain_payload_for_live_retry(
            "translate",
            HelperTaskPriority::Text,
            None,
        ));
        assert!(!retain_payload_for_live_retry(
            "transcribe",
            HelperTaskPriority::MeetingIncoming,
            None,
        ));
    }
}

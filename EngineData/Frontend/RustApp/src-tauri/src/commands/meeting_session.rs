use serde::Serialize;
use serde_json::{json, Value};

use crate::engine::audio::finalized_utterance::{
    clear_finalized_meeting_sequence, reset_finalized_meeting_sequence,
};
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::audio::meeting_output::{
    cancel_meeting_output_for_generation, clear_prepared_meeting_output_device,
    prepare_meeting_output_device, probe_prepared_meeting_output_device_functionally,
};
use crate::engine::audio::meeting_sound_capture::{
    start_meeting_sound_capture_runtime,
    stop_meeting_sound_capture_runtime,
};
use crate::engine::runtime_state::{
    begin_application_meeting_session, clear_runtime_session_if_generation,
    clear_runtime_session_state, commit_application_meeting_session_live,
    latest_runtime_session_state, mark_runtime_session_cleanup_incomplete,
    revoke_runtime_session_authority, runtime_generation_is_authoritative,
};

use super::helper_bridge::{
    cancel_helper_bridge_meeting_session, get_helper_bridge_status,
    prepare_required_outbound_ai_runtime, required_outbound_voice_actor_token,
    send_helper_worker_task, start_helper_bridge, HelperBridgeWorkerResponse,
};

mod committed_turns;
mod consumer_runtime;
mod incoming_activation;
mod incoming_deferred;
mod incoming_pipeline;
mod outbound_pipeline;
mod preflight;
mod session_state;
mod suppression;

use committed_turns::{
    clear_all_committed_turns, clear_committed_turns_for_session,
    current_committed_turn_snapshot, interrupt_committed_turns_for_generation,
    reset_committed_turns,
};
use consumer_runtime::{
    start_meeting_incoming_consumer, start_meeting_outbound_consumer,
    stop_meeting_incoming_consumer, stop_meeting_outbound_consumer,
};
use incoming_activation::schedule_optional_incoming_lane;
use incoming_deferred::clear_deferred_incoming_queue;
use outbound_pipeline::process_outbound_wav;
use preflight::{blocked_result, build_preflight, current_status, status_from_report};
use suppression::{
    begin_self_output_suppression, clear_self_output_suppression_for_session,
    disable_optional_incoming_for_outbound, reset_self_output_suppression,
    suppression_handle_for_session,
};
use session_state::{
    clear_all_start_preflight, clear_incoming_status, clear_outbound_status,
    clear_start_preflight_for_generation, current_incoming_status, current_outbound_status,
    current_start_preflight, elapsed_millis, incoming_lane_enabled,
    mark_incoming_cleanup_incomplete_status, record_first_playback_timing,
    remember_start_preflight, set_outbound_timing, update_incoming_status,
    update_outbound_status,
    OutboundTimingContext,
};

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSessionPreflightStatus {
    pub ready_for_start: bool,
    pub start_eligible: bool,
    pub functional_outbound_ready: bool,
    pub functional_outbound_verified_unix_ms: Option<u128>,
    pub microphone_ready: bool,
    pub models_ready: bool,
    pub helper_ready: bool,
    pub provider_ready: bool,
    pub meeting_route_ready: bool,
    pub generation_aware_outbound_stages_ready: bool,
    pub finalized_utterance_source_connected: bool,
    pub outbound_runtime_connected: bool,
    pub blockers: Vec<String>,
    pub summary: String,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingOutboundTiming {
    pub finalized_unix_ms: u128,
    pub first_playback_unix_ms: Option<u128>,
    pub speech_boundary_ms: u64,
    pub finalization_ms: u64,
    pub queue_ms: u64,
    pub audio_prepare_ms: u64,
    pub asr_ms: Option<u64>,
    pub translation_ms: Option<u64>,
    pub tts_ms: Option<u64>,
    pub delivery_ms: Option<u64>,
    pub outbound_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingOutboundRuntimeStatus {
    pub generation: Option<u64>,
    pub session_id: Option<String>,
    pub stage: String,
    pub utterance_sequence: u64,
    pub output_active: bool,
    pub last_stage_ok: bool,
    pub timing: Option<MeetingOutboundTiming>,
    pub overflow_dropped_utterance_count: u64,
    pub evicted_pending_utterance_count: u64,
    pub blocker: String,
    pub note: String,
    pub updated_unix_ms: u128,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingIncomingRuntimeStatus {
    pub session_id: Option<String>,
    pub stage: String,
    pub capture_active: bool,
    pub suppressed: bool,
    pub degraded: bool,
    pub blocker: String,
    pub note: String,
    pub updated_unix_ms: u128,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSessionStatus {
    pub lifecycle: String,
    pub has_session: bool,
    pub authority_active: bool,
    pub session_id: Option<String>,
    pub generation: Option<u64>,
    pub started_unix_ms: Option<u128>,
    pub active_age_ms: Option<u128>,
    pub capture_active: bool,
    pub owner_id: Option<String>,
    pub blocker: String,
    pub note: String,
    pub preflight: MeetingSessionPreflightStatus,
    pub outbound: MeetingOutboundRuntimeStatus,
    pub incoming: MeetingIncomingRuntimeStatus,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSessionActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub status: MeetingSessionStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingOutboundProcessResult {
    pub ok: bool,
    pub delivered: bool,
    pub state: String,
    pub blocker: String,
    pub note: String,
    pub generation: u64,
    pub utterance_sequence: u64,
    pub runtime_claim: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingCommittedTurn {
    pub session_id: String,
    pub sequence: u64,
    pub generation: Option<u64>,
    pub utterance_id: u64,
    pub lane: String,
    pub source_text: String,
    pub translated_text: String,
    pub delivery_state: Option<String>,
    pub outbound_timing: Option<MeetingOutboundTiming>,
    pub created_unix_ms: u128,
    pub updated_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingCommittedTurnsSnapshot {
    pub ok: bool,
    pub has_session: bool,
    pub session_id: Option<String>,
    pub turns: Vec<MeetingCommittedTurn>,
    pub dropped_turn_count: u64,
    pub truncated: bool,
    pub blocker: String,
    pub note: String,
    pub runtime_claim: String,
}

fn recover_helper_after_meeting_stop_if_needed() -> Result<(), String> {
    let helper = get_helper_bridge_status();
    if helper.state != "stopped"
        || helper.last_error.as_deref() != Some("helper_bridge:meeting_session_hard_cancelled")
    {
        return Ok(());
    }

    // Only the intentional Meeting Stop hard-cancel state is auto-recovered here.
    // Missing runtime/model/provider failures and generic helper stops remain explicit
    // blockers rather than being hidden behind a broad retry loop.
    let recovery = start_helper_bridge();
    if recovery.ok {
        Ok(())
    } else {
        Err(recovery.message)
    }
}

fn worker_json(response: &HelperBridgeWorkerResponse) -> Value {
    serde_json::from_str::<Value>(&response.worker_response_json).unwrap_or_else(|_| json!({}))
}

fn worker_text(response: &HelperBridgeWorkerResponse, key: &str) -> Option<String> {
    let value = worker_json(response);
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn worker_blocker(response: &HelperBridgeWorkerResponse, fallback: &str) -> String {
    worker_text(response, "blocker").unwrap_or_else(|| fallback.to_string())
}

fn generation_is_live(generation: u64) -> bool {
    if !runtime_generation_is_authoritative(generation) {
        return false;
    }
    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| snapshot.generation == generation && snapshot.phase == "live")
        .unwrap_or(false)
}

fn generation_is_starting(generation: u64) -> bool {
    if !runtime_generation_is_authoritative(generation) {
        return false;
    }
    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| snapshot.generation == generation && snapshot.phase == "starting")
        .unwrap_or(false)
}

fn incoming_session_is_eligible(session_id: &str) -> bool {
    if !incoming_lane_enabled(session_id) {
        return false;
    }

    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| {
            snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
                && snapshot.session_id == session_id
                && snapshot.phase == "live"
        })
        .unwrap_or(false)
}

fn process_authoritative_finalized_outbound_wav(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    audio_path: String,
    timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    process_outbound_wav(
        generation,
        session_id,
        event_sequence,
        utterance_id,
        audio_path,
        timing,
    )
}





#[tauri::command]
pub fn get_meeting_session_status() -> MeetingSessionStatus {
    current_status()
}

#[tauri::command]
pub fn get_meeting_committed_turns() -> MeetingCommittedTurnsSnapshot {
    current_committed_turn_snapshot()
}

pub fn start_meeting_translation() -> MeetingSessionActionResult {
    let current = latest_runtime_session_state();
    if current.has_active_session && current.snapshot.is_none() {
        return MeetingSessionActionResult {
            ok: false,
            state: "runtime_state_unavailable".to_string(),
            message: "Start Translation cannot verify current runtime ownership. No new Meeting resources were opened."
                .to_string(),
            status: status_from_report(current, build_preflight()),
        };
    }
    if let Some(snapshot) = current.snapshot.as_ref() {
        if snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
            && snapshot.authority_active
            && snapshot.phase == "live"
        {
            return MeetingSessionActionResult {
                ok: true,
                state: "already_live".to_string(),
                message: "Translation is already live. Duplicate Start did not create another Meeting session."
                    .to_string(),
                status: status_from_report(current, build_preflight()),
            };
        }
        return blocked_result(
            "active_session_conflict",
            "Another runtime session already owns Meeting resources. Stop it before starting a new Translation session."
                .to_string(),
        );
    }

    clear_all_start_preflight();
    clear_prepared_meeting_output_device();

    if let Err(message) = recover_helper_after_meeting_stop_if_needed() {
        return blocked_result(
            "helper_recovery_failed",
            format!(
                "Start Translation couldn't restore the local translation runtime after the previous Meeting Stop: {message}"
            ),
        );
    }

    // Refresh cheap worker capability truth once for this explicit Start. This
    // catches a newly approved/rebuilt My Voice without turning routine UI polling
    // into worker I/O or model loading. Functional MyVoice proof still occurs only
    // after the Meeting generation owns Starting authority.
    let _ = send_helper_worker_task("status", json!({ "meeting_start_prepare": true }));
    let preflight = build_preflight();
    if !preflight.start_eligible {
        return MeetingSessionActionResult {
            ok: false,
            state: "blocked".to_string(),
            message: preflight.summary.clone(),
            status: status_from_report(latest_runtime_session_state(), preflight),
        };
    }

    // Route discovery chooses one exact matched virtual-cable pair. Before Meeting
    // authority exists, retain the exact playback-side CPAL endpoint and verify its
    // native configuration. C5 performs the real silent callback probe transactionally
    // after the Starting authority and microphone resource exist, but before Live.
    let prepared_route = get_virtual_mic_route_selection();
    let Some(output_device) = prepared_route.selected_output_device.as_deref() else {
        return blocked_result(
            "meeting_output_prepare_failed",
            "Start Translation couldn't resolve the prepared Meeting virtual output endpoint."
                .to_string(),
        );
    };
    if let Err(blocker) = prepare_meeting_output_device(output_device) {
        return blocked_result(
            "meeting_output_prepare_failed",
            format!(
                "Start Translation couldn't prepare TranslateIT Meeting Microphone. Check Setup or Diagnostics and try again. Native output detail: {blocker}"
            ),
        );
    }

    let starting = begin_application_meeting_session();
    if !starting.blocker.is_empty() {
        return MeetingSessionActionResult {
            ok: false,
            state: "start_authority_conflict".to_string(),
            message: "Start Translation lost the runtime authority claim to another current owner. No Meeting resources were opened by this request."
                .to_string(),
            status: status_from_report(starting, build_preflight()),
        };
    }
    let Some(start_snapshot) = starting.snapshot.as_ref() else {
        return blocked_result(
            "start_authority_failed",
            "Start Translation could not establish application-level Meeting authority. No Meeting resources were opened."
                .to_string(),
        );
    };
    if start_snapshot.owner_id != APPLICATION_MEETING_OWNER_ID || !start_snapshot.authority_active {
        return blocked_result(
            "start_authority_conflict",
            "Start Translation did not receive the expected Meeting session authority. No additional resources were opened."
                .to_string(),
        );
    }

    let generation = start_snapshot.generation;
    let session_id = start_snapshot.session_id.clone();
    reset_committed_turns(&session_id);
    reset_finalized_meeting_sequence(&session_id);
    let _ = reset_self_output_suppression(&session_id);
    clear_incoming_status();

    let capture = start_live_capture_runtime(starting.clone());
    if !capture.ok {
        let _ = revoke_runtime_session_authority(
            generation,
            "Start Translation failed while opening the required microphone resource. Authority was revoked before rollback.",
        );
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back safely because the microphone resource could not be opened: {}",
                capture.message
            ),
        );
    }

    // A6 proves the real required AI path only after this generation owns Starting
    // authority and the microphone stream is open. Every worker request is tied to
    // this Meeting generation; My Voice is warm-loaded and a real English synthesis
    // fixture must complete before native output or the outbound consumer can activate.
    if let Err(stage) = prepare_required_outbound_ai_runtime(generation) {
        let _ = revoke_runtime_session_authority(
            generation,
            "Required outbound AI/My Voice verification failed during Starting. Authority was revoked before rollback.",
        );
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back before Live because {stage} could not be functionally verified for the authoritative Meeting generation."
            ),
        );
    }
    if !generation_is_starting(generation) {
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            "Start Translation lost Starting authority while verifying the required local AI/My Voice path. No Meeting output was activated.".to_string(),
        );
    }

    let ai_preflight = build_preflight();
    if !ai_preflight.ready_for_start || required_outbound_voice_actor_token(generation).is_none() {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting prerequisites changed after required AI/My Voice verification. Authority was revoked before rollback.",
        );
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return MeetingSessionActionResult {
            ok: false,
            state: "rolled_back".to_string(),
            message: "Start Translation verified My Voice, but another required Meeting prerequisite changed before native output activation. All opened resources were rolled back.".to_string(),
            status: status_from_report(latest_runtime_session_state(), ai_preflight),
        };
    }

    // Required native output execution must be proven while this generation owns
    // Starting authority. This writes silence only and requires the exact prepared
    // endpoint to build/start a CPAL stream and invoke its callback inside a bounded
    // wait. Meeting-app reception remains target-Windows evidence.
    if let Err(blocker) =
        probe_prepared_meeting_output_device_functionally(output_device, generation)
    {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting output functional verification failed during Starting. Authority was revoked before rollback.",
        );
        let _ = cancel_meeting_output_for_generation(generation);
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back because TranslateIT Meeting Microphone could not open a functional native output callback before Live: {blocker}"
            ),
        );
    }

    // The serialized required outbound consumer is a Live dependency, not a post-Live
    // best effort. Create it while the session is still Starting so a thread-spawn
    // failure can roll back without ever exposing a transient Live state.
    if let Err(error) = start_meeting_outbound_consumer(generation, &session_id) {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting outbound consumer could not start during Starting. Authority was revoked before rollback.",
        );
        let _ = cancel_meeting_output_for_generation(generation);
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        let helper_cancel = cancel_helper_bridge_meeting_session(&session_id);
        let consumer_cleanup = stop_meeting_outbound_consumer(generation);
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back before Live because the serialized outbound consumer could not start: {error}. Helper cleanup: {} Consumer cleanup: {}",
                helper_cancel.message, consumer_cleanup.message
            ),
        );
    }

    // Re-check the full required preflight after the native output callback and
    // serialized consumer both exist. A helper exit, actor invalidation, or other
    // required prerequisite loss in the activation window must roll back rather than
    // expose a transient Live state.
    let prepared_preflight = build_preflight();
    if !prepared_preflight.ready_for_start
        || required_outbound_voice_actor_token(generation).is_none()
    {
        let _ = revoke_runtime_session_authority(
            generation,
            "Final pre-Live My Voice readiness changed after required resources opened. Authority was revoked before rollback.",
        );
        let _ = cancel_meeting_output_for_generation(generation);
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        let helper_cancel = cancel_helper_bridge_meeting_session(&session_id);
        let consumer_cleanup = stop_meeting_outbound_consumer(generation);
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation rolled back before Live because final My Voice readiness changed after native resources opened. Helper cleanup: {} Consumer cleanup: {}",
                helper_cancel.message, consumer_cleanup.message
            ),
        );
    }
    remember_start_preflight(generation, prepared_preflight.clone());

    let committed = commit_application_meeting_session_live(
        generation,
        true,
        "Required microphone, generation-bound ASR/translation/My Voice functional proof, native Meeting output callback, and serialized outbound consumer were ready before the authoritative generation committed Live.",
    );
    if !committed.blocker.is_empty() {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting Live commit failed after all required pre-Live resources opened. Authority was revoked before rollback.",
        );
        let _ = cancel_meeting_output_for_generation(generation);
        let _ = stop_live_capture_runtime();
        let _ = stop_meeting_sound_capture_runtime();
        let consumer_cleanup = stop_meeting_outbound_consumer(generation);
        clear_finalized_meeting_sequence();
        clear_self_output_suppression_for_session(&session_id);
        clear_committed_turns_for_session(&session_id);
        clear_start_preflight_for_generation(generation);
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_state();
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation could not commit the Meeting generation Live, so all opened Meeting resources were rolled back. Outbound consumer cleanup: {}",
                consumer_cleanup.message
            ),
        );
    }

    update_outbound_status(
        generation,
        &session_id,
        "listening",
        0,
        false,
        true,
        "",
        "Translation Live is listening. Rolling audio remains preview-only; finalized utterances receive shared Meeting event sequence before AI.",
    );
    let incoming_message = schedule_optional_incoming_lane(&session_id);
    MeetingSessionActionResult {
        ok: true,
        state: "live".to_string(),
        message: format!(
            "Translation Live committed with authoritative My Voice outbound capture/consumer. {incoming_message}"
        ),
        status: status_from_report(committed, prepared_preflight),
    }
}

#[tauri::command]
pub fn stop_meeting_translation() -> MeetingSessionActionResult {
    let current = latest_runtime_session_state();
    if current.has_active_session && current.snapshot.is_none() {
        return MeetingSessionActionResult {
            ok: false,
            state: "runtime_state_unavailable".to_string(),
            message: "Stop Translation cannot verify current runtime ownership. The app will remain open and no cleanup success is claimed."
                .to_string(),
            status: status_from_report(current, build_preflight()),
        };
    }

    if let Some(snapshot) = current.snapshot.as_ref() {
        if snapshot.owner_id != APPLICATION_MEETING_OWNER_ID {
            return MeetingSessionActionResult {
                ok: false,
                state: "active_session_conflict".to_string(),
                message: "Stop Translation cannot control Mic Test or another non-Meeting runtime owner. Stop that operation from its own control first."
                    .to_string(),
                status: status_from_report(current, build_preflight()),
            };
        }
    }

    let Some(snapshot) = current.snapshot.as_ref() else {
        clear_all_start_preflight();
        clear_prepared_meeting_output_device();
        let incoming_capture_stop = stop_meeting_sound_capture_runtime();
        clear_finalized_incoming_utterance_producer();
        clear_deferred_incoming_queue();
        clear_finalized_meeting_sequence();
        clear_all_committed_turns();
        clear_outbound_status();
        clear_incoming_status();
        if !incoming_capture_stop.ok {
            return MeetingSessionActionResult {
                ok: false,
                state: "cleanup_incomplete".to_string(),
                message: format!(
                    "Translation has no active session, but optional Meeting Sound cleanup could not be confirmed: {}",
                    incoming_capture_stop.message
                ),
                status: status_from_report(current, build_preflight()),
            };
        }
        return MeetingSessionActionResult {
            ok: true,
            state: "already_stopped".to_string(),
            message: "Translation is already stopped. Stop remains idempotent and optional incoming audio state was cleared."
                .to_string(),
            status: status_from_report(current, build_preflight()),
        };
    };

    let generation = snapshot.generation;
    let session_id = snapshot.session_id.clone();

    let revoked = revoke_runtime_session_authority(
        generation,
        "Stop Translation accepted. Old outbound Meeting generation authority was revoked before full-session cleanup.",
    );
    if revoked.snapshot.is_none()
        || revoked
            .snapshot
            .as_ref()
            .map(|value| value.authority_active)
            .unwrap_or(true)
    {
        return blocked_result(
            "stop_authority_failed",
            "Stop Translation could not revoke Meeting generation authority, so cleanup was not allowed to proceed under an ambiguous owner."
                .to_string(),
        );
    }

    interrupt_committed_turns_for_generation(&session_id, generation);
    let _ = cancel_meeting_output_for_generation(generation);
    clear_prepared_meeting_output_device();
    let capture_stop = stop_live_capture_runtime();
    let incoming_capture_stop = stop_meeting_sound_capture_runtime();
    let helper_cancel = cancel_helper_bridge_meeting_session(&session_id);
    let outbound_cleanup = stop_meeting_outbound_consumer(generation);
    let incoming_cleanup = stop_meeting_incoming_consumer(&session_id);

    clear_self_output_suppression_for_session(&session_id);
    clear_finalized_meeting_sequence();
    clear_committed_turns_for_session(&session_id);
    clear_outbound_status();

    let cleanup_complete = meeting_cleanup_complete(
        capture_stop.ok,
        incoming_capture_stop.ok,
        helper_cancel.ok,
        outbound_cleanup.ok,
        incoming_cleanup.ok,
    );

    if !cleanup_complete {
        let mut failed = Vec::new();
        if !capture_stop.ok {
            failed.push("microphone capture");
        }
        if !incoming_capture_stop.ok {
            failed.push("Meeting Sound capture");
        }
        if !helper_cancel.ok {
            failed.push("local helper work");
        }
        if !outbound_cleanup.ok {
            failed.push("outbound consumer");
        }
        if !incoming_cleanup.ok {
            failed.push("incoming consumer");
        }
        let failed_summary = failed.join(", ");
        mark_incoming_cleanup_incomplete_status(
            &session_id,
            !incoming_capture_stop.ok,
            "Meeting output authority is revoked, but one or more cleanup steps still need attention.",
        );
        let retained = mark_runtime_session_cleanup_incomplete(
            generation,
            !capture_stop.ok,
            &format!(
                "Meeting output authority is revoked, but cleanup is incomplete for: {failed_summary}. Retry Stop Translation."
            ),
        );
        return MeetingSessionActionResult {
            ok: false,
            state: "cleanup_incomplete".to_string(),
            message: format!(
                "Translation output is stopped, but cleanup is incomplete for {failed_summary}. Retry Stop Translation. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Incoming: {}",
                capture_stop.message,
                incoming_capture_stop.message,
                helper_cancel.message,
                outbound_cleanup.message,
                incoming_cleanup.message,
            ),
            status: status_from_report(retained, build_preflight()),
        };
    }

    clear_incoming_status();
    let cleared = clear_runtime_session_if_generation(generation);
    if cleared.has_active_session
        || cleared.snapshot.is_some()
        || cleared.blocker != "runtime_session:cleared"
    {
        return MeetingSessionActionResult {
            ok: false,
            state: "cleanup_incomplete".to_string(),
            message: "All known Meeting resources stopped, but TranslateIT could not confirm that runtime ownership was cleared. The app will remain open."
                .to_string(),
            status: status_from_report(cleared, build_preflight()),
        };
    }

    clear_start_preflight_for_generation(generation);

    MeetingSessionActionResult {
        ok: true,
        state: "stopped".to_string(),
        message: format!(
            "Translation stopped. Authority was revoked before both audio lanes/helper/consumers and transient transcript/session state were cleaned. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Incoming: {}",
            capture_stop.message,
            incoming_capture_stop.message,
            helper_cancel.message,
            outbound_cleanup.message,
            incoming_cleanup.message,
        ),
        status: status_from_report(cleared, build_preflight()),
    }
}

fn meeting_cleanup_complete(
    microphone_capture_ok: bool,
    meeting_sound_capture_ok: bool,
    helper_cleanup_ok: bool,
    outbound_consumer_ok: bool,
    incoming_consumer_ok: bool,
) -> bool {
    microphone_capture_ok
        && meeting_sound_capture_ok
        && helper_cleanup_ok
        && outbound_consumer_ok
        && incoming_consumer_ok
}

#[cfg(test)]
mod b3_preflight_snapshot_tests {
    use super::{
        clear_all_start_preflight, current_start_preflight, remember_start_preflight,
        MeetingSessionPreflightStatus,
    };

    fn ready_preflight() -> MeetingSessionPreflightStatus {
        MeetingSessionPreflightStatus {
            ready_for_start: true,
            start_eligible: true,
            functional_outbound_ready: true,
            functional_outbound_verified_unix_ms: Some(1),
            microphone_ready: true,
            models_ready: true,
            helper_ready: true,
            provider_ready: true,
            meeting_route_ready: true,
            generation_aware_outbound_stages_ready: true,
            finalized_utterance_source_connected: true,
            outbound_runtime_connected: true,
            blockers: Vec::new(),
            summary: "ready".to_string(),
            runtime_claim: "b3_test".to_string(),
        }
    }

    #[test]
    fn start_preflight_snapshot_is_generation_bound() {
        clear_all_start_preflight();
        remember_start_preflight(41, ready_preflight());

        assert!(current_start_preflight(41).is_some());
        assert!(current_start_preflight(42).is_none());

        clear_all_start_preflight();
    }
}

#[cfg(test)]
mod c2_latency_tests {
    use super::{record_first_playback_timing, MeetingOutboundTiming, OutboundTimingContext};
    use std::time::{Duration, Instant};

    #[test]
    fn official_latency_runs_from_finalized_detection_to_first_playback() {
        let finalized_at = Instant::now();
        let delivery_started_at = finalized_at + Duration::from_millis(1_200);
        let first_playback_at = finalized_at + Duration::from_millis(1_275);
        let mut timing = OutboundTimingContext {
            finalized_at,
            metrics: MeetingOutboundTiming {
                finalized_unix_ms: 50_000,
                first_playback_unix_ms: None,
                speech_boundary_ms: 140,
                finalization_ms: 3,
                queue_ms: 12,
                audio_prepare_ms: 5,
                asr_ms: Some(900),
                translation_ms: Some(120),
                tts_ms: Some(160),
                delivery_ms: None,
                outbound_latency_ms: None,
            },
        };

        record_first_playback_timing(
            &mut timing,
            delivery_started_at,
            Some(first_playback_at),
            Some(51_275),
        );

        assert_eq!(timing.metrics.speech_boundary_ms, 140);
        assert_eq!(timing.metrics.delivery_ms, Some(75));
        assert_eq!(timing.metrics.outbound_latency_ms, Some(1_275));
        assert_eq!(timing.metrics.first_playback_unix_ms, Some(51_275));
    }
}

#[cfg(test)]
mod cleanup_truth_tests {
    use super::meeting_cleanup_complete;

    #[test]
    fn cleanup_truth_requires_every_owned_resource_to_release() {
        assert!(meeting_cleanup_complete(true, true, true, true, true));
        assert!(!meeting_cleanup_complete(false, true, true, true, true));
        assert!(!meeting_cleanup_complete(true, false, true, true, true));
        assert!(!meeting_cleanup_complete(true, true, false, true, true));
        assert!(!meeting_cleanup_complete(true, true, true, false, true));
        assert!(!meeting_cleanup_complete(true, true, true, true, false));
    }
}
#[cfg(test)]
mod a7_meeting_readiness_tests {
    use super::meeting_required_ai_ready;

    #[test]
    fn meeting_required_ai_readiness_depends_on_live_worker_capability_only() {
        assert!(meeting_required_ai_ready(true, true));
        assert!(!meeting_required_ai_ready(false, true));
        assert!(!meeting_required_ai_ready(true, false));
        assert!(!meeting_required_ai_ready(false, false));
    }
}

#[cfg(test)]
mod c4_functional_preflight_tests {
    use super::{meeting_required_ai_ready, meeting_start_ai_eligible};

    #[test]
    fn static_prerequisites_can_be_start_eligible_before_functional_ready() {
        assert!(meeting_start_ai_eligible(true, true));
        assert!(meeting_required_ai_ready(true, true));
        assert!(!meeting_start_ai_eligible(true, false));
        assert!(!meeting_required_ai_ready(false, true));
    }
}

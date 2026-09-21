use serde_json::json;

use crate::engine::audio::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, clear_finalized_meeting_sequence,
    reset_finalized_drop_counters, reset_finalized_meeting_sequence,
};
use crate::engine::audio::live_segment_writer::cleanup_finalized_meeting_session_wavs;
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::audio::meeting_output::{
    cancel_meeting_output_for_generation, clear_prepared_meeting_output_device,
    prepare_meeting_output_device, probe_prepared_meeting_output_device_functionally,
};
use crate::engine::audio::meeting_sound_capture::stop_meeting_sound_capture_runtime;
use crate::engine::runtime_state::{
    begin_application_meeting_session, clear_runtime_session_if_generation,
    commit_application_meeting_session_live,
    latest_runtime_session_state, mark_runtime_session_cleanup_incomplete,
    revoke_runtime_session_authority,
};

use super::super::virtual_mic_route::{
    bind_prepared_virtual_mic_route_to_generation, clear_prepared_virtual_mic_route_selection,
    get_virtual_mic_route_selection,
};
use super::super::helper_bridge::{
    cancel_helper_bridge_meeting_session, get_helper_bridge_status,
    prepare_required_outbound_ai_runtime, required_outbound_voice_actor_token,
    send_helper_worker_task, start_helper_bridge,
};
use super::committed_turns::{
    clear_all_committed_turns, clear_committed_turns_for_session,
    interrupt_committed_turns_for_generation, reset_committed_turns,
    retain_committed_turns_for_export,
};
use super::consumer_runtime::{
    start_meeting_outbound_consumer, stop_meeting_incoming_consumer,
    stop_meeting_outbound_consumer,
};
use super::incoming_activation::schedule_optional_incoming_lane;
use super::incoming_deferred::{
    clear_deferred_incoming_queue, reset_deferred_incoming_drop_counters,
};
use super::outbound_pipeline::cleanup_meeting_tts_for_session;
use super::preflight::{blocked_result, build_preflight, status_from_report};
use super::session_state::{
    clear_all_start_preflight, clear_incoming_status, clear_outbound_status,
    clear_start_preflight_for_generation, mark_incoming_cleanup_incomplete_status,
    remember_start_preflight, update_outbound_status,
};
use super::suppression::{
    clear_self_output_suppression_for_session, reset_self_output_suppression,
};
use super::{
    generation_is_starting, MeetingSessionActionResult, APPLICATION_MEETING_OWNER_ID,
};

fn rollback_starting_meeting_resources(
    generation: u64,
    session_id: &str,
) -> (String, String) {
    let _ = cancel_meeting_output_for_generation(generation);
    let _ = stop_live_capture_runtime();
    let _ = stop_meeting_sound_capture_runtime();
    let helper_cancel = cancel_helper_bridge_meeting_session(session_id);
    let consumer_cleanup = stop_meeting_outbound_consumer(generation);
    clear_finalized_meeting_sequence();
    clear_self_output_suppression_for_session(session_id);
    clear_committed_turns_for_session(session_id);
    clear_start_preflight_for_generation(generation);
    clear_prepared_meeting_output_device();
    clear_prepared_virtual_mic_route_selection();
    let _ = clear_runtime_session_if_generation(generation);
    (helper_cancel.message, consumer_cleanup.message)
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

pub(super) fn start_meeting_translation_impl() -> MeetingSessionActionResult {
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
        clear_prepared_meeting_output_device();
        return MeetingSessionActionResult {
            ok: false,
            state: "start_authority_conflict".to_string(),
            message: "Start Translation lost the runtime authority claim to another current owner. No Meeting resources were opened by this request."
                .to_string(),
            status: status_from_report(starting, build_preflight()),
        };
    }
    let Some(start_snapshot) = starting.snapshot.as_ref() else {
        clear_prepared_meeting_output_device();
        return blocked_result(
            "start_authority_failed",
            "Start Translation could not establish application-level Meeting authority. No Meeting resources were opened."
                .to_string(),
        );
    };
    if start_snapshot.owner_id != APPLICATION_MEETING_OWNER_ID || !start_snapshot.authority_active {
        clear_prepared_meeting_output_device();
        return blocked_result(
            "start_authority_conflict",
            "Start Translation did not receive the expected Meeting session authority. No additional resources were opened."
                .to_string(),
        );
    }

    let generation = start_snapshot.generation;
    let session_id = start_snapshot.session_id.clone();
    if let Err(blocker) = bind_prepared_virtual_mic_route_to_generation(generation) {
        let _ = revoke_runtime_session_authority(
            generation,
            "Prepared Meeting route could not bind to the new generation. Authority was revoked before opening Meeting resources.",
        );
        clear_prepared_meeting_output_device();
        let _ = clear_runtime_session_if_generation(generation);
        return blocked_result(
            "meeting_route_bind_failed",
            format!(
                "Start Translation couldn't bind the prepared Meeting microphone route to this session: {blocker}"
            ),
        );
    }
    reset_finalized_drop_counters();
    reset_deferred_incoming_drop_counters();
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
        let _ = rollback_starting_meeting_resources(generation, &session_id);
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
        let _ = rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back before Live because {stage} could not be functionally verified for the authoritative Meeting generation."
            ),
        );
    }
    if !generation_is_starting(generation) {
        let _ = rollback_starting_meeting_resources(generation, &session_id);
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
        let _ = rollback_starting_meeting_resources(generation, &session_id);
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
        let _ = rollback_starting_meeting_resources(generation, &session_id);
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
        let (helper_cleanup, consumer_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation was rolled back before Live because the serialized outbound consumer could not start: {error}. Helper cleanup: {} Consumer cleanup: {}",
                helper_cleanup, consumer_cleanup
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
        let (helper_cleanup, consumer_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation rolled back before Live because final My Voice readiness changed after native resources opened. Helper cleanup: {} Consumer cleanup: {}",
                helper_cleanup, consumer_cleanup
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
        let (helper_cleanup, consumer_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            "rolled_back",
            format!(
                "Start Translation could not commit the Meeting generation Live, so all opened Meeting resources were rolled back. Helper cleanup: {} Consumer cleanup: {}",
                helper_cleanup, consumer_cleanup
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

pub(super) fn stop_meeting_translation_impl() -> MeetingSessionActionResult {
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
        let deferred_cleanup = clear_deferred_incoming_queue();
        clear_finalized_meeting_sequence();
        let transcript_cleanup_ok = clear_all_committed_turns();
        clear_outbound_status();
        clear_incoming_status();
        if !incoming_capture_stop.ok || deferred_cleanup.is_err() || !transcript_cleanup_ok {
            return MeetingSessionActionResult {
                ok: false,
                state: "cleanup_incomplete".to_string(),
                message: format!(
                    "Translation has no active session, but transient cleanup could not be confirmed. Meeting Sound: {} Deferred queue: {} Transcript state: {}",
                    incoming_capture_stop.message,
                    deferred_cleanup
                        .err()
                        .unwrap_or_else(|| "clean".to_string()),
                    if transcript_cleanup_ok { "clean" } else { "unverified" }
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
    let transcript_retained_for_export = retain_committed_turns_for_export(&session_id);
    let _ = cancel_meeting_output_for_generation(generation);
    clear_prepared_meeting_output_device();
    let capture_stop = stop_live_capture_runtime();
    let incoming_capture_stop = stop_meeting_sound_capture_runtime();
    let helper_cancel = cancel_helper_bridge_meeting_session(&session_id);
    let outbound_cleanup = stop_meeting_outbound_consumer(generation);
    let incoming_cleanup = stop_meeting_incoming_consumer(&session_id);

    let suppression_cleanup_ok = clear_self_output_suppression_for_session(&session_id);
    clear_finalized_meeting_sequence();
    let transcript_cleanup_ok = clear_committed_turns_for_session(&session_id);
    let finalized_audio_cleanup = cleanup_finalized_meeting_session_wavs(&session_id);
    let tts_cache_cleanup = cleanup_meeting_tts_for_session(&session_id, generation);
    let transient_audio_cleanup_ok =
        finalized_audio_cleanup.is_ok() && tts_cache_cleanup.is_ok();
    clear_outbound_status();

    let cleanup_complete = transcript_retained_for_export && meeting_cleanup_complete(
        capture_stop.ok,
        incoming_capture_stop.ok,
        helper_cancel.ok,
        outbound_cleanup.ok,
        incoming_cleanup.ok,
        suppression_cleanup_ok,
        transcript_cleanup_ok,
        transient_audio_cleanup_ok,
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
        if !suppression_cleanup_ok {
            failed.push("self-output suppression state");
        }
        if !transcript_retained_for_export {
            failed.push("transient transcript export snapshot");
        }
        if !transcript_cleanup_ok {
            failed.push("committed transcript state");
        }
        if !transient_audio_cleanup_ok {
            failed.push("transient Meeting audio cache");
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
                "Translation output is stopped, but cleanup is incomplete for {failed_summary}. Retry Stop Translation. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Incoming: {} Suppression: {} Transcript: {} Finalized audio: {} TTS cache: {}",
                capture_stop.message,
                incoming_capture_stop.message,
                helper_cancel.message,
                outbound_cleanup.message,
                incoming_cleanup.message,
                if suppression_cleanup_ok { "clean" } else { "unverified" },
                if transcript_cleanup_ok { "clean" } else { "unverified" },
                finalized_audio_cleanup
                    .as_ref()
                    .map(|count| format!("clean ({count} removed)"))
                    .unwrap_or_else(|error| error.clone()),
                tts_cache_cleanup
                    .as_ref()
                    .map(|count| format!("clean ({count} removed)"))
                    .unwrap_or_else(|error| error.clone()),
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
            "Translation stopped. Authority was revoked before both audio lanes/helper/consumers and transient transcript/session state were cleaned. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Incoming: {} Suppression: clean Transcript: clean Finalized audio: {} TTS cache: {}",
            capture_stop.message,
            incoming_capture_stop.message,
            helper_cancel.message,
            outbound_cleanup.message,
            incoming_cleanup.message,
            finalized_audio_cleanup
                .as_ref()
                .map(|count| format!("clean ({count} removed)"))
                .unwrap_or_else(|error| error.clone()),
            tts_cache_cleanup
                .as_ref()
                .map(|count| format!("clean ({count} removed)"))
                .unwrap_or_else(|error| error.clone()),
        ),
        status: status_from_report(cleared, build_preflight()),
    }
}

pub(super) fn meeting_cleanup_complete(
    microphone_capture_ok: bool,
    meeting_sound_capture_ok: bool,
    helper_cleanup_ok: bool,
    outbound_consumer_ok: bool,
    incoming_consumer_ok: bool,
    suppression_cleanup_ok: bool,
    transcript_cleanup_ok: bool,
    transient_audio_cleanup_ok: bool,
) -> bool {
    microphone_capture_ok
        && meeting_sound_capture_ok
        && helper_cleanup_ok
        && outbound_consumer_ok
        && incoming_consumer_ok
        && suppression_cleanup_ok
        && transcript_cleanup_ok
        && transient_audio_cleanup_ok
}

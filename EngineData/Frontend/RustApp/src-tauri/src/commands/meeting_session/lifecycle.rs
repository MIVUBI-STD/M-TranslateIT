use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::engine::audio::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, clear_finalized_meeting_sequence,
    reset_finalized_drop_counters, reset_finalized_meeting_sequence,
};
use crate::engine::audio::live_segment_writer::cleanup_finalized_meeting_session_wavs;
use crate::engine::audio::original_voice_pcm;
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::audio::meeting_output::{
    cancel_meeting_output_for_generation, stop_original_voice_output, start_original_voice_output, clear_prepared_meeting_output_device,
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
    clear_inactive_committed_turns, clear_committed_turns_for_session,
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
use super::playback_runtime::{
    start_meeting_playback_runtime, stop_meeting_playback_runtime,
};
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

// A no-session Stop can still fail to clear transient audio/transcript resources.
// Keep that native fact visible to preflight and the UI until Stop succeeds.
static STOP_CLEANUP_PENDING: AtomicBool = AtomicBool::new(false);

pub(super) fn transient_stop_cleanup_pending() -> bool {
    STOP_CLEANUP_PENDING.load(Ordering::Acquire)
}

// A failed Starting transaction may release session ownership only after
// every owned resource reports cleanup success. Incoming consumer starts later.
fn starting_rollback_cleanup_complete(
    output_cancelled: bool,
    microphone_ok: bool,
    sound_ok: bool,
    helper_ok: bool,
    outbound_ok: bool,
    playback_ok: bool,
    suppression_ok: bool,
    transcript_ok: bool,
    temporary_audio_ok: bool,
) -> bool {
    meeting_cleanup_complete(
        output_cancelled, microphone_ok, sound_ok, helper_ok, outbound_ok, playback_ok,
        true, suppression_ok, transcript_ok, temporary_audio_ok,
    )
}

fn rollback_starting_meeting_resources(
    generation: u64,
    session_id: &str,
) -> (String, String) {
    // Silence the raw PCM callback before any potentially slow native teardown.
    original_voice_pcm::stop(generation);
    let tts_cancelled = cancel_meeting_output_for_generation(generation);
    let original_stopped = stop_original_voice_output(generation);
    let output_cancelled = tts_cancelled && original_stopped;
    let microphone = stop_live_capture_runtime();
    let sound = stop_meeting_sound_capture_runtime();
    let helper = cancel_helper_bridge_meeting_session(session_id);
    let outbound = stop_meeting_outbound_consumer(generation);
    let playback = stop_meeting_playback_runtime(generation);
    clear_finalized_meeting_sequence();
    let suppression_ok = clear_self_output_suppression_for_session(session_id);
    let transcript_ok = clear_committed_turns_for_session(session_id);
    let wav_ok = cleanup_finalized_meeting_session_wavs(session_id).is_ok();
    let tts_ok = cleanup_meeting_tts_for_session(session_id, generation).is_ok();
    clear_start_preflight_for_generation(generation);
    clear_prepared_meeting_output_device();
    clear_prepared_virtual_mic_route_selection();

    // Starting has no optional incoming consumer; all other owned cleanup
    // must pass before releasing the revoked generation to another session.
    let cleanup_ok = starting_rollback_cleanup_complete(
        output_cancelled, microphone.ok, sound.ok, helper.ok, outbound.ok,
        playback.ok, suppression_ok, transcript_ok, wav_ok && tts_ok,
    );
    if cleanup_ok {
        let cleared = clear_runtime_session_if_generation(generation);
        STOP_CLEANUP_PENDING.store(
            cleared.has_active_session || cleared.snapshot.is_some()
                || cleared.blocker != "runtime_session:cleared",
            Ordering::Release,
        );
    } else {
        STOP_CLEANUP_PENDING.store(true, Ordering::Release);
        let _ = mark_runtime_session_cleanup_incomplete(
            generation,
            !microphone.ok,
            "Starting rollback cleanup is incomplete. Retry Stop Translation.",
        );
    }
    let state = if transient_stop_cleanup_pending() {
        "cleanup_incomplete: retry Stop Translation"
    } else {
        "cleaned"
    };
    (
        helper.message,
        format!(
            "Microphone: {} Meeting Sound: {} Outbound: {} Playback: {} Suppression: {} Transcript: {} Temporary audio: {} Rollback: {}.",
            microphone.message, sound.message, outbound.message, playback.message,
            if suppression_ok { "clean" } else { "unverified" },
            if transcript_ok { "clean" } else { "unverified" },
            if wav_ok && tts_ok { "clean" } else { "unverified" },
            state,
        ),
    )
}

// Preserve the only recoverable active transcript when its last-session
// archive could not be written. The retry Stop must see the original turns.
fn clear_transcript_only_after_retention(
    retention_succeeded: bool,
    clear_active: impl FnOnce() -> bool,
) -> bool {
    retention_succeeded && clear_active()
}

fn rollback_completion_state() -> &'static str {
    if transient_stop_cleanup_pending() { "cleanup_incomplete" } else { "rolled_back" }
}

#[cfg(test)]
mod rollback_cleanup_contract_tests {
    use super::starting_rollback_cleanup_complete;

    #[test]
    fn failed_transcript_archive_must_not_clear_active_turns() {
        use super::clear_transcript_only_after_retention;

        let mut cleared = false;
        assert!(!clear_transcript_only_after_retention(false, || {
            cleared = true;
            true
        }));
        assert!(!cleared, "failed archive must preserve active turns for Stop retry");

        assert!(clear_transcript_only_after_retention(true, || {
            cleared = true;
            true
        }));
        assert!(cleared);
        assert!(!clear_transcript_only_after_retention(true, || false));
    }

    #[test]
    fn failed_start_cannot_treat_any_required_cleanup_as_complete() {
        let all = [true; 9];
        assert!(starting_rollback_cleanup_complete(
            all[0], all[1], all[2], all[3], all[4], all[5],
            all[6], all[7], all[8],
        ));
        for failure in 0..9 {
            let mut results = all;
            results[failure] = false;
            assert!(!starting_rollback_cleanup_complete(
                results[0], results[1], results[2], results[3], results[4],
                results[5], results[6], results[7], results[8],
            ), "cleanup requirement {failure} must prevent releasing session");
        }
    }
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
    start_meeting_impl(false)
}

/// The Original mode bypasses helper/ASR/translation/TTS initialization.
/// It shares the exact same Meeting generation and Stop cleanup as translation.
pub(super) fn start_meeting_original_impl() -> MeetingSessionActionResult {
    start_meeting_impl(true)
}

fn start_meeting_impl(original_requested: bool) -> MeetingSessionActionResult {
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

    if transient_stop_cleanup_pending() {
        return blocked_result(
            "cleanup_incomplete",
            "The previous Meeting Stop still needs cleanup. Retry Stop Translation before starting a new session.".to_string(),
        );
    }

    clear_all_start_preflight();
    clear_prepared_meeting_output_device();

    if !original_requested {
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
    }
    let preflight = build_preflight();
    let original_eligible = preflight.microphone_ready && preflight.meeting_route_ready
        && !transient_stop_cleanup_pending();
    if !(if original_requested { original_eligible } else { preflight.start_eligible }) {
        return MeetingSessionActionResult {
            ok: false,
            state: "blocked".to_string(),
            message: if original_requested {
                if !preflight.microphone_ready {
                    "Original Microphone needs a working physical microphone.".to_string()
                } else if !preflight.meeting_route_ready {
                    "Original Microphone needs the configured virtual audio route.".to_string()
                } else {
                    "Original Microphone audio setup is not ready. Retry Stop if cleanup is incomplete.".to_string()
                }
            } else {
                preflight.summary.clone()
            },
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
    if let Err(blocker) = original_voice_pcm::register_translated(generation) {
        let _ = revoke_runtime_session_authority(generation, "Audio source initialization failed.");
        let _ = rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(rollback_completion_state(),
            format!("Meeting audio source initialization failed: {blocker}"));
    }
    if original_requested {
        // Source epoch 2 starts raw passthrough immediately; no outgoing VAD
        // finalization is allowed while Original is selected.
        if let Err(blocker) = original_voice_pcm::begin(generation, 2) {
            let _ = revoke_runtime_session_authority(generation, "Original audio initialization failed.");
            let _ = rollback_starting_meeting_resources(generation, &session_id);
            return blocked_result(rollback_completion_state(),
                format!("Original microphone could not initialize: {blocker}"));
        }
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
            rollback_completion_state(),
            format!(
                "Start Translation could not open the microphone: {}. Rollback attempted; retry Stop Translation if cleanup is incomplete.",
                capture.message
            ),
        );
    }

    if original_requested {
        // The source selector has only physical capture and native virtual output
        // prerequisites. No helper process, model probe or outgoing TTS consumer.
        if !generation_is_starting(generation) {
            let _ = revoke_runtime_session_authority(generation, "Original Meeting start lost authority.");
            let _ = rollback_starting_meeting_resources(generation, &session_id);
            return blocked_result(rollback_completion_state(),
                "Original Meeting start lost session authority before output.".to_string());
        }
        if let Err(blocker) = start_original_voice_output(output_device, generation, 2) {
            let _ = revoke_runtime_session_authority(generation, "Original Meeting output failed.");
            let _ = rollback_starting_meeting_resources(generation, &session_id);
            return blocked_result(rollback_completion_state(),
                format!("Original Microphone could not start: {blocker}"));
        }
        let committed = commit_application_meeting_session_live(
            generation, true,
            "Original microphone capture and native virtual output are active. AI was not required.",
        );
        if !committed.blocker.is_empty() {
            let _ = revoke_runtime_session_authority(generation, "Original Meeting Live commit failed.");
            let _ = rollback_starting_meeting_resources(generation, &session_id);
            return blocked_result(rollback_completion_state(),
                "Original Microphone could not enter Live mode.".to_string());
        }
        update_outbound_status(
            generation, &session_id, "listening", 0, false, true, "",
            "Original Microphone is passing audio directly to the virtual input without AI.",
        );
        return MeetingSessionActionResult {
            ok: true,
            state: "live".to_string(),
            message: "Original Microphone is live. Translation models were not started.".to_string(),
            status: status_from_report(committed, build_preflight()),
        };
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
            rollback_completion_state(),
            format!(
                "Start Translation could not verify {stage} before Live. Rollback attempted; retry Stop Translation if cleanup is incomplete."
            ),
        );
    }
    if !generation_is_starting(generation) {
        let _ = rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            rollback_completion_state(),
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
            state: rollback_completion_state().to_string(),
            message: "Start Translation verified the voice, but another prerequisite changed before Live. Rollback attempted; retry Stop Translation if cleanup is incomplete.".to_string(),
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
        let _ = rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            rollback_completion_state(),
            format!(
                "Start Translation could not open the Meeting output callback before Live: {blocker}. Rollback attempted; retry Stop Translation if cleanup is incomplete."
            ),
        );
    }

    // Playback ownership is established before the AI consumer so the first
    // prepared turn always has a bounded receiver. Failure rolls back before Live.
    if let Err(error) = start_meeting_playback_runtime(generation, &session_id) {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting playback runtime could not start during Starting. Authority was revoked before rollback.",
        );
        let (helper_cleanup, runtime_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            rollback_completion_state(),
            format!(
                "Start Translation could not start Meeting playback before Live: {error}. Helper cleanup: {} Runtime cleanup: {}",
                helper_cleanup, runtime_cleanup
            ),
        );
    }

    // The serialized required outbound AI consumer is a Live dependency, not a
    // post-Live best effort. It hands prepared voice output to bounded playback
    // instead of waiting for prior playback to finish.
    if let Err(error) = start_meeting_outbound_consumer(generation, &session_id) {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting outbound consumer could not start during Starting. Authority was revoked before rollback.",
        );
        let (helper_cleanup, consumer_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            rollback_completion_state(),
            format!(
                "Start Translation could not start the outbound consumer before Live: {error}. Helper cleanup: {} Consumer cleanup: {}",
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
            rollback_completion_state(),
            format!(
                "Start Translation lost final voice readiness before Live. Helper cleanup: {} Consumer cleanup: {}",
                helper_cleanup, consumer_cleanup
            ),
        );
    }
    remember_start_preflight(generation, prepared_preflight.clone());

    let committed = commit_application_meeting_session_live(
        generation,
        true,
        "Required microphone, generation-bound ASR/translation/My Voice functional proof, native Meeting output callback, bounded playback runtime, and serialized outbound AI consumer were ready before the authoritative generation committed Live.",
    );
    if !committed.blocker.is_empty() {
        let _ = revoke_runtime_session_authority(
            generation,
            "Meeting Live commit failed after all required pre-Live resources opened. Authority was revoked before rollback.",
        );
        let (helper_cleanup, consumer_cleanup) =
            rollback_starting_meeting_resources(generation, &session_id);
        return blocked_result(
            rollback_completion_state(),
            format!(
                "Start Translation could not commit the Meeting generation Live. Rollback attempted. Helper cleanup: {} Consumer cleanup: {}",
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
        let transcript_cleanup_ok = clear_inactive_committed_turns();
        clear_outbound_status();
        clear_incoming_status();
        if !incoming_capture_stop.ok || deferred_cleanup.is_err() || !transcript_cleanup_ok {
            STOP_CLEANUP_PENDING.store(true, Ordering::Release);
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
        STOP_CLEANUP_PENDING.store(false, Ordering::Release);
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
    // Silence the raw PCM callback before any potentially slow native teardown.
    original_voice_pcm::stop(generation);
    let tts_cancelled = cancel_meeting_output_for_generation(generation);
    let original_stopped = stop_original_voice_output(generation);
    let output_cancelled = tts_cancelled && original_stopped;
    clear_prepared_meeting_output_device();
    let capture_stop = stop_live_capture_runtime();
    let incoming_capture_stop = stop_meeting_sound_capture_runtime();
    let helper_cancel = cancel_helper_bridge_meeting_session(&session_id);
    let outbound_cleanup = stop_meeting_outbound_consumer(generation);
    let playback_cleanup = stop_meeting_playback_runtime(generation);
    let incoming_cleanup = stop_meeting_incoming_consumer(&session_id);

    let suppression_cleanup_ok = clear_self_output_suppression_for_session(&session_id);
    clear_finalized_meeting_sequence();
    let transcript_cleanup_ok = clear_transcript_only_after_retention(
        transcript_retained_for_export,
        || clear_committed_turns_for_session(&session_id),
    );
    let finalized_audio_cleanup = cleanup_finalized_meeting_session_wavs(&session_id);
    let tts_cache_cleanup = cleanup_meeting_tts_for_session(&session_id, generation);
    let transient_audio_cleanup_ok =
        finalized_audio_cleanup.is_ok() && tts_cache_cleanup.is_ok();
    clear_outbound_status();

    let cleanup_complete = transcript_retained_for_export && meeting_cleanup_complete(
        output_cancelled,
        capture_stop.ok,
        incoming_capture_stop.ok,
        helper_cancel.ok,
        outbound_cleanup.ok,
        playback_cleanup.ok,
        incoming_cleanup.ok,
        suppression_cleanup_ok,
        transcript_cleanup_ok,
        transient_audio_cleanup_ok,
    );

    if !cleanup_complete {
        let mut failed = Vec::new();
        if !output_cancelled {
            failed.push("Meeting output cancellation");
        }
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
        if !playback_cleanup.ok {
            failed.push("playback runtime");
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
                "Meeting output authority is revoked, but cleanup is incomplete for {failed_summary}. Retry Stop Translation. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Playback: {} Incoming: {} Suppression: {} Transcript: {} Finalized audio: {} TTS cache: {}",
                capture_stop.message,
                incoming_capture_stop.message,
                helper_cancel.message,
                outbound_cleanup.message,
                playback_cleanup.message,
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
        STOP_CLEANUP_PENDING.store(true, Ordering::Release);
        return MeetingSessionActionResult {
            ok: false,
            state: "cleanup_incomplete".to_string(),
            message: "All known Meeting resources stopped, but TranslateIT could not confirm that runtime ownership was cleared. The app will remain open."
                .to_string(),
            status: status_from_report(cleared, build_preflight()),
        };
    }

    clear_start_preflight_for_generation(generation);
    STOP_CLEANUP_PENDING.store(false, Ordering::Release);

    MeetingSessionActionResult {
        ok: true,
        state: "stopped".to_string(),
        message: format!(
            "Translation stopped. Authority was revoked before both audio lanes/helper/consumers and transient transcript/session state were cleaned. Microphone: {} Meeting Sound: {} Helper: {} Outbound: {} Playback: {} Incoming: {} Suppression: clean Transcript: clean Finalized audio: {} TTS cache: {}",
            capture_stop.message,
            incoming_capture_stop.message,
            helper_cancel.message,
            outbound_cleanup.message,
            playback_cleanup.message,
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
    output_cancellation_ok: bool,
    microphone_capture_ok: bool,
    meeting_sound_capture_ok: bool,
    helper_cleanup_ok: bool,
    outbound_consumer_ok: bool,
    playback_runtime_ok: bool,
    incoming_consumer_ok: bool,
    suppression_cleanup_ok: bool,
    transcript_cleanup_ok: bool,
    transient_audio_cleanup_ok: bool,
) -> bool {
    output_cancellation_ok
        && microphone_capture_ok
        && meeting_sound_capture_ok
        && helper_cleanup_ok
        && outbound_consumer_ok
        && playback_runtime_ok
        && incoming_consumer_ok
        && suppression_cleanup_ok
        && transcript_cleanup_ok
        && transient_audio_cleanup_ok
}

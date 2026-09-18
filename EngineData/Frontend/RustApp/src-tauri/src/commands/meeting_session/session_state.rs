use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::engine::audio::finalized_utterance::{
    evicted_pending_utterance_count, overflow_dropped_utterance_count, FinalizedMeetingUtterance,
};
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;

use super::super::helper_bridge_runtime::unix_ms;
use super::{
    MeetingIncomingRuntimeStatus, MeetingOutboundRuntimeStatus, MeetingOutboundTiming,
    MeetingSessionPreflightStatus,
};

struct MeetingStartPreflightRuntime {
    generation: u64,
    status: MeetingSessionPreflightStatus,
}

pub(super) struct OutboundTimingContext {
    pub(super) finalized_at: Instant,
    pub(super) metrics: MeetingOutboundTiming,
}

static MEETING_OUTBOUND_STATUS: OnceLock<Mutex<MeetingOutboundRuntimeStatus>> = OnceLock::new();
static MEETING_INCOMING_STATUS: OnceLock<Mutex<MeetingIncomingRuntimeStatus>> = OnceLock::new();
static MEETING_START_PREFLIGHT: OnceLock<Mutex<Option<MeetingStartPreflightRuntime>>> =
    OnceLock::new();

pub(super) fn idle_outbound_status() -> MeetingOutboundRuntimeStatus {
    MeetingOutboundRuntimeStatus {
        generation: None,
        session_id: None,
        stage: "idle".to_string(),
        utterance_sequence: 0,
        output_active: false,
        last_stage_ok: true,
        timing: None,
        overflow_dropped_utterance_count: overflow_dropped_utterance_count(),
        evicted_pending_utterance_count: evicted_pending_utterance_count(),
        blocker: String::new(),
        note: "The finalized-utterance producer and serialized Meeting outbound consumer are source-connected. No output is active until an authoritative Live session produces finalized speech."
            .to_string(),
        updated_unix_ms: unix_ms(),
        runtime_claim: "meeting_outbound_finalized_segment_contract_source_side_not_windows_runtime_proof"
            .to_string(),
    }
}

pub(super) fn idle_incoming_status() -> MeetingIncomingRuntimeStatus {
    MeetingIncomingRuntimeStatus {
        session_id: None,
        stage: "unavailable".to_string(),
        capture_active: false,
        suppressed: false,
        degraded: false,
        blocker: String::new(),
        note: "Incoming Meeting Sound is an optional lane and is not active without an application Meeting session."
            .to_string(),
        updated_unix_ms: unix_ms(),
        runtime_claim: "meeting_incoming_optional_lane_source_contract_not_windows_runtime_proof"
            .to_string(),
    }
}

pub(super) fn outbound_status_store() -> &'static Mutex<MeetingOutboundRuntimeStatus> {
    MEETING_OUTBOUND_STATUS.get_or_init(|| Mutex::new(idle_outbound_status()))
}

pub(super) fn incoming_status_store() -> &'static Mutex<MeetingIncomingRuntimeStatus> {
    MEETING_INCOMING_STATUS.get_or_init(|| Mutex::new(idle_incoming_status()))
}

pub(super) fn start_preflight_store() -> &'static Mutex<Option<MeetingStartPreflightRuntime>> {
    MEETING_START_PREFLIGHT.get_or_init(|| Mutex::new(None))
}

pub(super) fn remember_start_preflight(generation: u64, status: MeetingSessionPreflightStatus) {
    if let Ok(mut guard) = start_preflight_store().lock() {
        *guard = Some(MeetingStartPreflightRuntime { generation, status });
    }
}

pub(super) fn current_start_preflight(generation: u64) -> Option<MeetingSessionPreflightStatus> {
    start_preflight_store().lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|snapshot| snapshot.generation == generation)
            .map(|snapshot| snapshot.status.clone())
    })
}

pub(super) fn clear_start_preflight_for_generation(generation: u64) {
    if let Ok(mut guard) = start_preflight_store().lock() {
        if guard.as_ref().map(|snapshot| snapshot.generation) == Some(generation) {
            *guard = None;
        }
    }
}

pub(super) fn clear_all_start_preflight() {
    if let Ok(mut guard) = start_preflight_store().lock() {
        *guard = None;
    }
}

pub(super) fn current_outbound_status() -> MeetingOutboundRuntimeStatus {
    outbound_status_store()
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| idle_outbound_status())
}

pub(super) fn current_incoming_status() -> MeetingIncomingRuntimeStatus {
    let mut status = incoming_status_store()
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| idle_incoming_status());
    let capture = meeting_sound_capture_status();
    if status.session_id.is_some() {
        if status.stage == "cleanup_incomplete"
            && capture.blocker == "meeting_sound:state_lock_failed"
        {
            status.capture_active = true;
            status.suppressed = false;
        } else {
            status.capture_active = capture.stream_active;
            status.suppressed = capture.suppression_active;
        }
        if capture.stream_active && capture.callback_error_count > 0 {
            status.degraded = true;
            if status.blocker.is_empty() {
                status.blocker = "meeting_incoming:capture_callback_error".to_string();
            }
        }
    }
    status
}

pub(super) fn update_outbound_status(
    generation: u64,
    session_id: &str,
    stage: &str,
    utterance_sequence: u64,
    output_active: bool,
    last_stage_ok: bool,
    blocker: &str,
    note: &str,
) {
    if let Ok(mut status) = outbound_status_store().lock() {
        let timing = if status.generation == Some(generation)
            && status.session_id.as_deref() == Some(session_id)
            && status.utterance_sequence == utterance_sequence
        {
            status.timing.clone()
        } else {
            None
        };
        *status = MeetingOutboundRuntimeStatus {
            generation: Some(generation),
            session_id: Some(session_id.to_string()),
            stage: stage.to_string(),
            utterance_sequence,
            output_active,
            last_stage_ok,
            timing,
            overflow_dropped_utterance_count: overflow_dropped_utterance_count(),
            evicted_pending_utterance_count: evicted_pending_utterance_count(),
            blocker: blocker.to_string(),
            note: note.to_string(),
            updated_unix_ms: unix_ms(),
            runtime_claim:
                "meeting_outbound_finalized_segment_contract_source_side_not_windows_runtime_proof"
                    .to_string(),
        };
    }
}

pub(super) fn set_outbound_timing(
    generation: u64,
    session_id: &str,
    utterance_sequence: u64,
    timing: &MeetingOutboundTiming,
) {
    if let Ok(mut status) = outbound_status_store().lock() {
        if status.generation == Some(generation)
            && status.session_id.as_deref() == Some(session_id)
            && status.utterance_sequence == utterance_sequence
        {
            status.timing = Some(timing.clone());
            status.updated_unix_ms = unix_ms();
        }
    }
}

pub(super) fn duration_to_millis(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn elapsed_millis(start: Instant, end: Instant) -> u64 {
    end.checked_duration_since(start)
        .map(duration_to_millis)
        .unwrap_or(0)
}

pub(super) fn timing_context_from_utterance(
    utterance: &FinalizedMeetingUtterance,
    queue_ms: u64,
    audio_prepare_ms: u64,
) -> OutboundTimingContext {
    OutboundTimingContext {
        finalized_at: utterance.finalized_at,
        metrics: MeetingOutboundTiming {
            finalized_unix_ms: utterance.finalized_unix_ms,
            first_playback_unix_ms: None,
            speech_boundary_ms: utterance.speech_boundary_ms,
            finalization_ms: utterance.finalization_ms,
            queue_ms,
            audio_prepare_ms,
            asr_ms: None,
            translation_ms: None,
            tts_ms: None,
            delivery_ms: None,
            outbound_latency_ms: None,
        },
    }
}

pub(super) fn record_first_playback_timing(
    timing: &mut OutboundTimingContext,
    delivery_started_at: Instant,
    first_playback_at: Option<Instant>,
    first_playback_unix_ms: Option<u128>,
) {
    let Some(first_playback_at) = first_playback_at else {
        return;
    };
    timing.metrics.delivery_ms = Some(elapsed_millis(delivery_started_at, first_playback_at));
    timing.metrics.outbound_latency_ms =
        Some(elapsed_millis(timing.finalized_at, first_playback_at));
    timing.metrics.first_playback_unix_ms = first_playback_unix_ms;
}

pub(super) fn update_incoming_status(
    session_id: &str,
    stage: &str,
    degraded: bool,
    blocker: &str,
    note: &str,
) {
    if let Ok(mut status) = incoming_status_store().lock() {
        let capture = meeting_sound_capture_status();
        *status = MeetingIncomingRuntimeStatus {
            session_id: Some(session_id.to_string()),
            stage: stage.to_string(),
            capture_active: capture.stream_active,
            suppressed: capture.suppression_active,
            degraded,
            blocker: blocker.to_string(),
            note: note.to_string(),
            updated_unix_ms: unix_ms(),
            runtime_claim:
                "meeting_incoming_optional_lane_source_contract_not_windows_runtime_proof"
                    .to_string(),
        };
    }
}

pub(super) fn clear_outbound_status() {
    if let Ok(mut status) = outbound_status_store().lock() {
        *status = idle_outbound_status();
    }
}

pub(super) fn clear_incoming_status() {
    if let Ok(mut status) = incoming_status_store().lock() {
        *status = idle_incoming_status();
    }
}

pub(super) fn mark_incoming_cleanup_incomplete_status(
    session_id: &str,
    capture_potentially_active: bool,
    note: &str,
) {
    if let Ok(mut status) = incoming_status_store().lock() {
        *status = MeetingIncomingRuntimeStatus {
            session_id: Some(session_id.to_string()),
            stage: "cleanup_incomplete".to_string(),
            capture_active: capture_potentially_active,
            suppressed: false,
            degraded: true,
            blocker: "meeting_session:cleanup_incomplete".to_string(),
            note: note.to_string(),
            updated_unix_ms: unix_ms(),
            runtime_claim: "meeting_incoming_cleanup_incomplete_resource_release_not_confirmed"
                .to_string(),
        };
    }
}

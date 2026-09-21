use serde::Serialize;
use serde_json::{json, Value};

use crate::engine::runtime_state::{
    latest_runtime_session_state, runtime_generation_is_authoritative,
};

use super::helper_bridge::HelperBridgeWorkerResponse;

mod committed_turns;
mod consumer_runtime;
mod incoming_activation;
mod incoming_deferred;
mod incoming_pipeline;
mod lifecycle;
mod outbound_pipeline;
mod preflight;
mod session_state;
mod suppression;
mod transcript_export;

use committed_turns::current_committed_turn_snapshot;
use transcript_export::{
    export_transcript, transcript_export_status, MeetingTranscriptExportResult,
    MeetingTranscriptExportStatus,
};
use outbound_pipeline::process_outbound_wav;
use preflight::current_status;
use session_state::{incoming_lane_enabled, OutboundTimingContext};

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
    pub translation_tokenization_ms: Option<f64>,
    pub translation_inference_ms: Option<f64>,
    pub translation_decode_ms: Option<f64>,
    pub translation_tokens_per_second: Option<f64>,
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
    pub source_language: String,
    pub target_language: String,
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

fn worker_number(response: &HelperBridgeWorkerResponse, key: &str) -> Option<f64> {
    worker_json(response).get(key).and_then(Value::as_f64)
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
    speech_duration_ms: u64,
    audio_path: String,
    timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    process_outbound_wav(
        generation,
        session_id,
        event_sequence,
        utterance_id,
        speech_duration_ms,
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

#[tauri::command]
pub fn get_meeting_transcript_export_status() -> MeetingTranscriptExportStatus {
    transcript_export_status()
}

#[tauri::command]
pub fn export_meeting_transcript(format: String) -> MeetingTranscriptExportResult {
    export_transcript(&format)
}

pub fn start_meeting_translation() -> MeetingSessionActionResult {
    lifecycle::start_meeting_translation_impl()
}

#[tauri::command]
pub fn stop_meeting_translation() -> MeetingSessionActionResult {
    lifecycle::stop_meeting_translation_impl()
}

#[cfg(test)]
mod b3_preflight_snapshot_tests {
    use super::MeetingSessionPreflightStatus;
    use super::session_state::{
        clear_all_start_preflight, current_start_preflight, remember_start_preflight,
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
    use super::{MeetingOutboundTiming, OutboundTimingContext};
    use super::session_state::record_first_playback_timing;
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
                translation_tokenization_ms: Some(4.5),
                translation_inference_ms: Some(110.0),
                translation_decode_ms: Some(2.0),
                translation_tokens_per_second: Some(72.0),
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
    use super::lifecycle::meeting_cleanup_complete;

    #[test]
    fn cleanup_truth_requires_every_owned_resource_to_release() {
        assert!(meeting_cleanup_complete(true, true, true, true, true, true, true, true));
        assert!(!meeting_cleanup_complete(false, true, true, true, true, true, true, true));
        assert!(!meeting_cleanup_complete(true, false, true, true, true, true, true, true));
        assert!(!meeting_cleanup_complete(true, true, false, true, true, true, true, true));
        assert!(!meeting_cleanup_complete(true, true, true, false, true, true, true, true));
        assert!(!meeting_cleanup_complete(true, true, true, true, false, true, true, true));
        assert!(!meeting_cleanup_complete(true, true, true, true, true, false, true, true));
        assert!(!meeting_cleanup_complete(true, true, true, true, true, true, false, true));
        assert!(!meeting_cleanup_complete(true, true, true, true, true, true, true, false));
    }
}




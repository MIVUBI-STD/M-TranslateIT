use serde::Serialize;

use crate::engine::settings::RuntimeSettings;

use super::super::helper_bridge::HelperBridgeWorkerResponse;
use super::super::helper_bridge_runtime::HelperBridgeStatus;
use super::super::meeting_session::MeetingSessionStatus;

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
pub struct MeetingSummary {
    pub lifecycle: String,
    pub has_session: bool,
    pub application_owned: bool,
    pub ready_for_start: bool,
    pub owner_id: Option<String>,
    pub blocker: String,
    pub note: String,
    pub preflight_blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerSummary {
    pub state: String,
    pub ready: bool,
    pub degraded: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioSummary {
    pub ready: bool,
    pub running: bool,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationInputStatus {
    pub ready: bool,
    pub prepared: bool,
    pub functional_verified: bool,
    pub callback_frames_observed: u64,
    pub selected_device_name: Option<String>,
    pub input_device_name: Option<String>,
    pub device_count: u32,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationSubsystemSummaries {
    pub meeting: MeetingSummary,
    pub worker: WorkerSummary,
    pub audio: AudioSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceArbitration {
    pub microphone_available: bool,
    pub meeting_audio_available: bool,
    pub active_owner: Option<String>,
    pub blocker: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationSnapshot {
    pub revision: u64,
    pub lifecycle: String,
    pub active_owner: Option<String>,
    pub settings: RuntimeSettings,
    pub meeting: MeetingSessionStatus,
    pub helper: HelperBridgeStatus,
    pub worker: Option<HelperBridgeWorkerResponse>,
    pub input: ApplicationInputStatus,
    pub summaries: ApplicationSubsystemSummaries,
    pub resources: ResourceArbitration,
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

#[derive(Debug)]
pub struct IntentOutcome {
    pub ok: bool,
    pub state: String,
    pub message: String,
}

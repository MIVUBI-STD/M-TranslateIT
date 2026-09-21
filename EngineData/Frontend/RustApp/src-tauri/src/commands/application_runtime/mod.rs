mod capabilities;
mod contract;
mod events;
mod intents;
mod lifecycle;
mod problems;
mod resources;
mod snapshot;
mod summaries;

pub use contract::{
    ApplicationCapabilities, ApplicationInputStatus, ApplicationIntentResult, ApplicationProblem, ApplicationSnapshot,
    ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, ResourceArbitration,
    WorkerSummary,
};
pub use snapshot::current_application_snapshot;

#[tauri::command]
pub fn get_application_snapshot() -> ApplicationSnapshot {
    current_application_snapshot()
}

#[tauri::command]
pub fn dispatch_product_intent(
    app: tauri::AppHandle,
    intent: String,
) -> ApplicationIntentResult {
    let normalized = intent.trim().to_ascii_lowercase();
    let outcome = intents::dispatch(&normalized);
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, &normalized, &snapshot);

    ApplicationIntentResult {
        ok: outcome.ok,
        intent: normalized,
        state: outcome.state,
        message: outcome.message,
        snapshot,
    }
}


#[tauri::command]
pub fn select_product_audio_device(
    app: tauri::AppHandle,
    kind: String,
    device_id: Option<String>,
) -> super::settings::AudioDeviceSelectionResult {
    let result = super::settings::select_audio_device(kind, device_id);
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "select_audio_device", &snapshot);
    result
}

#[tauri::command]
pub fn start_product_voice_recording(
    app: tauri::AppHandle,
    line_id: u32,
    authorized_voice_confirmed: bool,
) -> super::voice_lab_recording::GuidedRecordingActionResult {
    let result = super::voice_lab_recording::start_voice_lab_guided_take(
        line_id,
        authorized_voice_confirmed,
    );
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "start_voice_recording", &snapshot);
    result
}

#[tauri::command]
pub fn stop_product_voice_recording(
    app: tauri::AppHandle,
    line_id: u32,
) -> super::voice_lab_recording::GuidedRecordingActionResult {
    let result = super::voice_lab_recording::stop_voice_lab_guided_take(line_id);
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "stop_voice_recording", &snapshot);
    result
}


#[tauri::command]
pub fn start_product_voice_build(
    app: tauri::AppHandle,
    authorized_voice_confirmed: bool,
) -> super::voice_lab_build::VoiceLabBuildActionResult {
    let result = super::voice_lab_build::start_voice_lab_build(authorized_voice_confirmed);
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "start_voice_build", &snapshot);
    result
}

#[tauri::command]
pub fn cancel_product_voice_build(
    app: tauri::AppHandle,
) -> super::voice_lab_build::VoiceLabBuildActionResult {
    let result = super::voice_lab_build::cancel_voice_lab_build();
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "cancel_voice_build", &snapshot);
    result
}

#[tauri::command]
pub fn approve_product_voice_candidate(
    app: tauri::AppHandle,
    reviewed_line_ids: Vec<u32>,
) -> super::voice_lab_build::VoiceLabBuildActionResult {
    let result = super::voice_lab_build::approve_voice_lab_candidate(reviewed_line_ids);
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "approve_voice_candidate", &snapshot);
    result
}

#[tauri::command]
pub fn select_product_builtin_voice(
    app: tauri::AppHandle,
    voice_id: String,
    authorized_voice_confirmed: bool,
) -> super::voice_lab_build::VoiceLabBuildActionResult {
    let result = super::voice_lab_build::select_builtin_voice(
        voice_id,
        authorized_voice_confirmed,
    );
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(&app, "select_builtin_voice", &snapshot);
    result
}

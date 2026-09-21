pub fn emit_after<T>(
    app: &tauri::AppHandle,
    reason: &str,
    action: impl FnOnce() -> T,
) -> T {
    let result = action();
    let snapshot = super::current_application_snapshot();
    super::events::emit_application_snapshot(app, reason, &snapshot);
    result
}

#[tauri::command]
pub fn select_product_audio_device(
    app: tauri::AppHandle,
    kind: String,
    device_id: Option<String>,
) -> super::super::settings::AudioDeviceSelectionResult {
    emit_after(&app, "select_audio_device", || {
        super::super::settings::select_audio_device(kind, device_id)
    })
}

#[tauri::command]
pub fn start_product_voice_recording(
    app: tauri::AppHandle,
    line_id: u32,
    authorized_voice_confirmed: bool,
) -> super::super::voice_lab_recording::GuidedRecordingActionResult {
    emit_after(&app, "start_voice_recording", || {
        super::super::voice_lab_recording::start_voice_lab_guided_take(
            line_id,
            authorized_voice_confirmed,
        )
    })
}

#[tauri::command]
pub fn stop_product_voice_recording(
    app: tauri::AppHandle,
    line_id: u32,
) -> super::super::voice_lab_recording::GuidedRecordingActionResult {
    emit_after(&app, "stop_voice_recording", || {
        super::super::voice_lab_recording::stop_voice_lab_guided_take(line_id)
    })
}

#[tauri::command]
pub fn start_product_voice_build(
    app: tauri::AppHandle,
    authorized_voice_confirmed: bool,
) -> super::super::voice_lab_build::VoiceLabBuildActionResult {
    emit_after(&app, "start_voice_build", || {
        super::super::voice_lab_build::start_voice_lab_build(authorized_voice_confirmed)
    })
}

#[tauri::command]
pub fn cancel_product_voice_build(
    app: tauri::AppHandle,
) -> super::super::voice_lab_build::VoiceLabBuildActionResult {
    emit_after(&app, "cancel_voice_build", || {
        super::super::voice_lab_build::cancel_voice_lab_build()
    })
}

#[tauri::command]
pub fn approve_product_voice_candidate(
    app: tauri::AppHandle,
    reviewed_line_ids: Vec<u32>,
) -> super::super::voice_lab_build::VoiceLabBuildActionResult {
    emit_after(&app, "approve_voice_candidate", || {
        super::super::voice_lab_build::approve_voice_lab_candidate(reviewed_line_ids)
    })
}

#[tauri::command]
pub fn select_product_builtin_voice(
    app: tauri::AppHandle,
    voice_id: String,
    authorized_voice_confirmed: bool,
) -> super::super::voice_lab_build::VoiceLabBuildActionResult {
    emit_after(&app, "select_builtin_voice", || {
        super::super::voice_lab_build::select_builtin_voice(
            voice_id,
            authorized_voice_confirmed,
        )
    })
}


#[tauri::command]
pub fn save_product_settings(
    app: tauri::AppHandle,
    settings: crate::engine::settings::RuntimeSettings,
) -> crate::engine::state::CommandResult {
    emit_after(&app, "save_settings", || {
        super::super::settings::save_runtime_settings(settings)
    })
}

#[tauri::command]
pub fn apply_product_meeting_preset(
    app: tauri::AppHandle,
    settings: crate::engine::settings::RuntimeSettings,
) -> super::super::settings::MeetingPresetApplyResult {
    emit_after(&app, "apply_meeting_preset", || {
        super::super::settings::apply_meeting_preset(settings)
    })
}

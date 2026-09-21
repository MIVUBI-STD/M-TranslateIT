use serde::Serialize;
use std::path::PathBuf;

use crate::commands::audio::{probe_input_device_candidate, probe_output_device_candidate};
use crate::commands::diagnostic_trace::{trace_command_end, trace_command_start};
use crate::engine;
use crate::engine::logging::{write_jsonl_event, RuntimeLogEvent};
use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{
    latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID,
    VOICE_RECORDING_OWNER_ID,
};
use crate::engine::settings::RuntimeSettings;
use crate::engine::state::{CommandResult, LifecycleState};

#[derive(Debug, Clone, Serialize)]
pub struct MeetingPresetApplyResult {
    pub ok: bool,
    pub message: String,
    pub settings: RuntimeSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceSelectionResult {
    pub ok: bool,
    pub kind: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub message: String,
    pub settings: RuntimeSettings,
}

fn normalized_device_id(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn persist_runtime_settings(settings: RuntimeSettings) -> CommandResult {
    let project_paths = ProjectPaths::discover();
    let settings_path =
        PathBuf::from(&project_paths.user_cache_dir).join("rust_runtime_settings.json");
    let log_dir = PathBuf::from(project_paths.user_log_dir);
    let settings = settings.sanitized();

    match settings.save_pretty(&settings_path) {
        Ok(()) => {
            let _ = write_jsonl_event(
                &log_dir,
                "rust_runtime_latest.jsonl",
                &RuntimeLogEvent::info("settings", "Runtime settings saved."),
            );
            CommandResult::ok(LifecycleState::Idle, "Runtime settings saved.")
        }
        Err(_) => {
            let _ = write_jsonl_event(
                &log_dir,
                "rust_runtime_latest.jsonl",
                &RuntimeLogEvent::warning("settings", "Runtime settings save failed."),
            );
            CommandResult::blocked(
                LifecycleState::Error,
                "Failed to save runtime settings. Open Diagnostics for details.",
            )
        }
    }
}

fn runtime_session_owns_audio_resources() -> bool {
    // `has_active_session` is deliberately fail-closed: an unreadable authority
    // store is treated as potentially owned rather than as safe-to-rebind idle state.
    latest_runtime_session_state().has_active_session
}

fn audio_resource_lock_message(action: &str) -> String {
    let runtime = latest_runtime_session_state();
    let owner = runtime
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.owner_id.as_str());

    let activity = match owner {
        Some(APPLICATION_MEETING_OWNER_ID) => "Translation",
        Some(MIC_TEST_OWNER_ID) => "Mic Test",
        Some(VOICE_RECORDING_OWNER_ID) => "My Voice recording",
        Some(_) => "the current audio action",
        None => "the current audio action",
    };

    format!("Stop {activity} before {action}. The current audio preferences were kept.")
}

fn audio_preferences_changed(current: &RuntimeSettings, candidate: &RuntimeSettings) -> bool {
    current.audio.input_device_id != candidate.audio.input_device_id
        || current.audio.output_device_id != candidate.audio.output_device_id
}

fn current_device_label(settings: &RuntimeSettings, kind: &str) -> String {
    let selected = if kind == "microphone" {
        settings.audio.input_device_id.as_deref()
    } else {
        settings.audio.output_device_id.as_deref()
    };
    selected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Windows Default")
        .to_string()
}

#[tauri::command]
pub fn load_runtime_settings() -> RuntimeSettings {
    let started = trace_command_start(
        "load_runtime_settings",
        "loading runtime settings for launcher startup",
    );
    let settings = engine::load_settings();
    trace_command_end("load_runtime_settings", started, "ok");
    settings
}

#[tauri::command]
pub fn save_runtime_settings(settings: RuntimeSettings) -> CommandResult {
    let started = trace_command_start("save_runtime_settings", "saving runtime settings");
    let current = engine::load_settings();
    let candidate = settings.sanitized();
    let result = if runtime_session_owns_audio_resources()
        && audio_preferences_changed(&current, &candidate)
    {
        CommandResult::blocked(
            LifecycleState::Error,
            audio_resource_lock_message("changing audio devices"),
        )
    } else {
        persist_runtime_settings(candidate)
    };
    trace_command_end(
        "save_runtime_settings",
        started,
        format!("state={}", result.state),
    );
    result
}

#[tauri::command]
pub fn apply_meeting_preset(settings: RuntimeSettings) -> MeetingPresetApplyResult {
    let started = trace_command_start("apply_meeting_preset", "validating and applying Meeting preset");
    let current = engine::load_settings();
    if runtime_session_owns_audio_resources() {
        trace_command_end("apply_meeting_preset", started, "active_runtime_session_locked");
        return MeetingPresetApplyResult {
            ok: false,
            message: audio_resource_lock_message("applying a Meeting preset")
                .replace("audio preferences", "setup"),
            settings: current,
        };
    }

    let candidate = settings.sanitized();
    let input_probe = probe_input_device_candidate(candidate.audio.input_device_id.clone());
    if !(input_probe.prepared && input_probe.functional_verified) {
        trace_command_end("apply_meeting_preset", started, "microphone_unavailable");
        return MeetingPresetApplyResult {
            ok: false,
            message: "Preset not applied: the saved microphone is unavailable. The current setup was kept.".to_string(),
            settings: current,
        };
    }

    let output_probe = probe_output_device_candidate(candidate.audio.output_device_id.clone());
    if !output_probe.ok {
        trace_command_end("apply_meeting_preset", started, "meeting_sound_unavailable");
        return MeetingPresetApplyResult {
            ok: false,
            message: "Preset not applied: the saved Meeting Sound device is unavailable. The current setup was kept.".to_string(),
            settings: current,
        };
    }

    let save = persist_runtime_settings(candidate);
    if !save.ok {
        trace_command_end("apply_meeting_preset", started, "save_failed");
        return MeetingPresetApplyResult {
            ok: false,
            message: "The preset was validated, but it could not be saved. The previous setup was kept.".to_string(),
            settings: current,
        };
    }

    let saved = engine::load_settings();
    trace_command_end("apply_meeting_preset", started, "ok");
    MeetingPresetApplyResult {
        ok: true,
        message: "Meeting preset applied.".to_string(),
        settings: saved,
    }
}

#[tauri::command]
pub fn select_audio_device(kind: String, device_id: Option<String>) -> AudioDeviceSelectionResult {
    let started = trace_command_start("select_audio_device", format!("kind={kind}"));
    let current = engine::load_settings();
    let requested = normalized_device_id(device_id);

    if !matches!(kind.as_str(), "microphone" | "meeting-sound") {
        trace_command_end("select_audio_device", started, "invalid_kind");
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name: "Unknown device".to_string(),
            message: "That audio device type is not supported.".to_string(),
            settings: current,
        };
    }

    // A Meeting session and Mic Test own live audio resources until their canonical
    // Stop path releases the runtime session. Persisting a different device while a
    // stream still owns the previous endpoint would make Settings disagree with the
    // active runtime. Defer the change instead of attempting a mid-session hot rebind.
    if runtime_session_owns_audio_resources() {
        trace_command_end(
            "select_audio_device",
            started,
            "active_runtime_session_locked",
        );
        return AudioDeviceSelectionResult {
            ok: false,
            kind: kind.clone(),
            device_id: requested,
            device_name: current_device_label(&current, &kind),
            message: audio_resource_lock_message("changing audio devices")
                .replace("audio preferences", "preference"),
            settings: current,
        };
    }

    let (available, device_name) = match kind.as_str() {
        "microphone" => {
            let probe = probe_input_device_candidate(requested.clone());
            (
                probe.prepared && probe.functional_verified,
                probe
                    .input_device_name
                    .clone()
                    .or_else(|| requested.clone())
                    .unwrap_or_else(|| "Windows Default".to_string()),
            )
        }
        "meeting-sound" => {
            let probe = probe_output_device_candidate(requested.clone());
            (
                probe.ok,
                probe
                    .resolved_device_name
                    .clone()
                    .or_else(|| requested.clone())
                    .unwrap_or_else(|| "Windows Default".to_string()),
            )
        }
        _ => unreachable!("audio device kind validated above"),
    };

    if !available {
        trace_command_end("select_audio_device", started, "candidate_unavailable");
        let message = if kind == "microphone" {
            "This microphone can't be used right now. Choose another microphone or Windows Default. The previous preference was kept."
        } else {
            "This meeting sound device can't be used right now. Choose another device or Windows Default. The previous preference was kept."
        };
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name,
            message: message.to_string(),
            settings: current,
        };
    }

    let mut candidate = current.clone();
    if kind == "microphone" {
        candidate.audio.input_device_id = requested.clone();
    } else {
        candidate.audio.output_device_id = requested.clone();
    }

    let save = persist_runtime_settings(candidate);
    if !save.ok {
        trace_command_end("select_audio_device", started, "save_failed");
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name,
            message: "The device is available, but the preference could not be saved. The previous device preference was kept."
                .to_string(),
            settings: current,
        };
    }

    let saved = engine::load_settings();
    let label = if kind == "microphone" {
        "Microphone"
    } else {
        "Meeting sound"
    };
    trace_command_end("select_audio_device", started, "ok");
    AudioDeviceSelectionResult {
        ok: true,
        kind,
        device_id: requested,
        device_name: device_name.clone(),
        message: format!("{label} set to {device_name}."),
        settings: saved,
    }
}

#[cfg(test)]
mod tests {
    use super::audio_preferences_changed;
    use crate::engine::settings::RuntimeSettings;

    #[test]
    fn audio_preference_change_detects_only_audio_drift() {
        let current = RuntimeSettings::default();
        let mut language_only = current.clone();
        language_only.source_language = "en".to_string();
        language_only.target_language = "id".to_string();
        assert!(!audio_preferences_changed(&current, &language_only));

        let mut input_change = current.clone();
        input_change.audio.input_device_id = Some("Another Microphone".to_string());
        assert!(audio_preferences_changed(&current, &input_change));

        let mut output_change = current.clone();
        output_change.audio.output_device_id = Some("Another Meeting Sound".to_string());
        assert!(audio_preferences_changed(&current, &output_change));
    }
}

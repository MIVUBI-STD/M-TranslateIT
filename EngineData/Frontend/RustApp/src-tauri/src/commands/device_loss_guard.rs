use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::audio::live_capture::live_capture_status;
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;
use crate::engine::runtime_settings::load_settings;

use super::audio::list_audio_devices;
use super::incident_log::record_runtime_incident;
use super::meeting_session::get_meeting_session_status;
use super::virtual_mic_route::get_virtual_mic_route_selection;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceLossGuardStatus {
    pub state: String,
    pub healthy: bool,
    pub action_required: bool,
    pub required_device_lost: bool,
    pub optional_device_lost: bool,
    pub component: String,
    pub blocker: String,
    pub note: String,
    pub updated_unix_ms: u128,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn status(
    state: &str,
    healthy: bool,
    action_required: bool,
    required_device_lost: bool,
    optional_device_lost: bool,
    component: &str,
    blocker: &str,
    note: &str,
) -> DeviceLossGuardStatus {
    DeviceLossGuardStatus {
        state: state.to_string(),
        healthy,
        action_required,
        required_device_lost,
        optional_device_lost,
        component: component.to_string(),
        blocker: blocker.to_string(),
        note: note.to_string(),
        updated_unix_ms: unix_ms(),
    }
}

fn selected_device_present(selected: Option<&str>, names: impl Iterator<Item = String>) -> bool {
    let Some(selected) = selected.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    names.any(|name| name == selected)
}

fn record_if_needed(status: DeviceLossGuardStatus) -> DeviceLossGuardStatus {
    if !status.healthy && !status.blocker.is_empty() {
        let _ = record_runtime_incident(
            "device_loss",
            &status.component,
            &status.blocker,
            &status.note,
        );
    }
    status
}

#[tauri::command]
pub fn get_device_loss_guard_status() -> DeviceLossGuardStatus {
    let meeting = get_meeting_session_status();
    if !meeting.has_session {
        return record_if_needed(status(
            "idle",
            true,
            false,
            false,
            false,
            "",
            "",
            "No Meeting session is active, so no live device ownership can be lost.",
        ));
    }

    let settings = load_settings();
    let devices = list_audio_devices();
    let live = live_capture_status();
    let incoming = meeting_sound_capture_status();
    let route = get_virtual_mic_route_selection();

    if !live.stream_active {
        return record_if_needed(status(
            "required_device_lost",
            false,
            true,
            true,
            false,
            "microphone",
            "device_loss:microphone_capture_inactive",
            "The required microphone capture is no longer active. TranslateIT will not silently switch microphones; stop Translation and reconnect or reselect the device.",
        ));
    }
    if live.callback_error_count > 0 {
        return record_if_needed(status(
            "required_device_lost",
            false,
            true,
            true,
            false,
            "microphone",
            "device_loss:microphone_callback_error",
            "The required microphone stream reported a native callback error. TranslateIT will not silently switch devices; stop Translation before recovery.",
        ));
    }

    if settings.audio.input_device_id.is_some()
        && !selected_device_present(
            settings.audio.input_device_id.as_deref(),
            devices.input_devices.iter().map(|device| device.name.clone()),
        )
    {
        return record_if_needed(status(
            "required_device_lost",
            false,
            true,
            true,
            false,
            "microphone",
            "device_loss:selected_microphone_missing",
            "The explicitly selected microphone is no longer present. TranslateIT did not substitute the Windows default device.",
        ));
    }

    if !route.route_ready {
        return record_if_needed(status(
            "required_device_lost",
            false,
            true,
            true,
            false,
            "virtual_microphone",
            "device_loss:virtual_microphone_route_missing",
            "The bound TranslateIT virtual microphone route is no longer available. Required outbound voice delivery must not silently move to another endpoint.",
        ));
    }

    let explicit_meeting_sound_missing = settings.audio.output_device_id.is_some()
        && !selected_device_present(
            settings.audio.output_device_id.as_deref(),
            devices.output_devices.iter().map(|device| device.name.clone()),
        );
    if explicit_meeting_sound_missing {
        return record_if_needed(status(
            "optional_device_lost",
            false,
            false,
            false,
            true,
            "meeting_sound",
            "device_loss:selected_meeting_sound_missing",
            "The explicitly selected Meeting Sound endpoint is no longer present. Incoming captions may be unavailable, but required outbound translation remains authoritative.",
        ));
    }

    if incoming.stream_active && incoming.callback_error_count > 0 {
        return record_if_needed(status(
            "optional_device_lost",
            false,
            false,
            false,
            true,
            "meeting_sound",
            "device_loss:meeting_sound_callback_error",
            "Meeting Sound capture reported a native callback error. Incoming translation is optional; required outbound translation remains authoritative.",
        ));
    }

    if !devices.ok {
        return record_if_needed(status(
            "unverified",
            false,
            false,
            false,
            false,
            "audio_host",
            "device_loss:enumeration_unavailable",
            "Native audio-device enumeration is temporarily unavailable. Existing active streams are left untouched; no fallback device was selected.",
        ));
    }

    record_if_needed(status(
        "healthy",
        true,
        false,
        false,
        false,
        "",
        "",
        "Required Meeting audio ownership is still present and no native capture error is reported.",
    ))
}

#[cfg(test)]
mod tests {
    use super::selected_device_present;

    #[test]
    fn default_device_selection_does_not_require_name_identity() {
        assert!(selected_device_present(None, Vec::<String>::new().into_iter()));
    }

    #[test]
    fn explicit_device_requires_exact_presence() {
        assert!(selected_device_present(
            Some("Mic A"),
            vec!["Mic A".to_string(), "Mic B".to_string()].into_iter(),
        ));
        assert!(!selected_device_present(
            Some("Mic A"),
            vec!["Mic B".to_string()].into_iter(),
        ));
    }
}

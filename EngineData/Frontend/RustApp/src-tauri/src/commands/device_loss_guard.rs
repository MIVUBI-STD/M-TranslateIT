use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::engine::audio::live_capture::live_capture_status;
use crate::engine::audio::meeting_output::original_voice_output_healthy;
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;
use crate::engine::runtime_settings::load_settings;
use crate::engine::runtime_state::APPLICATION_MEETING_OWNER_ID;

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

const CAPTURE_STALL_THRESHOLD: Duration = Duration::from_secs(24);

#[derive(Debug)]
struct CaptureProgress {
    session_id: String,
    stream_started_unix_ms: u128,
    frames_received: u64,
    last_progress: Instant,
}

static CAPTURE_PROGRESS: OnceLock<Mutex<Option<CaptureProgress>>> = OnceLock::new();

fn capture_progress_store() -> &'static Mutex<Option<CaptureProgress>> {
    CAPTURE_PROGRESS.get_or_init(|| Mutex::new(None))
}

// Native PCM frames progress even while a user is silent. No timestamps are
// acquired on the realtime audio callback; only the slow health reader samples.
fn capture_frames_stalled(
    state: &mut Option<CaptureProgress>,
    session_id: &str,
    started_unix_ms: u128,
    frames_received: u64,
    now: Instant,
) -> bool {
    let reset = match state {
        Some(previous) => previous.session_id != session_id
            || previous.stream_started_unix_ms != started_unix_ms
            || frames_received < previous.frames_received,
        None => true,
    };
    if reset {
        *state = Some(CaptureProgress {
            session_id: session_id.to_string(),
            stream_started_unix_ms: started_unix_ms,
            frames_received,
            last_progress: now,
        });
        return false;
    }
    let previous = state.as_mut().expect("progress state was initialized");
    if frames_received > previous.frames_received {
        previous.frames_received = frames_received;
        previous.last_progress = now;
        return false;
    }
    now.saturating_duration_since(previous.last_progress) >= CAPTURE_STALL_THRESHOLD
}

fn clear_capture_progress() {
    if let Ok(mut state) = capture_progress_store().lock() {
        *state = None;
    }
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

fn selected_device_present(selected: Option<&str>, mut names: impl Iterator<Item = String>) -> bool {
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
    if !meeting.has_session
        || meeting.owner_id.as_deref() != Some(APPLICATION_MEETING_OWNER_ID)
    {
        clear_capture_progress();
        return record_if_needed(status(
            "idle",
            true,
            false,
            false,
            false,
            "",
            "",
            "No application Meeting session is active, so Meeting device-loss checks are idle.",
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

    // A native stream can stall with no CPAL callback error.
    if let (Some(session_id), Some(started)) =
        (live.session_id.as_deref(), live.started_unix_ms)
    {
        let stalled = match capture_progress_store().lock() {
            Ok(mut state) => capture_frames_stalled(
                &mut state, session_id, started, live.frames_received, Instant::now(),
            ),
            Err(_) => {
                return record_if_needed(status(
                    "unverified", false, true, true, false, "microphone",
                    "device_loss:microphone_progress_unavailable",
                    "Microphone callback progress could not be inspected safely.",
                ));
            }
        };
        if stalled {
            return record_if_needed(status(
                "required_device_lost", false, true, true, false, "microphone",
                "device_loss:microphone_callback_stalled",
                "The microphone stream is open but native frames have stopped for at least 24 seconds. Stop Translation and recover the selected microphone; no fallback was chosen.",
            ));
        }
    } else {
        return record_if_needed(status(
            "unverified", false, true, true, false, "microphone",
            "device_loss:microphone_progress_identity_missing",
            "Active microphone capture is missing its session/start identity.",
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
    // Windows may retain a listed virtual endpoint even after CPAL output
    // callbacks fail. Original voice must not be reported healthy in that case.
    if meeting.audio_source == "original" {
        match meeting.generation.and_then(original_voice_output_healthy) {
            Some(true) => {}
            Some(false) => return record_if_needed(status(
                "required_device_lost", false, true, true, false,
                "virtual_microphone", "device_loss:original_output_callback_failed",
                "Original Microphone output reported a native stream failure. Stop Microphone to recover; no alternate audio source was selected.",
            )),
            None => return record_if_needed(status(
                "unverified", false, true, true, false,
                "virtual_microphone", "device_loss:original_output_unverified",
                "Original Microphone output cannot be verified. Stop Microphone before attempting recovery.",
            )),
        }
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
    use super::{capture_frames_stalled, selected_device_present, CaptureProgress, CAPTURE_STALL_THRESHOLD};
    use std::time::{Duration, Instant};

    #[test]
    fn silent_pcm_counts_as_callback_progress_but_no_frames_stalls() {
        let now = Instant::now();
        let mut state: Option<CaptureProgress> = None;
        assert!(!capture_frames_stalled(&mut state, "session-a", 100, 0, now));
        assert!(!capture_frames_stalled(
            &mut state, "session-a", 100, 48_000, now + Duration::from_secs(8),
        ));
        assert!(!capture_frames_stalled(
            &mut state, "session-a", 100, 48_000,
            now + Duration::from_secs(8) + CAPTURE_STALL_THRESHOLD - Duration::from_millis(1),
        ));
        assert!(capture_frames_stalled(
            &mut state, "session-a", 100, 48_000,
            now + Duration::from_secs(8) + CAPTURE_STALL_THRESHOLD,
        ));
        assert!(!capture_frames_stalled(
            &mut state, "session-a", 100, 48_001, now + Duration::from_secs(40),
        ));
    }

    #[test]
    fn new_session_and_new_stream_reset_liveness_tracking() {
        let now = Instant::now();
        let mut state: Option<CaptureProgress> = None;
        assert!(!capture_frames_stalled(&mut state, "a", 100, 500, now));
        assert!(!capture_frames_stalled(
            &mut state, "b", 100, 0, now + Duration::from_secs(60),
        ));
        assert!(!capture_frames_stalled(
            &mut state, "b", 200, 0, now + Duration::from_secs(120),
        ));
    }

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

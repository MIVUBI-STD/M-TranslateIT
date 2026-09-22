use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

pub const MEETING_RUNTIME_EVENT: &str = "translateit://meeting-runtime";
pub const VOICE_BUILD_RUNTIME_EVENT: &str = "translateit://voice-build-runtime";

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static MEETING_EVENT_REVISION: AtomicU64 = AtomicU64::new(0);
static VOICE_BUILD_EVENT_REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct MeetingRuntimeEvent {
    pub revision: u64,
    pub reason: String,
    pub session_id: Option<String>,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceBuildRuntimeEvent {
    pub revision: u64,
    pub reason: String,
    pub generation: u64,
}

pub fn install_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

pub fn emit_meeting_runtime_event(
    reason: &str,
    session_id: Option<&str>,
    sequence: Option<u64>,
) {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    let event = MeetingRuntimeEvent {
        revision: MEETING_EVENT_REVISION.fetch_add(1, Ordering::AcqRel) + 1,
        reason: reason.to_string(),
        session_id: session_id.map(str::to_string),
        sequence,
    };
    let _ = app.emit(MEETING_RUNTIME_EVENT, event);
}

pub fn emit_voice_build_runtime_event(reason: &str, generation: u64) {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    let event = VoiceBuildRuntimeEvent {
        revision: VOICE_BUILD_EVENT_REVISION.fetch_add(1, Ordering::AcqRel) + 1,
        reason: reason.to_string(),
        generation,
    };
    let _ = app.emit(VOICE_BUILD_RUNTIME_EVENT, event);
}

#[cfg(test)]
mod tests {
    use super::{MEETING_RUNTIME_EVENT, VOICE_BUILD_RUNTIME_EVENT};

    #[test]
    fn meeting_runtime_event_name_is_stable() {
        assert_eq!(MEETING_RUNTIME_EVENT, "translateit://meeting-runtime");
    }

    #[test]
    fn voice_build_runtime_event_name_is_stable() {
        assert_eq!(VOICE_BUILD_RUNTIME_EVENT, "translateit://voice-build-runtime");
    }
}

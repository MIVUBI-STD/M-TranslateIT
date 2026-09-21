use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::logging::sanitize_diagnostic_text;
use crate::engine::paths::ProjectPaths;

const INCIDENT_FILE: &str = "runtime_incidents.json";
const MAX_INCIDENTS: usize = 20;
const INCIDENT_REPEAT_SUPPRESSION_MS: u128 = 60_000;
static INCIDENT_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

thread_local! {
    static INCIDENT_RECORDING_SUPPRESSED: Cell<bool> = const { Cell::new(false) };
}

struct IncidentRecordingSuppressionGuard {
    previous: bool,
}

impl Drop for IncidentRecordingSuppressionGuard {
    fn drop(&mut self) {
        INCIDENT_RECORDING_SUPPRESSED.with(|flag| flag.set(self.previous));
    }
}

pub(super) fn without_runtime_incident_recording<T>(action: impl FnOnce() -> T) -> T {
    let previous = INCIDENT_RECORDING_SUPPRESSED.with(|flag| flag.replace(true));
    let _guard = IncidentRecordingSuppressionGuard { previous };
    action()
}

fn incident_recording_suppressed() -> bool {
    INCIDENT_RECORDING_SUPPRESSED.with(Cell::get)
}

fn incident_io_lock() -> &'static Mutex<()> {
    INCIDENT_IO_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeIncident {
    pub category: String,
    pub component: String,
    pub blocker: String,
    pub note: String,
    pub occurred_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeIncidentSnapshot {
    pub incidents: Vec<RuntimeIncident>,
    pub count: usize,
    pub truncated: bool,
    pub note: String,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn incident_path() -> PathBuf {
    let paths = ProjectPaths::discover();
    PathBuf::from(paths.user_log_dir).join(INCIDENT_FILE)
}

fn read_incidents(path: &Path) -> Vec<RuntimeIncident> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<RuntimeIncident>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .take(MAX_INCIDENTS)
        .collect()
}

fn write_incidents(path: &Path, incidents: &[RuntimeIncident]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "incident path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(incidents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&temp, body)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp, path)
}

pub fn record_runtime_incident(
    category: &str,
    component: &str,
    blocker: &str,
    note: &str,
) -> bool {
    if incident_recording_suppressed() {
        return true;
    }

    let category = sanitize_diagnostic_text(category);
    let component = sanitize_diagnostic_text(component);
    let blocker = sanitize_diagnostic_text(blocker);
    let note = sanitize_diagnostic_text(note);
    if blocker.trim().is_empty() {
        return false;
    }

    let Ok(_io_guard) = incident_io_lock().lock() else {
        return false;
    };
    let path = incident_path();
    let mut incidents = read_incidents(&path);
    let next = RuntimeIncident {
        category,
        component,
        blocker,
        note,
        occurred_unix_ms: unix_ms(),
    };

    if let Some(index) = incidents.iter().position(|current| {
        current.category == next.category
            && current.component == next.component
            && current.blocker == next.blocker
    }) {
        let previous = &incidents[index];
        if next.occurred_unix_ms.saturating_sub(previous.occurred_unix_ms)
            < INCIDENT_REPEAT_SUPPRESSION_MS
        {
            return true;
        }
        incidents.remove(index);
    }

    incidents.insert(0, next);
    incidents.truncate(MAX_INCIDENTS);
    write_incidents(&path, &incidents).is_ok()
}

#[tauri::command]
pub fn runtime_incident_count() -> usize {
    let Ok(_io_guard) = incident_io_lock().lock() else {
        return 0;
    };
    read_incidents(&incident_path()).len()
}

#[tauri::command]
pub fn get_recent_runtime_incidents() -> RuntimeIncidentSnapshot {
    let incidents = incident_io_lock()
        .lock()
        .ok()
        .map(|_guard| read_incidents(&incident_path()))
        .unwrap_or_default();
    RuntimeIncidentSnapshot {
        count: incidents.len(),
        truncated: incidents.len() >= MAX_INCIDENTS,
        incidents,
        note: "Recent incidents contain redacted runtime failure metadata only; conversation text and audio are never stored here.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        incident_recording_suppressed, without_runtime_incident_recording, RuntimeIncident,
        INCIDENT_REPEAT_SUPPRESSION_MS, MAX_INCIDENTS,
    };

    fn same_incident_at(occurred_unix_ms: u128) -> RuntimeIncident {
        RuntimeIncident {
            category: "runtime_watchdog".to_string(),
            component: "meeting_outbound".to_string(),
            blocker: "runtime_watchdog:outbound_progress_stalled".to_string(),
            note: "stalled".to_string(),
            occurred_unix_ms,
        }
    }


    #[test]
    fn incident_recording_suppression_is_scoped_and_restored() {
        assert!(!incident_recording_suppressed());
        let suppressed = without_runtime_incident_recording(incident_recording_suppressed);
        assert!(suppressed);
        assert!(!incident_recording_suppressed());
    }

    #[test]
    fn incident_bound_remains_small() {
        assert_eq!(MAX_INCIDENTS, 20);
        assert!(std::mem::size_of::<RuntimeIncident>() < 256);
    }

    #[test]
    fn repeat_suppression_window_covers_live_poll_cadence() {
        assert!(INCIDENT_REPEAT_SUPPRESSION_MS >= 30_000);
        assert!(INCIDENT_REPEAT_SUPPRESSION_MS <= 300_000);
    }

    #[test]
    fn same_incident_identity_is_searchable_beyond_front_entry() {
        let incidents = [
            RuntimeIncident {
                category: "device_loss".to_string(),
                component: "microphone".to_string(),
                blocker: "device_loss:microphone_capture_inactive".to_string(),
                note: "lost".to_string(),
                occurred_unix_ms: 2,
            },
            same_incident_at(1),
        ];
        let next = same_incident_at(10);
        let index = incidents.iter().position(|current| {
            current.category == next.category
                && current.component == next.component
                && current.blocker == next.blocker
        });
        assert_eq!(index, Some(1));
    }
}

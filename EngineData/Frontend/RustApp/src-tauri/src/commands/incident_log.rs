use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::logging::sanitize_diagnostic_text;
use crate::engine::paths::ProjectPaths;

const INCIDENT_FILE: &str = "runtime_incidents.json";
const MAX_INCIDENTS: usize = 20;

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
    fs::rename(temp, path)
}

pub fn record_runtime_incident(
    category: &str,
    component: &str,
    blocker: &str,
    note: &str,
) -> bool {
    let category = sanitize_diagnostic_text(category);
    let component = sanitize_diagnostic_text(component);
    let blocker = sanitize_diagnostic_text(blocker);
    let note = sanitize_diagnostic_text(note);
    if blocker.trim().is_empty() {
        return false;
    }

    let path = incident_path();
    let mut incidents = read_incidents(&path);
    let next = RuntimeIncident {
        category,
        component,
        blocker,
        note,
        occurred_unix_ms: unix_ms(),
    };

    if incidents.first().is_some_and(|current| {
        current.category == next.category
            && current.component == next.component
            && current.blocker == next.blocker
    }) {
        incidents[0] = next;
    } else {
        incidents.insert(0, next);
        incidents.truncate(MAX_INCIDENTS);
    }
    write_incidents(&path, &incidents).is_ok()
}

#[tauri::command]
pub fn get_recent_runtime_incidents() -> RuntimeIncidentSnapshot {
    let incidents = read_incidents(&incident_path());
    RuntimeIncidentSnapshot {
        count: incidents.len(),
        truncated: incidents.len() >= MAX_INCIDENTS,
        incidents,
        note: "Recent incidents contain redacted runtime failure metadata only; conversation text and audio are never stored here.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeIncident, MAX_INCIDENTS};

    #[test]
    fn incident_bound_remains_small() {
        assert_eq!(MAX_INCIDENTS, 20);
        assert!(std::mem::size_of::<RuntimeIncident>() < 256);
    }
}

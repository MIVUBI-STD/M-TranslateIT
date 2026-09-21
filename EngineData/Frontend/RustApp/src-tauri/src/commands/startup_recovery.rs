use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};

use crate::engine::paths::ProjectPaths;

use super::incident_log::record_runtime_incident;

const MARKER_DIR: &str = "runtime_sessions";
const MAX_RECOVERY_BLOCKERS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeOpenMarker {
    process_id: u32,
    started_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupRecoveryReport {
    pub previous_unclean_shutdown: bool,
    pub another_instance_detected: bool,
    pub cleanup_attempted: bool,
    pub cleanup_ok: bool,
    pub removed_files: usize,
    pub blocker: String,
    pub note: String,
    pub checked_unix_ms: u128,
}

static STARTUP_RECOVERY_REPORT: OnceLock<Mutex<StartupRecoveryReport>> = OnceLock::new();

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn default_report() -> StartupRecoveryReport {
    StartupRecoveryReport {
        previous_unclean_shutdown: false,
        another_instance_detected: false,
        cleanup_attempted: false,
        cleanup_ok: true,
        removed_files: 0,
        blocker: String::new(),
        note: "Startup recovery has not detected an interrupted previous run.".to_string(),
        checked_unix_ms: unix_ms(),
    }
}

fn report_store() -> &'static Mutex<StartupRecoveryReport> {
    STARTUP_RECOVERY_REPORT.get_or_init(|| Mutex::new(default_report()))
}

fn marker_dir(paths: &ProjectPaths) -> PathBuf {
    PathBuf::from(&paths.user_log_dir).join(MARKER_DIR)
}

fn marker_path(paths: &ProjectPaths, process_id: u32) -> PathBuf {
    marker_dir(paths).join(format!("runtime_{process_id}.json"))
}

fn marker_matches_process_start(marker_started_unix_ms: u128, process_started_unix_s: u64) -> bool {
    let process_started_unix_ms = u128::from(process_started_unix_s).saturating_mul(1_000);
    process_started_unix_ms <= marker_started_unix_ms.saturating_add(1_000)
}

fn marker_process_is_alive(system: &System, marker: &RuntimeOpenMarker) -> bool {
    if marker.process_id == 0 {
        return false;
    }
    system
        .process(Pid::from_u32(marker.process_id))
        .is_some_and(|process| marker_matches_process_start(marker.started_unix_ms, process.start_time()))
}

fn read_marker(path: &Path) -> Option<RuntimeOpenMarker> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<RuntimeOpenMarker>(&bytes).ok()
}

fn write_marker(path: &Path, marker: &RuntimeOpenMarker) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "startup marker has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension("json.tmp");
    let body = serde_json::to_vec(marker)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&temp, body)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp, path)
}

fn remove_matching_files(
    directory: &Path,
    predicate: impl Fn(&str) -> bool,
) -> Result<usize, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(format!("read_dir:{}", directory.display())),
    };
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| format!("read_entry:{}", directory.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|_| format!("file_type:{}", directory.display()))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !predicate(&name) {
            continue;
        }
        fs::remove_file(entry.path()).map_err(|_| format!("remove_file:{name}"))?;
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}

fn cleanup_interrupted_meeting_cache(paths: &ProjectPaths) -> (usize, Vec<String>) {
    let cache = PathBuf::from(&paths.user_cache_dir);
    let mut removed = 0usize;
    let mut blockers = Vec::new();

    let jobs = [
        (
            cache.join("audio_segments"),
            Box::new(|name: &str| {
                name.starts_with("final_") && (name.ends_with(".wav") || name.ends_with(".wav.tmp"))
            }) as Box<dyn Fn(&str) -> bool>,
        ),
        (
            cache.join("meeting_tts"),
            Box::new(|name: &str| name.ends_with(".wav") || name.ends_with(".wav.tmp"))
                as Box<dyn Fn(&str) -> bool>,
        ),
        (
            cache.join("helper_functional_readiness"),
            Box::new(|name: &str| {
                matches!(
                    name,
                    "required_outbound_myvoice.wav"
                        | "diagnostic_myvoice.wav"
                        | "required_outbound_myvoice.wav.tmp"
                        | "diagnostic_myvoice.wav.tmp"
                )
            }) as Box<dyn Fn(&str) -> bool>,
        ),
    ];

    for (directory, predicate) in jobs {
        match remove_matching_files(&directory, predicate) {
            Ok(count) => removed = removed.saturating_add(count),
            Err(blocker) if blockers.len() < MAX_RECOVERY_BLOCKERS => blockers.push(blocker),
            Err(_) => {}
        }
    }

    (removed, blockers)
}

fn inspect_previous_markers(
    paths: &ProjectPaths,
    current_pid: u32,
    system: &System,
) -> (bool, bool) {
    let Ok(entries) = fs::read_dir(marker_dir(paths)) else {
        return (false, false);
    };
    let mut stale_found = false;
    let mut live_other_found = false;

    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(marker) = read_marker(&path) else {
            let _ = fs::remove_file(path);
            continue;
        };
        if marker.process_id == current_pid {
            stale_found = true;
            let _ = fs::remove_file(path);
            continue;
        }
        if marker_process_is_alive(system, &marker) {
            live_other_found = true;
        } else {
            stale_found = true;
            let _ = fs::remove_file(path);
        }
    }
    (stale_found, live_other_found)
}

pub fn begin_startup_recovery(paths: &ProjectPaths) -> io::Result<StartupRecoveryReport> {
    let current_pid = std::process::id();
    let system = System::new_all();
    let (stale_found, live_other_found) = inspect_previous_markers(paths, current_pid, &system);

    let mut report = default_report();
    report.previous_unclean_shutdown = stale_found;
    report.another_instance_detected = live_other_found;

    if stale_found && !live_other_found {
        report.cleanup_attempted = true;
        let (removed, blockers) = cleanup_interrupted_meeting_cache(paths);
        report.removed_files = removed;
        report.cleanup_ok = blockers.is_empty();
        if blockers.is_empty() {
            report.note = format!(
                "Recovered from an interrupted previous run. Removed {removed} stale Meeting cache file(s); no Meeting session was resumed."
            );
        } else {
            report.blocker = "startup_recovery:cleanup_incomplete".to_string();
            report.note = format!(
                "Previous run ended unexpectedly. Recovery removed {removed} stale Meeting cache file(s), but some cleanup could not be confirmed: {}",
                blockers.join(", ")
            );
        }
    } else if stale_found && live_other_found {
        report.cleanup_ok = true;
        report.note = "An interrupted previous run was detected, but another TranslateIT process is still active. Shared Meeting cache was left untouched to avoid interfering with that live instance.".to_string();
    } else if live_other_found {
        report.note = "Another TranslateIT process appears to be running. This instance owns a separate startup marker and did not modify the other process's Meeting cache.".to_string();
    }

    write_marker(
        &marker_path(paths, current_pid),
        &RuntimeOpenMarker {
            process_id: current_pid,
            started_unix_ms: unix_ms(),
        },
    )?;

    report.checked_unix_ms = unix_ms();
    if report.previous_unclean_shutdown {
        let blocker = if report.blocker.is_empty() {
            "startup_recovery:previous_unclean_shutdown"
        } else {
            report.blocker.as_str()
        };
        let _ = record_runtime_incident("startup_recovery", "application", blocker, &report.note);
    }
    if let Ok(mut guard) = report_store().lock() {
        *guard = report.clone();
    }
    Ok(report)
}

pub fn mark_clean_shutdown() -> bool {
    let paths = ProjectPaths::discover();
    let path = marker_path(&paths, std::process::id());
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

#[tauri::command]
pub fn get_startup_recovery_status() -> StartupRecoveryReport {
    report_store()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| StartupRecoveryReport {
            cleanup_ok: false,
            blocker: "startup_recovery:state_lock_failed".to_string(),
            note: "Startup recovery status is unavailable.".to_string(),
            ..default_report()
        })
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_interrupted_meeting_cache, marker_matches_process_start, marker_path, ProjectPaths,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths() -> ProjectPaths {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("translateit-startup-recovery-{nonce}"));
        let cache = root.join("CacheData");
        let log = root.join("LogData");
        let saved = root.join("SavedProject");
        fs::create_dir_all(&cache).expect("create cache");
        fs::create_dir_all(&log).expect("create log");
        fs::create_dir_all(&saved).expect("create saved");
        ProjectPaths {
            project_root: root.to_string_lossy().into_owned(),
            runtime_root: root.to_string_lossy().into_owned(),
            worker_runtime_dir: root.join("worker").to_string_lossy().into_owned(),
            python_runtime_dir: root.join("python").to_string_lossy().into_owned(),
            user_data_root: root.to_string_lossy().into_owned(),
            user_cache_dir: cache.to_string_lossy().into_owned(),
            user_log_dir: log.to_string_lossy().into_owned(),
            user_saved_dir: saved.to_string_lossy().into_owned(),
            asr_model_dir: root.join("asr").to_string_lossy().into_owned(),
            translation_model_dir: root.join("translation").to_string_lossy().into_owned(),
            voice_runtime_dir: root.join("voice").to_string_lossy().into_owned(),
            backend_contract_dir: root.join("contract").to_string_lossy().into_owned(),
            path_mode: "test".to_string(),
            packaged_context_initialized: false,
            development_root_verified: false,
            discovery_note: "test".to_string(),
        }
    }

    #[test]
    fn cleanup_is_scoped_to_known_meeting_cache_namespaces() {
        let paths = temp_paths();
        let cache = std::path::PathBuf::from(&paths.user_cache_dir);
        let audio = cache.join("audio_segments");
        let tts = cache.join("meeting_tts");
        let readiness = cache.join("helper_functional_readiness");
        fs::create_dir_all(&audio).expect("audio dir");
        fs::create_dir_all(&tts).expect("tts dir");
        fs::create_dir_all(&readiness).expect("readiness dir");

        fs::write(audio.join("final_session_s1_you_g1_u1.wav"), b"x").expect("meeting audio");
        fs::write(audio.join("keep_reference.wav"), b"x").expect("foreign audio");
        fs::write(tts.join("session_g1_s1.wav"), b"x").expect("meeting tts");
        fs::write(readiness.join("required_outbound_myvoice.wav"), b"x").expect("probe");
        fs::write(readiness.join("keep.txt"), b"x").expect("foreign readiness");

        let (removed, blockers) = cleanup_interrupted_meeting_cache(&paths);
        assert_eq!(removed, 3);
        assert!(blockers.is_empty());
        assert!(audio.join("keep_reference.wav").exists());
        assert!(readiness.join("keep.txt").exists());

        let _ = fs::remove_dir_all(&paths.user_data_root);
    }


    #[test]
    fn marker_rejects_pid_reuse_by_a_newer_process() {
        assert!(marker_matches_process_start(10_500, 10));
        assert!(marker_matches_process_start(10_000, 11));
        assert!(!marker_matches_process_start(10_000, 12));
    }

    #[test]
    fn markers_are_pid_scoped_under_log_data() {
        let paths = temp_paths();
        let marker = marker_path(&paths, 4242);
        assert!(marker.starts_with(&paths.user_log_dir));
        assert!(!marker.starts_with(&paths.user_saved_dir));
        assert!(marker.ends_with("runtime_4242.json"));
        let _ = fs::remove_dir_all(&paths.user_data_root);
    }
}

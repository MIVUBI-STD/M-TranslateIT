//! Quick Voice Preview is strictly an isolated VoiceLab creation task.
//! It cannot install, approve, or select any voice for Meeting.
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::engine::paths::ProjectPaths;
use super::bridge_paths::{resolve_worker_python_command, worker_root};
use super::voice_lab::{
    begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    request_voice_lab_build_cancel, VoiceLabStoragePaths, MIN_REFERENCE_MS, MAX_REFERENCE_MS,
};
use super::voice_lab_recording::{accepted_guided_recordings, voice_lab_recording_active};

const TIMEOUT: Duration = Duration::from_secs(180);
static ACTIVE_PREVIEW_GENERATION: AtomicU64 = AtomicU64::new(0);
const PREVIEW_WAV: &str = "quick_voice_preview.wav";
const MAX_PREVIEW_WAV_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct QuickVoicePreviewResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
}

fn outcome(ok: bool, state: &str, message: &str) -> QuickVoicePreviewResult {
    QuickVoicePreviewResult { ok, state: state.to_string(), message: message.to_string() }
}

fn preview_dir() -> PathBuf {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
        .cache_root.join("QuickPreview")
}

fn generate_once() -> QuickVoicePreviewResult {
    if voice_lab_recording_active() {
        return outcome(false, "recording_active", "Finish recording before creating a preview.");
    }
    let accepted = accepted_guided_recordings();
    // Accepted takes are canonical 32 kHz mono PCM16. Rank eligible takes
    // closest to the existing five-second reference target, then line ID.
    // Python remains the final WAV and signal-quality authority.
    const TARGET_REFERENCE_BYTES: u64 = 5_000 * 64 + 44;
    let selected = accepted.into_iter().filter_map(|take| {
        let meta = fs::symlink_metadata(&take.path).ok()?;
        if !meta.file_type().is_file() || meta.file_type().is_symlink() { return None; }
        let bytes = meta.len();
        if !(MIN_REFERENCE_MS * 64 + 44..=MAX_REFERENCE_MS * 64 + 44).contains(&bytes) {
            return None;
        }
        Some((bytes.abs_diff(TARGET_REFERENCE_BYTES), take.line_id, take))
    }).min_by_key(|(distance, line_id, _)| (*distance, *line_id))
      .map(|(_, _, take)| take);
    let Some(reference) = selected else {
        return outcome(false, "reference_required", "Accept one clear 3–10 second recording first.");
    };
    let Some(python) = resolve_worker_python_command() else {
        return outcome(false, "python_unavailable", "The local voice runtime is not installed.");
    };
    let script = worker_root().join("voice_lab_quick_preview.py");
    let source = PathBuf::from(ProjectPaths::discover().voice_runtime_dir)
        .join("GPTSoVITS").join("Source");
    if !script.is_file() || !source.is_dir() {
        return outcome(false, "assets_missing", "Required local GPT-SoVITS assets are missing.");
    }
    let started = match begin_voice_lab_build() {
        Ok(s) => s,
        Err(_) => return outcome(false, "resource_busy", "Meeting or another voice creation task is active."),
    };
    let Some(generation) = started.generation else {
        return outcome(false, "resource_unavailable", "Voice creation authority is unavailable.");
    };
    ACTIVE_PREVIEW_GENERATION.store(generation, Ordering::Release);
    let result = (|| -> Result<(), String> {
        let destination = preview_dir();
        fs::create_dir_all(&destination).map_err(|_| "preview_storage_unavailable")?;
        let file = destination.join(PREVIEW_WAV);
        match fs::remove_file(&file) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("preview_cleanup_failed".into()),
        }
        let mut command = Command::new(python.program);
        command.args(python.bootstrap_args);
        command.arg(script).arg("--source-root").arg(source)
            .arg("--reference-wav").arg(&reference.path)
            .arg("--reference-text").arg(reference.text)
            .arg("--output-dir").arg(&destination)
            .current_dir(worker_root())
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = command.spawn().map_err(|_| "preview_process_unavailable")?;
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() { return Err("preview_generation_failed".into()); }
                    break;
                }
                Ok(None) => (),
                Err(_) => { let _ = child.kill(); let _ = child.wait(); return Err("preview_wait_failed".into()); }
            }
            let authority = current_voice_lab_build_snapshot();
            if authority.generation != Some(generation) || authority.cancel_requested || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("preview_cancelled_or_timed_out".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        // A child can exit successfully after Stop was requested; success must
        // still belong to the current, non-cancelled VoiceLab generation.
        let authority = current_voice_lab_build_snapshot();
        if authority.generation != Some(generation) || authority.cancel_requested || Instant::now() >= deadline {
            return Err("preview_cancelled_or_timed_out".into());
        }
        let info = fs::metadata(&file).map_err(|_| "preview_audio_missing")?;
        if !info.is_file() || info.len() <= 44 || info.len() > MAX_PREVIEW_WAV_BYTES {
            return Err("preview_audio_invalid".into());
        }
        Ok(())
    })();
    ACTIVE_PREVIEW_GENERATION.store(0, Ordering::Release);
    let _ = fail_voice_lab_build(generation); // release resource; NEVER complete/approve a trained actor
    if let Err(blocker) = result {
        let _ = fs::remove_file(preview_dir().join(PREVIEW_WAV));
        outcome(false, &blocker, "Quick Preview did not complete. Meeting voice was unchanged.")
    } else {
        outcome(true, "preview_ready", "Preview is ready to listen to in My Voice only.")
    }
}

pub fn quick_voice_preview_active() -> bool {
    let generation = ACTIVE_PREVIEW_GENERATION.load(Ordering::Acquire);
    generation != 0 && current_voice_lab_build_snapshot().generation == Some(generation)
}

#[tauri::command]
pub async fn generate_voice_lab_quick_preview() -> QuickVoicePreviewResult {
    match tauri::async_runtime::spawn_blocking(generate_once).await {
        Ok(result) => result,
        Err(_) => outcome(false, "preview_task_failed", "Preview could not finish safely."),
    }
}

#[tauri::command]
pub fn cancel_voice_lab_quick_preview() -> QuickVoicePreviewResult {
    let current = current_voice_lab_build_snapshot();
    if let Some(generation) = current.generation {
        // Never cancel a real training run through the preview endpoint.
        if ACTIVE_PREVIEW_GENERATION.load(Ordering::Acquire) == generation
            && current.phase == "preparing"
            && request_voice_lab_build_cancel(generation).is_ok() {
            return outcome(true, "cancelling", "Stopping preview generation.");
        }
    }
    outcome(false, "no_active_preview", "There is no pending Quick Preview to cancel.")
}

#[tauri::command]
pub fn get_voice_lab_quick_preview_audio() -> Result<tauri::ipc::Response, String> {
    if current_voice_lab_build_snapshot().active {
        return Err("voice_lab:preview_busy".into());
    }
    let path = preview_dir().join(PREVIEW_WAV);
    let size = fs::metadata(&path).map_err(|_| "voice_lab:preview_unavailable")?.len();
    if !(45..=MAX_PREVIEW_WAV_BYTES).contains(&size) || path.is_symlink() {
        return Err("voice_lab:preview_invalid".into());
    }
    let bytes = fs::read(path).map_err(|_| "voice_lab:preview_unavailable")?;
    Ok(tauri::ipc::Response::new(bytes))
}

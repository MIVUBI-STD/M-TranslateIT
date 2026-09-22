use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_events::emit_voice_build_runtime_event;

use super::bridge_paths::{resolve_worker_python_command, worker_root, worker_python_unavailable_message};
use super::helper_bridge::invalidate_required_outbound_readiness_for_voice_change;
use super::builtin_voice::{
    install_builtin_voice, is_supported_builtin_voice, meeting_blocks_voice_change,
};
use super::voice_lab::{
    approved_voice_actor_ready, begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    finish_voice_lab_build, mark_voice_lab_build_evaluating, mark_voice_lab_build_training,
    prepare_guided_dataset, promote_voice_actor_candidate, request_voice_lab_build_cancel,
    voice_lab_build_blocks_meeting, GuidedDatasetManifest, GuidedTakeContract,
    VoiceLabStoragePaths, MAX_REFERENCE_MS, MIN_REFERENCE_MS, VOICE_ACTOR_ENGINE,
    VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};
use super::voice_lab_recording::{
    accepted_guided_recordings, voice_lab_recording_active,
};

mod evaluation;

pub use evaluation::VoiceLabEvaluationSample;
use evaluation::{
    candidate_selection_matches_actor_json, evaluation_dir, evaluation_manifest,
    evaluation_review_complete, held_out_contract, EvaluationManifest, MAX_EVALUATION_WAV_BYTES,
};

const MIN_TRAINING_SPEECH_MS: u64 = 60_000;
const CANCEL_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrainingCoverageGroup {
    start_line_id: u32,
    end_line_id: u32,
    label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabCoverageGuidance {
    pub start_line_id: u32,
    pub end_line_id: u32,
    pub label: &'static str,
}

const TRAINING_COVERAGE_GROUPS: &[TrainingCoverageGroup] = &[
    TrainingCoverageGroup {
        start_line_id: 1,
        end_line_id: 24,
        label: "short conversational speech",
    },
    TrainingCoverageGroup {
        start_line_id: 25,
        end_line_id: 30,
        label: "questions and changing intonation",
    },
    TrainingCoverageGroup {
        start_line_id: 31,
        end_line_id: 64,
        label: "natural varied sentences",
    },
    TrainingCoverageGroup {
        start_line_id: 65,
        end_line_id: 96,
        label: "names, numbers, dates, or technical details",
    },
    TrainingCoverageGroup {
        start_line_id: 97,
        end_line_id: 128,
        label: "longer explanations",
    },
];

#[derive(Debug, Clone, Deserialize)]
struct BuildChildStatusFile {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    phase: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabBuildStatus {
    pub active: bool,
    pub generation: Option<u64>,
    pub phase: String,
    pub message: String,
    pub accepted_take_count: usize,
    pub accepted_duration_ms: u64,
    pub minimum_duration_ms: u64,
    pub missing_coverage: Option<VoiceLabCoverageGuidance>,
    pub can_build: bool,
    pub evaluation_ready: bool,
    pub evaluation_samples: Vec<VoiceLabEvaluationSample>,
    pub approved_voice_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabBuildActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub build: VoiceLabBuildStatus,
}

#[derive(Debug, Default)]
struct BuildProcessState {
    generation: Option<u64>,
    pid: Option<u32>,
    terminal_message: String,
}

static BUILD_PROCESS: OnceLock<(Mutex<BuildProcessState>, Condvar)> = OnceLock::new();

fn process_store() -> &'static (Mutex<BuildProcessState>, Condvar) {
    BUILD_PROCESS.get_or_init(|| (Mutex::new(BuildProcessState::default()), Condvar::new()))
}

fn storage() -> VoiceLabStoragePaths {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
}

fn work_dir(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("Runtime")
}

fn status_path(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("status.json")
}

fn log_path(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("voice_lab_build.log")
}

fn remove_file_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("voice_lab:{label}_cleanup_failed:{error}")),
    }
}

fn remove_dir_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("voice_lab:{label}_cleanup_failed:{error}")),
    }
}

fn clear_previous_build_workspace(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    remove_file_if_present(&status_path(paths), "status")?;
    remove_dir_if_present(&evaluation_dir(paths), "evaluation")?;
    remove_dir_if_present(&work_dir(paths), "work")?;
    remove_dir_if_present(&paths.candidate_actor_dir, "candidate_actor")?;
    Ok(())
}

fn reviewable_evaluation(paths: &VoiceLabStoragePaths) -> Option<EvaluationManifest> {
    let evaluation = evaluation_manifest(paths)?;
    let actor_bytes = fs::read(paths.candidate_actor_dir.join("actor.json")).ok()?;
    let actor_json = serde_json::from_slice::<serde_json::Value>(&actor_bytes).ok()?;
    candidate_selection_matches_actor_json(&evaluation, &actor_json).then_some(evaluation)
}

fn clear_build_terminal_message() {
    if let Ok(mut process) = process_store().0.lock() {
        process.terminal_message.clear();
    }
}

fn cleanup_approved_build_workspace(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    let mut errors = Vec::new();
    for result in [
        remove_dir_if_present(&evaluation_dir(paths), "evaluation"),
        remove_dir_if_present(&work_dir(paths), "work"),
        remove_dir_if_present(&paths.candidate_actor_dir, "candidate_actor"),
        remove_dir_if_present(&paths.build_dataset_dir, "build_dataset"),
        remove_file_if_present(&status_path(paths), "status"),
        remove_file_if_present(&log_path(paths), "build_log"),
    ] {
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(";"))
    }
}

fn source_root() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().voice_runtime_dir)
        .join("GPTSoVITS")
        .join("Source")
}

fn build_script() -> PathBuf {
    worker_root().join("voice_lab_build.py")
}

fn a3_wav_duration_ms(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF"
        || &header[8..12] != b"WAVE"
        || &header[12..16] != b"fmt "
        || u16::from_le_bytes([header[20], header[21]]) != 1
        || u16::from_le_bytes([header[22], header[23]]) != 1
        || u32::from_le_bytes([header[24], header[25], header[26], header[27]]) != 32_000
        || u16::from_le_bytes([header[34], header[35]]) != 16
        || &header[36..40] != b"data"
    {
        return None;
    }
    let data_size = u32::from_le_bytes([header[40], header[41], header[42], header[43]]) as u64;
    (data_size > 0).then_some(data_size.saturating_mul(1_000) / 64_000)
}

fn accepted_contract() -> (Vec<GuidedTakeContract>, u64, bool) {
    let mut takes = Vec::new();
    let mut duration_ms = 0u64;
    let mut reference_take_ready = false;
    for recording in accepted_guided_recordings() {
        let Some(duration) = a3_wav_duration_ms(&recording.path) else {
            continue;
        };
        let Some(wav_file) = recording
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        duration_ms = duration_ms.saturating_add(duration);
        if (MIN_REFERENCE_MS..=MAX_REFERENCE_MS).contains(&duration) {
            reference_take_ready = true;
        }
        takes.push(GuidedTakeContract {
            line_id: recording.line_id,
            exact_text: recording.text.to_string(),
            wav_file,
        });
    }
    (takes, duration_ms, reference_take_ready)
}

fn missing_training_coverage_group(
    takes: &[GuidedTakeContract],
) -> Option<TrainingCoverageGroup> {
    TRAINING_COVERAGE_GROUPS.iter().copied().find(|group| {
        !takes.iter().any(|take| {
            (group.start_line_id..=group.end_line_id).contains(&take.line_id)
        })
    })
}

fn training_coverage_guidance(group: TrainingCoverageGroup, approved_voice_ready: bool) -> String {
    let prefix = if approved_voice_ready {
        "My Voice is ready. To create it again, add a little more recording variety."
    } else {
        "Add a little more recording variety before creating My Voice."
    };
    format!(
        "{prefix} Try one accepted line from Lines {}-{} for {}.",
        group.start_line_id, group.end_line_id, group.label
    )
}

fn child_status(paths: &VoiceLabStoragePaths) -> Option<BuildChildStatusFile> {
    let bytes = fs::read(status_path(paths)).ok()?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return None;
    }
    let value = serde_json::from_slice::<BuildChildStatusFile>(&bytes).ok()?;
    (value.schema_version == VOICE_LAB_SCHEMA_VERSION
        && value.engine == VOICE_ACTOR_ENGINE
        && value.engine_revision == VOICE_ACTOR_ENGINE_REVISION)
        .then_some(value)
}

fn reconcile_phase(paths: &VoiceLabStoragePaths) {
    let snapshot = current_voice_lab_build_snapshot();
    let Some(generation) = snapshot.generation else {
        return;
    };
    if !snapshot.active || snapshot.phase == "cancelling" {
        return;
    }
    let Some(status) = child_status(paths) else {
        return;
    };
    match (snapshot.phase.as_str(), status.phase.as_str()) {
        ("preparing", "training") => {
            let _ = mark_voice_lab_build_training(generation);
        }
        ("preparing", "evaluating") | ("preparing", "ready_for_review") => {
            if mark_voice_lab_build_training(generation).is_ok() {
                let _ = mark_voice_lab_build_evaluating(generation);
            }
        }
        ("training", "evaluating") | ("training", "ready_for_review") => {
            let _ = mark_voice_lab_build_evaluating(generation);
        }
        _ => {}
    }
}

fn current_status() -> VoiceLabBuildStatus {
    let paths = storage();
    reconcile_phase(&paths);
    let snapshot = current_voice_lab_build_snapshot();
    let recording_active = voice_lab_recording_active();
    let (takes, duration_ms, reference_take_ready) = accepted_contract();
    let missing_coverage = missing_training_coverage_group(&takes);
    let evaluation = reviewable_evaluation(&paths);
    let child = child_status(&paths);
    let approved_ready = approved_voice_actor_ready(&paths);
    let terminal_message = process_store()
        .0
        .lock()
        .ok()
        .map(|state| state.terminal_message.clone())
        .unwrap_or_default();
    let message = if snapshot.active {
        child.as_ref().map(|status| status.message.clone()).unwrap_or_else(|| {
            match snapshot.phase.as_str() {
                "preparing" => "Preparing VoiceLab training data.".to_string(),
                "training" => "Creating your Voice Actor.".to_string(),
                "evaluating" => "Creating voice samples for review.".to_string(),
                "cancelling" => "Stopping VoiceLab creation safely.".to_string(),
                _ => "VoiceLab creation is running.".to_string(),
            }
        })
    } else if evaluation.is_some() {
        "Voice Actor samples are ready. Listen before approving My Voice.".to_string()
    } else if !terminal_message.is_empty() {
        terminal_message
    } else if duration_ms < MIN_TRAINING_SPEECH_MS {
        if approved_ready {
            "My Voice is ready. To create it again, keep recording accepted lines until there is at least one minute of usable speech."
                .to_string()
        } else {
            "Keep recording accepted lines until there is at least one minute of usable speech."
                .to_string()
        }
    } else if let Some(group) = missing_coverage {
        training_coverage_guidance(group, approved_ready)
    } else if !reference_take_ready {
        if approved_ready {
            "My Voice is ready. To create it again, record at least one clear line that lasts between 3 and 10 seconds.".to_string()
        } else {
            "Record at least one clear line that lasts between 3 and 10 seconds before creating My Voice.".to_string()
        }
    } else if approved_ready {
        "My Voice is approved and stored on this device. Your accepted recordings are also ready if you want to create it again."
            .to_string()
    } else {
        "Accepted recordings have enough usable speech and variety to create My Voice.".to_string()
    };

    VoiceLabBuildStatus {
        active: snapshot.active,
        generation: snapshot.generation,
        phase: snapshot.phase,
        message,
        accepted_take_count: takes.len(),
        accepted_duration_ms: duration_ms,
        minimum_duration_ms: MIN_TRAINING_SPEECH_MS,
        missing_coverage: missing_coverage.map(|group| VoiceLabCoverageGuidance {
            start_line_id: group.start_line_id,
            end_line_id: group.end_line_id,
            label: group.label,
        }),
        can_build: !snapshot.active
            && !recording_active
            && duration_ms >= MIN_TRAINING_SPEECH_MS
            && missing_coverage.is_none()
            && reference_take_ready,
        evaluation_ready: evaluation.is_some(),
        evaluation_samples: evaluation.map(|value| value.samples).unwrap_or_default(),
        approved_voice_ready: approved_ready,
    }
}

fn result(ok: bool, state: &str, message: impl Into<String>) -> VoiceLabBuildActionResult {
    VoiceLabBuildActionResult {
        ok,
        state: state.to_string(),
        message: message.into(),
        build: current_status(),
    }
}

fn preflight_assets() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let script = build_script();
    if !script.is_file() {
        return Err("VoiceLab build runtime is missing. Repair the TranslateIT installation.".to_string());
    }
    let source = source_root();
    if !source.is_dir() {
        return Err("VoiceLab model assets are not installed yet. Repair the TranslateIT installation.".to_string());
    }
    let marker = source.join("TRANSLATEIT_GPTSOVITS_REVISION.txt");
    let revision = fs::read_to_string(marker).unwrap_or_default();
    if revision.trim() != VOICE_ACTOR_ENGINE_REVISION {
        return Err("VoiceLab model assets do not match this TranslateIT build.".to_string());
    }
    let asr_model_root = PathBuf::from(ProjectPaths::discover().asr_model_dir);
    let primary_asr = asr_model_root.join("faster-whisper-large-v3-turbo").join("model.bin");
    let backup_asr = asr_model_root.join("faster-whisper-medium").join("model.bin");
    if !primary_asr.is_file() && !backup_asr.is_file() {
        return Err("My Voice evaluation ASR assets are not installed yet. Repair the TranslateIT installation.".to_string());
    }
    Ok((script, source, asr_model_root))
}

fn terminate_process_tree(pid: u32) -> bool {
    #[cfg(windows)]
    {
        return Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }
    #[cfg(not(windows))]
    {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

#[tauri::command]
pub fn get_voice_lab_build_status() -> VoiceLabBuildStatus {
    current_status()
}

#[tauri::command]
pub fn start_voice_lab_build(authorized_voice_confirmed: bool) -> VoiceLabBuildActionResult {
    if !authorized_voice_confirmed {
        return result(false, "authorization_required", "Confirm that this is your voice, or that you have permission to create it.");
    }
    if voice_lab_recording_active() {
        return result(false, "recording_active", "Stop the current VoiceLab recording before creating My Voice.");
    }
    let current = current_status();
    if current.active {
        return result(false, "build_active", "VoiceLab creation is already running.");
    }
    if !current.can_build {
        return result(false, "more_recording_needed", current.message);
    }

    let (script, source, asr_model_root) = match preflight_assets() {
        Ok(value) => value,
        Err(message) => return result(false, "assets_unavailable", message),
    };
    let python = match resolve_worker_python_command() {
        Some(command) => command,
        None => return result(false, "python_unavailable", worker_python_unavailable_message()),
    };

    let started = match begin_voice_lab_build() {
        Ok(snapshot) => snapshot,
        Err(error) => return result(false, "build_blocked", error),
    };
    let Some(generation) = started.generation else {
        return result(false, "build_state_invalid", "VoiceLab could not establish build ownership.");
    };

    let project_paths = ProjectPaths::discover();
    let paths = VoiceLabStoragePaths::from_project_paths(&project_paths);
    let (takes, _, _) = accepted_contract();
    let manifest = GuidedDatasetManifest {
        schema_version: VOICE_LAB_SCHEMA_VERSION,
        authorized_voice_confirmed: true,
        takes,
        held_out_lines: held_out_contract(),
    };
    if let Err(error) = prepare_guided_dataset(&project_paths, generation, &manifest) {
        let _ = fail_voice_lab_build(generation);
        return result(false, "dataset_prepare_failed", error);
    }

    let build_root = paths.cache_root.join("Build");
    if let Err(error) = fs::create_dir_all(&build_root) {
        let _ = fail_voice_lab_build(generation);
        return result(false, "build_storage_failed", format!("VoiceLab could not prepare build storage: {error}"));
    }
    if let Err(error) = clear_previous_build_workspace(&paths) {
        let _ = fail_voice_lab_build(generation);
        return result(
            false,
            "build_storage_failed",
            format!("VoiceLab could not clear the previous build workspace safely: {error}"),
        );
    }

    let log = match File::create(log_path(&paths)) {
        Ok(file) => file,
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_log_failed", format!("VoiceLab could not open its build log: {error}"));
        }
    };
    let stderr = match log.try_clone() {
        Ok(file) => file,
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_log_failed", format!("VoiceLab could not prepare its build log: {error}"));
        }
    };

    let mut command = Command::new(&python.program);
    command.args(&python.bootstrap_args);
    command
        .arg(script)
        .arg("--source-root")
        .arg(source)
        .arg("--asr-model-root")
        .arg(asr_model_root)
        .arg("--dataset-dir")
        .arg(&paths.build_dataset_dir)
        .arg("--candidate-dir")
        .arg(&paths.candidate_actor_dir)
        .arg("--evaluation-dir")
        .arg(evaluation_dir(&paths))
        .arg("--work-dir")
        .arg(work_dir(&paths))
        .arg("--status-path")
        .arg(status_path(&paths))
        .current_dir(worker_root())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_spawn_failed", format!("VoiceLab could not start the local build process: {error}"));
        }
    };
    let pid = child.id();
    match process_store().0.lock() {
        Ok(mut process) => {
            process.generation = Some(generation);
            process.pid = Some(pid);
            process.terminal_message.clear();
        }
        Err(_) => {
            let _ = terminate_process_tree(pid);
            let _ = child.wait();
            let _ = fail_voice_lab_build(generation);
            return result(
                false,
                "build_state_unavailable",
                "VoiceLab started the local build process, but process ownership could not be recorded safely. The child process was stopped.",
            );
        }
    }

    std::thread::spawn(move || {
        let exit = child.wait();
        let paths = storage();
        reconcile_phase(&paths);
        let ready = exit.as_ref().map(|status| status.success()).unwrap_or(false)
            && child_status(&paths)
                .map(|status| status.phase == "ready_for_review")
                .unwrap_or(false)
            && reviewable_evaluation(&paths).is_some();
        let cancelling = current_voice_lab_build_snapshot().phase == "cancelling";
        if ready || cancelling {
            let _ = finish_voice_lab_build(generation);
        } else {
            let _ = fail_voice_lab_build(generation);
        }
        let message = if ready {
            "Voice Actor samples are ready. Listen before approving My Voice.".to_string()
        } else if cancelling {
            "VoiceLab creation stopped before a candidate was approved.".to_string()
        } else {
            "VoiceLab could not create a reviewable Voice Actor. Check Diagnostics and try again.".to_string()
        };
        let event_reason = if ready {
            "voice_build_completed"
        } else if cancelling {
            "voice_build_cancelled"
        } else {
            "voice_build_failed"
        };
        let (lock, signal) = process_store();
        if let Ok(mut process) = lock.lock() {
            if process.generation == Some(generation) {
                process.pid = None;
                process.generation = None;
                process.terminal_message = message;
            }
            signal.notify_all();
        }
        emit_voice_build_runtime_event(event_reason, generation);
    });

    result(true, "building", "VoiceLab started creating My Voice. You can leave this page open while it works.")
}

#[tauri::command]
pub fn cancel_voice_lab_build() -> VoiceLabBuildActionResult {
    let snapshot = current_voice_lab_build_snapshot();
    let Some(generation) = snapshot.generation else {
        return result(true, "idle", "There is no active VoiceLab build to stop.");
    };
    if !snapshot.active {
        return result(true, "idle", "There is no active VoiceLab build to stop.");
    }
    if let Err(error) = request_voice_lab_build_cancel(generation) {
        return result(false, "cancel_failed", error);
    }
    let (lock, signal) = process_store();
    let pid = lock.lock().ok().and_then(|state| state.pid);
    let Some(pid) = pid else {
        return result(false, "cancel_pending", "VoiceLab is stopping, but the build process could not be addressed yet.");
    };
    if !terminate_process_tree(pid) {
        return result(false, "cancel_pending", "VoiceLab could not confirm that the build process stopped yet.");
    }
    let guard = match lock.lock() {
        Ok(guard) => guard,
        Err(_) => return result(false, "cancel_pending", "VoiceLab is stopping, but process state is temporarily unavailable."),
    };
    let _ = signal.wait_timeout_while(guard, CANCEL_WAIT, |state| state.pid == Some(pid));
    if current_voice_lab_build_snapshot().active {
        return result(false, "cancel_pending", "VoiceLab is still finishing build cleanup.");
    }
    result(true, "cancelled", "VoiceLab creation stopped. Your accepted recordings were kept.")
}

#[tauri::command]
pub fn get_builtin_voice_preview_audio(voice_id: String) -> Result<tauri::ipc::Response, String> {
    if !is_supported_builtin_voice(&voice_id) {
        return Err("builtin_voice:unknown_voice".to_string());
    }
    let project_paths = ProjectPaths::discover();
    let path = std::path::PathBuf::from(project_paths.voice_runtime_dir)
        .join("BuiltInVoices")
        .join(&voice_id)
        .join("reference.wav");
    let bytes = fs::read(path).map_err(|_| "builtin_voice:preview_unavailable".to_string())?;
    if bytes.len() < 44 || bytes.len() as u64 > MAX_EVALUATION_WAV_BYTES {
        return Err("builtin_voice:preview_invalid".to_string());
    }
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn select_builtin_voice(
    voice_id: String,
    authorized_voice_confirmed: bool,
) -> VoiceLabBuildActionResult {
    if !is_supported_builtin_voice(&voice_id) {
        return result(false, "unknown_builtin_voice", "Choose one of the two built-in voices.");
    }
    if meeting_blocks_voice_change() {
        return result(
            false,
            "meeting_active",
            "Stop Meeting translation before changing the Meeting voice.",
        );
    }
    if voice_lab_build_blocks_meeting() {
        return result(
            false,
            "build_active",
            "Wait for My Voice creation to finish before changing the Meeting voice.",
        );
    }

    let project_paths = ProjectPaths::discover();
    let storage = VoiceLabStoragePaths::from_project_paths(&project_paths);
    let target = storage.approved_actor_dir.clone();
    if target.join("actor.json").is_file() && !authorized_voice_confirmed {
        return result(
            false,
            "approval_required",
            "Replacing the current Meeting voice needs your explicit confirmation.",
        );
    }

    match install_builtin_voice(&project_paths, &voice_id, &target, VOICE_ACTOR_ENGINE_REVISION) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            let label = voice_id.trim_end_matches("Voice").to_lowercase();
            result(
                true,
                "selected",
                format!("Built-in {label} voice selected for Meeting."),
            )
        }
        Err(error) => result(false, "builtin_selection_failed", error),
    }
}

#[tauri::command]
pub fn approve_voice_lab_candidate(reviewed_line_ids: Vec<u32>) -> VoiceLabBuildActionResult {
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Wait for VoiceLab creation to finish before approving My Voice.");
    }
    let paths = storage();
    let ready_for_review = child_status(&paths)
        .map(|status| status.phase == "ready_for_review")
        .unwrap_or(false);
    if !ready_for_review {
        return result(
            false,
            "evaluation_required",
            "My Voice does not have a completed reviewable build yet.",
        );
    }
    let Some(evaluation) = reviewable_evaluation(&paths) else {
        return result(
            false,
            "evaluation_candidate_mismatch",
            "My Voice review files no longer match the candidate that would be saved. Create My Voice again.",
        );
    };
    if !evaluation_review_complete(&evaluation, &reviewed_line_ids) {
        return result(
            false,
            "evaluation_listening_required",
            "Finish listening to every My Voice preview before approving it.",
        );
    }
    let project_paths = ProjectPaths::discover();
    match promote_voice_actor_candidate(&project_paths) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            let paths = VoiceLabStoragePaths::from_project_paths(&project_paths);
            clear_build_terminal_message();
            match cleanup_approved_build_workspace(&paths) {
                Ok(()) => result(true, "approved", "My Voice was approved and saved on this device."),
                Err(_error) => result(
                    true,
                    "approved_cleanup_attention",
                    "My Voice was saved, but some temporary build files could not be cleared yet.",
                ),
            }
        }
        Err(error) => result(false, "approval_failed", error),
    }
}

#[tauri::command]
pub fn get_voice_lab_evaluation_audio(line_id: u32) -> Result<tauri::ipc::Response, String> {
    let paths = storage();
    let manifest = reviewable_evaluation(&paths)
        .ok_or_else(|| "voice_lab:evaluation_unavailable".to_string())?;
    let sample = manifest
        .samples
        .into_iter()
        .find(|sample| sample.line_id == line_id)
        .ok_or_else(|| "voice_lab:evaluation_line_missing".to_string())?;
    let path = evaluation_dir(&paths).join(sample.wav_file);
    let bytes = fs::read(path).map_err(|_| "voice_lab:evaluation_audio_read_failed".to_string())?;
    if bytes.len() < 44 || bytes.len() as u64 > MAX_EVALUATION_WAV_BYTES {
        return Err("voice_lab:evaluation_audio_invalid".to_string());
    }
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
#[path = "voice_lab_build_tests.rs"]
mod p1_recording_coverage_tests;

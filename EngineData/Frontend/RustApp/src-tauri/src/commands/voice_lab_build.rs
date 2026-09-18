use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::engine::paths::ProjectPaths;

use super::bridge_paths::{resolve_worker_python_command, worker_root, worker_python_unavailable_message};
use super::helper_bridge::invalidate_required_outbound_readiness_for_voice_change;
use super::builtin_voice::{
    install_builtin_voice, is_supported_builtin_voice, meeting_blocks_voice_change,
};
use super::voice_lab::{
    approved_voice_actor_ready, begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    finish_voice_lab_build, mark_voice_lab_build_evaluating, mark_voice_lab_build_training,
    prepare_guided_dataset, promote_voice_actor_candidate, request_voice_lab_build_cancel,
    voice_lab_build_blocks_meeting, GuidedDatasetManifest, GuidedEvaluationLineContract,
    GuidedTakeContract, VoiceLabStoragePaths,
};
use super::voice_lab_recording::get_voice_lab_guided_recording_state;

const SCHEMA_VERSION: u32 = 1;
const ENGINE: &str = "gpt-sovits-v2proplus";
const ENGINE_REVISION: &str = "d523079fc05d9a8028d6085bffe4a2757c32abb6";
const MIN_TRAINING_SPEECH_MS: u64 = 60_000;
const MAX_EVALUATION_WAV_BYTES: u64 = 16 * 1024 * 1024;
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

const HELD_OUT_LINES: &[(u32, &str)] = &[
    (1001, "Please confirm the final schedule before we send the update to the client."),
    (1002, "The system should remain clear and natural during a longer technical discussion."),
    (1003, "I can review the latest results tomorrow morning and share my decision with the team."),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLabEvaluationSample {
    pub line_id: u32,
    pub exact_text: String,
    pub wav_file: String,
    pub speaker_similarity: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct EvaluationManifest {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    samples: Vec<VoiceLabEvaluationSample>,
}

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

fn evaluation_dir(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Evaluation")
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

fn accepted_contract() -> (Vec<GuidedTakeContract>, u64) {
    let paths = storage();
    let recording = get_voice_lab_guided_recording_state();
    let mut takes = Vec::new();
    let mut duration_ms = 0u64;
    for line in recording.lines.into_iter().filter(|line| line.accepted) {
        let wav_file = format!("take_{:04}.wav", line.line_id);
        let wav = paths.takes_dir.join(&wav_file);
        let Some(duration) = a3_wav_duration_ms(&wav) else {
            continue;
        };
        duration_ms = duration_ms.saturating_add(duration);
        takes.push(GuidedTakeContract {
            line_id: line.line_id,
            exact_text: line.text,
            wav_file,
        });
    }
    (takes, duration_ms)
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

fn held_out_contract() -> Vec<GuidedEvaluationLineContract> {
    HELD_OUT_LINES
        .iter()
        .map(|(line_id, exact_text)| GuidedEvaluationLineContract {
            line_id: *line_id,
            exact_text: (*exact_text).to_string(),
        })
        .collect()
}

fn child_status(paths: &VoiceLabStoragePaths) -> Option<BuildChildStatusFile> {
    let bytes = fs::read(status_path(paths)).ok()?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return None;
    }
    let value = serde_json::from_slice::<BuildChildStatusFile>(&bytes).ok()?;
    (value.schema_version == SCHEMA_VERSION
        && value.engine == ENGINE
        && value.engine_revision == ENGINE_REVISION)
        .then_some(value)
}

fn evaluation_manifest(paths: &VoiceLabStoragePaths) -> Option<EvaluationManifest> {
    let root = evaluation_dir(paths);
    let bytes = fs::read(root.join("evaluation.json")).ok()?;
    if bytes.is_empty() || bytes.len() > 128 * 1024 {
        return None;
    }
    let manifest = serde_json::from_slice::<EvaluationManifest>(&bytes).ok()?;
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.engine != ENGINE
        || manifest.engine_revision != ENGINE_REVISION
        || manifest.samples.len() != HELD_OUT_LINES.len()
    {
        return None;
    }
    for sample in &manifest.samples {
        if !sample.speaker_similarity.is_finite()
            || sample.wav_file.trim().is_empty()
            || Path::new(&sample.wav_file).file_name().and_then(|name| name.to_str())
                != Some(sample.wav_file.as_str())
        {
            return None;
        }
        let wav = root.join(&sample.wav_file);
        let metadata = fs::symlink_metadata(wav).ok()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() < 44
            || metadata.len() > MAX_EVALUATION_WAV_BYTES
        {
            return None;
        }
    }
    Some(manifest)
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
    let recording_active = get_voice_lab_guided_recording_state().recording_line_id.is_some();
    let (takes, duration_ms) = accepted_contract();
    let missing_coverage = missing_training_coverage_group(&takes);
    let evaluation = evaluation_manifest(&paths);
    let child = child_status(&paths);
    let approved_ready = approved_voice_actor_ready(&ProjectPaths::discover());
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
            && missing_coverage.is_none(),
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

fn preflight_assets() -> Result<(PathBuf, PathBuf), String> {
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
    if revision.trim() != ENGINE_REVISION {
        return Err("VoiceLab model assets do not match this TranslateIT build.".to_string());
    }
    Ok((script, source))
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
    if get_voice_lab_guided_recording_state().recording_line_id.is_some() {
        return result(false, "recording_active", "Stop the current VoiceLab recording before creating My Voice.");
    }
    let current = current_status();
    if current.active {
        return result(false, "build_active", "VoiceLab creation is already running.");
    }
    if !current.can_build {
        return result(false, "more_recording_needed", current.message);
    }

    let (script, source) = match preflight_assets() {
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
    let (takes, _) = accepted_contract();
    let manifest = GuidedDatasetManifest {
        schema_version: SCHEMA_VERSION,
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
    let _ = fs::remove_file(status_path(&paths));
    let _ = fs::remove_dir_all(evaluation_dir(&paths));
    let _ = fs::remove_dir_all(work_dir(&paths));
    let _ = fs::remove_dir_all(&paths.candidate_actor_dir);

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
            && evaluation_manifest(&paths).is_some();
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
        let (lock, signal) = process_store();
        if let Ok(mut process) = lock.lock() {
            if process.generation == Some(generation) {
                process.pid = None;
                process.generation = None;
                process.terminal_message = message;
            }
            signal.notify_all();
        }
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

    match install_builtin_voice(&project_paths, &voice_id, &target, ENGINE_REVISION) {
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
pub fn approve_voice_lab_candidate() -> VoiceLabBuildActionResult {
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Wait for VoiceLab creation to finish before approving My Voice.");
    }
    if evaluation_manifest(&storage()).is_none() {
        return result(false, "evaluation_required", "Listen to a completed VoiceLab evaluation before approving My Voice.");
    }
    let project_paths = ProjectPaths::discover();
    match promote_voice_actor_candidate(&project_paths) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            let paths = VoiceLabStoragePaths::from_project_paths(&project_paths);
            let _ = fs::remove_dir_all(evaluation_dir(&paths));
            result(true, "approved", "My Voice was approved and saved on this device.")
        }
        Err(error) => result(false, "approval_failed", error),
    }
}

#[tauri::command]
pub fn get_voice_lab_evaluation_audio(line_id: u32) -> Result<tauri::ipc::Response, String> {
    let paths = storage();
    let manifest = evaluation_manifest(&paths)
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
mod p1_recording_coverage_tests {
    use super::{missing_training_coverage_group, GuidedTakeContract};

    fn take(line_id: u32) -> GuidedTakeContract {
        GuidedTakeContract {
            line_id,
            exact_text: format!("line {line_id}"),
            wav_file: format!("take_{line_id:04}.wav"),
        }
    }

    #[test]
    fn many_accepted_lines_from_one_style_do_not_satisfy_recording_variety() {
        let takes = (1..=12).map(take).collect::<Vec<_>>();
        let missing = missing_training_coverage_group(&takes).expect("coverage must remain incomplete");

        assert_eq!(missing.start_line_id, 25);
        assert_eq!(missing.end_line_id, 30);
    }

    #[test]
    fn one_accepted_line_from_each_curated_block_satisfies_recording_variety() {
        let takes = [1, 25, 31, 65, 97]
            .into_iter()
            .map(take)
            .collect::<Vec<_>>();

        assert!(missing_training_coverage_group(&takes).is_none());
    }
}

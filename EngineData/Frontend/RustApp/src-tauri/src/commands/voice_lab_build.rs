use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
#[cfg(windows)]
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_events::emit_voice_build_runtime_event;

use super::bridge_paths::{resolve_worker_python_command, worker_root, worker_python_unavailable_message};
use super::helper_bridge::invalidate_required_outbound_readiness_for_voice_change;
use super::builtin_voice::{
    install_builtin_voice, is_supported_builtin_voice, meeting_blocks_voice_change,
};
use super::voice_lab::{
    approved_voice_actor_ready, begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    finish_voice_lab_build, preserve_training_checkpoints, prepare_guided_dataset,
    prepare_guided_dataset_from_archives, choose_archived_takes,
    promote_voice_actor_candidate,
    request_voice_lab_build_cancel,
    voice_lab_build_blocks_meeting, archive_selected_personal_voice,
    activate_stored_personal_voice, recover_selected_actor_if_missing,
    selected_voice_info, actor_version_page, activate_archived_personal_voice, selected_personal_actor_matches_version, voice_backup_active,
    ActorVersionPage, GuidedDatasetManifest, GuidedTakeContract,
    VoiceLabStoragePaths, MAX_REFERENCE_MS, MIN_REFERENCE_MS, VOICE_ACTOR_ENGINE,
    VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};
use super::voice_lab_recording::{
    accepted_guided_recordings, guided_training_prompts, guided_training_coverage,
    guided_recording_recommendations, RecordingSuggestion, voice_lab_recording_active,
};

mod evaluation;
mod phase;
mod receipts;
mod process_owner;
#[cfg(windows)]
use process_owner::OwnedBuildJob;
use receipts::{completion_file, completion_matches, commit_completion, create_identity, identity_file, read_identity};

pub use evaluation::VoiceLabEvaluationSample;
use phase::reconcile_phase;
use evaluation::{
    evaluation_dir, evaluation_review_complete, held_out_contract, held_out_wavs_intact,
    reviewable_evaluation as reviewable_evaluation_for_takes, verified_held_out_wav,
    EvaluationManifest, MAX_EVALUATION_WAV_BYTES,
};

const MIN_TRAINING_SPEECH_MS: u64 = 60_000;
// Distinct from the 600-prompt library; pinned speaker-consistency processing
// currently accepts at most 128 recordings in a single training build.
const MAX_TRAINING_TAKES: usize = 128;
const CANCEL_WAIT: Duration = Duration::from_secs(10);

// Guard select/approve on the shared worker actor path. No parallel promotion.
static VOICE_SELECTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
pub(crate) fn voice_selection_guard() -> Result<std::sync::MutexGuard<'static, ()>, ()> {
    if voice_backup_active() { return Err(()); }
    VOICE_SELECTION_LOCK.get_or_init(|| Mutex::new(())).lock().map_err(|_| ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrainingCoverageGroup {
    start_line_id: u32,
    end_line_id: u32,
    coverage: &'static str,
    label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabCoverageGuidance {
    pub start_line_id: u32,
    pub end_line_id: u32,
    pub label: &'static str,
    pub suggested_line_id: Option<u32>,
}

const TRAINING_COVERAGE_GROUPS: &[TrainingCoverageGroup] = &[
    TrainingCoverageGroup {
        start_line_id: 1,
        end_line_id: 24,
        label: "short conversational speech",
        coverage: "conversation",
    },
    TrainingCoverageGroup {
        start_line_id: 25,
        end_line_id: 30,
        label: "questions and changing intonation",
        coverage: "intonation",
    },
    TrainingCoverageGroup {
        start_line_id: 31,
        end_line_id: 64,
        label: "natural varied sentences",
        coverage: "natural",
    },
    TrainingCoverageGroup {
        start_line_id: 65,
        end_line_id: 96,
        label: "names, numbers, dates, or technical details",
        coverage: "details",
    },
    TrainingCoverageGroup {
        start_line_id: 97,
        end_line_id: 128,
        label: "longer explanations",
        coverage: "explanations",
    },
];

#[derive(Debug, Clone, Deserialize)]
struct BuildChildStatusFile {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    phase: String,
    message: String,
    run_token: String,
    run_generation: u64,
    sequence: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyVoiceComputeStatus {
    pub state: String,
    pub cpu_name: String,
    pub gpu_name: Option<String>,
    pub vram_mib: Option<u64>,
    pub free_vram_mib: Option<u64>,
    pub reserve_mib: Option<u64>,
    pub safe_budget_mib: Option<u64>,
    pub admission_state: String,
    pub profile: String,
    pub detail: String,
}

impl MyVoiceComputeStatus {
    fn unavailable(detail: &str) -> Self {
        Self {
            state: "unavailable".into(),
            cpu_name: "CPU name unavailable".into(),
            gpu_name: None,
            vram_mib: None,
            free_vram_mib: None,
            reserve_mib: None,
            safe_budget_mib: None,
            admission_state: "unknown".into(),
            profile: "SAFE_8GB_V1".into(),
            detail: detail.into(),
        }
    }
}

// Explicit-only read-only probe. Do NOT add CUDA/PyTorch imports to the
// 1.4-second build-status poll or the persistent Meeting worker.
fn probe_my_voice_compute_status() -> MyVoiceComputeStatus {
    let Some(python) = resolve_worker_python_command() else {
        return MyVoiceComputeStatus::unavailable(
            "Local Python runtime is unavailable. Repair the installation before training.",
        );
    };
    let root = worker_root();
    let script = root.join("my_voice_compute_probe.py");
    if !script.is_file() {
        return MyVoiceComputeStatus::unavailable(
            "My Voice hardware checker is missing. Repair the installation before training.",
        );
    }
    let mut command = Command::new(&python.program);
    command.args(&python.bootstrap_args)
        .arg("-s").arg(script)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return MyVoiceComputeStatus::unavailable(
            "Could not start the local Python hardware checker.",
        );
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(exit)) => {
                if !exit.success() {
                    return MyVoiceComputeStatus::unavailable(
                        "Local Python hardware checker exited unsuccessfully.",
                    );
                }
                let mut output = Vec::new();
                let Some(stdout) = child.stdout.take() else {
                    return MyVoiceComputeStatus::unavailable("Hardware checker output is missing.");
                };
                if stdout.take(4097).read_to_end(&mut output).is_err() || output.len() > 4096 {
                    return MyVoiceComputeStatus::unavailable("Hardware checker output is invalid.");
                }
                let Ok(report) = serde_json::from_slice::<MyVoiceComputeStatus>(&output) else {
                    return MyVoiceComputeStatus::unavailable("Hardware checker output is unreadable.");
                };
                if !matches!(report.state.as_str(), "cuda_detected" | "cpu_only" | "unavailable")
                    || !matches!(report.admission_state.as_str(), "admitted" | "blocked_gpu_headroom" | "unknown")
                    || report.profile != "SAFE_8GB_V1"
                    || report.cpu_name.len() > 200 || report.detail.len() > 500
                    || report.gpu_name.as_ref().is_some_and(|name| name.len() > 200)
                    || report.vram_mib.is_some_and(|mib| mib > 1_048_576)
                    || report.free_vram_mib.is_some_and(|mib| mib > 1_048_576)
                    || report.safe_budget_mib.is_some_and(|mib| mib > 1_048_576)
                    || report.reserve_mib.is_some_and(|mib| mib > 1_048_576)
                    || (report.state != "cuda_detected"
                        && (report.gpu_name.is_some() || report.vram_mib.is_some()
                            || report.free_vram_mib.is_some() || report.safe_budget_mib.is_some()))
                    || (report.state == "cuda_detected"
                        && (report.gpu_name.is_none() || report.vram_mib.is_none()
                            || report.free_vram_mib.is_none() || report.safe_budget_mib.is_none()
                            || report.reserve_mib.is_none()))
                    || (report.admission_state == "admitted"
                        && (report.safe_budget_mib.unwrap_or(0) < 3072
                            || report.safe_budget_mib > report.free_vram_mib))
                {
                    return MyVoiceComputeStatus::unavailable("Hardware checker returned inconsistent data.");
                }
                return report;
            }
            Ok(None) if started.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return MyVoiceComputeStatus::unavailable(
                    "Hardware check failed or timed out. Try again before training.",
                );
            }
        }
    }
}

#[tauri::command]
pub async fn get_my_voice_compute_status() -> MyVoiceComputeStatus {
    tauri::async_runtime::spawn_blocking(probe_my_voice_compute_status)
        .await
        .unwrap_or_else(|_| MyVoiceComputeStatus::unavailable(
            "Hardware checker could not finish safely.",
        ))
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabBuildStatus {
    pub active: bool,
    pub preview_active: bool,
    pub generation: Option<u64>,
    pub phase: String,
    pub message: String,
    pub accepted_take_count: usize,
    pub accepted_duration_ms: u64,
    pub minimum_duration_ms: u64,
    pub missing_coverage: Option<VoiceLabCoverageGuidance>,
    pub recording_suggestions: Vec<RecordingSuggestion>,
    pub can_build: bool,
    pub evaluation_ready: bool,
    pub evaluation_review_id: Option<String>,
    pub evaluation_samples: Vec<VoiceLabEvaluationSample>,
    pub approved_voice_ready: bool,
    pub selected_voice_kind: String,
    pub selected_builtin_voice_id: Option<String>,
    pub saved_personal_voice_available: bool,
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
    #[cfg(windows)]
    owned_job: Option<Arc<OwnedBuildJob>>,
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
    remove_file_if_present(&completion_file(paths), "completion_receipt")?;
    remove_file_if_present(&identity_file(paths), "run_identity")?;
    remove_file_if_present(&status_path(paths), "status")?;
    remove_dir_if_present(&evaluation_dir(paths), "evaluation")?;
    remove_dir_if_present(&work_dir(paths), "work")?;
    remove_dir_if_present(&paths.candidate_actor_dir, "candidate_actor")?;
    Ok(())
}

fn reviewable_evaluation(paths: &VoiceLabStoragePaths) -> Option<EvaluationManifest> {
    reviewable_evaluation_for_takes(paths, &accepted_contract().0)
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
        remove_file_if_present(&completion_file(paths), "completion_receipt"),
        remove_file_if_present(&identity_file(paths), "run_identity"),
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
            guided_training_coverage(take.line_id) == Some(group.coverage)
        })
    })
}

fn training_coverage_guidance(group: TrainingCoverageGroup, approved_voice_ready: bool) -> String {
    let prefix = if approved_voice_ready {
        "My Voice is ready. To create it again, add a little more recording variety."
    } else {
        "Add a little more recording variety before creating My Voice."
    };
    format!("{prefix} Record another sentence covering {}.", group.label)
}

fn child_status(paths: &VoiceLabStoragePaths) -> Option<BuildChildStatusFile> {
    let bytes = fs::read(status_path(paths)).ok()?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return None;
    }
    let value = serde_json::from_slice::<BuildChildStatusFile>(&bytes).ok()?;
    let identity = read_identity(paths)?;
    (value.schema_version == VOICE_LAB_SCHEMA_VERSION
        && value.engine == VOICE_ACTOR_ENGINE
        && value.engine_revision == VOICE_ACTOR_ENGINE_REVISION
        && value.sequence > 0 && value.message.len() <= 500
        && matches!(value.phase.as_str(), "preparing" | "training" | "evaluating" | "ready_for_review" | "failed")
        && value.run_token == identity.token
        && value.run_generation == identity.generation)
        .then_some(value)
}

fn current_status() -> VoiceLabBuildStatus {
    let paths = storage();
    reconcile_phase(&paths);
    let snapshot = current_voice_lab_build_snapshot();
    let recording_active = voice_lab_recording_active();
    let (takes, duration_ms, reference_take_ready) = accepted_contract();
    let missing_coverage = missing_training_coverage_group(&takes);
    let accepted_ids: Vec<u32> = takes.iter().map(|take| take.line_id).collect();
    // Advisory written-English patterns. This never gates training or edits WAVs.
    let recording_suggestions = guided_recording_recommendations(
        &accepted_ids, missing_coverage.map(|group| group.coverage),
    );
    let evaluation = reviewable_evaluation(&paths)
        .filter(|manifest| completion_matches(&paths, &manifest.review_id));
    let preview_active = super::voice_lab_preview::quick_voice_preview_active();
    let child = if preview_active { None } else { child_status(&paths) };
    let approved_ready = approved_voice_actor_ready(&paths);
    let selected_voice = selected_voice_info(&paths);
    let terminal_message = process_store()
        .0
        .lock()
        .ok()
        .map(|state| state.terminal_message.clone())
        .unwrap_or_default();
    let message = if preview_active && snapshot.phase == "cancelling" {
        "Stopping Quick Preview safely.".to_string()
    } else if preview_active {
        "Creating Quick Preview locally.".to_string()
    } else if snapshot.active {
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
    } else if child.as_ref().is_some_and(|status| status.phase == "ready_for_review") {
        "Recordings have changed or the earlier voice review is unavailable. Create My Voice again before approval. Your existing Meeting voice is unchanged.".to_string()
    } else if !terminal_message.is_empty() {
        terminal_message
    } else if takes.len() > MAX_TRAINING_TAKES {
        "Your saved recordings are available. Choose up to 128 recordings from Saved Recordings to create another voice."
            .to_string()
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
        preview_active,
        generation: snapshot.generation,
        phase: snapshot.phase,
        message,
        accepted_take_count: takes.len(),
        accepted_duration_ms: duration_ms,
        minimum_duration_ms: MIN_TRAINING_SPEECH_MS,
        missing_coverage: missing_coverage.map(|group| {
            let suggested_line_id = guided_training_prompts().iter()
                .filter(|(id, _)| guided_training_coverage(*id) == Some(group.coverage))
                .find(|(id, _)| !takes.iter().any(|take| take.line_id == *id))
                .map(|(id, _)| *id);
            VoiceLabCoverageGuidance {
                start_line_id: group.start_line_id,
                end_line_id: group.end_line_id,
                label: group.label,
                suggested_line_id,
            }
        }),
        recording_suggestions,
        can_build: !snapshot.active
            && !recording_active
            && takes.len() <= MAX_TRAINING_TAKES
            && duration_ms >= MIN_TRAINING_SPEECH_MS
            && missing_coverage.is_none()
            && reference_take_ready,
        evaluation_ready: evaluation.is_some(),
        evaluation_review_id: evaluation.as_ref().map(|value| value.review_id.clone()),
        evaluation_samples: evaluation.map(|value| value.samples).unwrap_or_default(),
        approved_voice_ready: approved_ready,
        selected_voice_kind: selected_voice.kind,
        selected_builtin_voice_id: selected_voice.builtin_id,
        saved_personal_voice_available: selected_voice.personal_available,
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

pub fn start_voice_lab_build(authorized_voice_confirmed: bool) -> VoiceLabBuildActionResult {
    start_voice_lab_build_selected(authorized_voice_confirmed, None)
}

/// User-selected archived recordings are rebuilt with the SAME pinned
/// pretrained weights and existing training/evaluation worker pipeline.
pub fn start_voice_lab_improvement(
    authorized_voice_confirmed: bool,
    archive_ids: Vec<String>,
) -> VoiceLabBuildActionResult {
    start_voice_lab_build_selected(authorized_voice_confirmed, Some(archive_ids))
}

fn start_voice_lab_build_selected(
    authorized_voice_confirmed: bool,
    selected_archives: Option<Vec<String>>,
) -> VoiceLabBuildActionResult {
    if voice_backup_active() {
        return result(false, "backup_active", "Finish the My Voice backup or restore before starting training.");
    }
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
    if selected_archives.is_some() && (meeting_blocks_voice_change()
        || super::voice_lab_preview::quick_voice_preview_active())
    {
        return result(false, "voice_busy",
            "Stop Meeting or Quick Preview before improving My Voice.");
    }
    // Inspect archive selection BEFORE claiming build ownership. The hard
    // per-run maximum is 128; a 600+ prompt library is not a training cap.
    let selection = if let Some(ids) = selected_archives.as_ref() {
        match choose_archived_takes(&storage(), ids, guided_training_prompts()) {
            Ok(selection) => {
                if selection.duration_ms < MIN_TRAINING_SPEECH_MS {
                    return result(false, "more_recording_needed",
                        "Select at least one minute of usable saved recordings.");
                }
                if let Some(missing) = missing_training_coverage_group(&selection.takes) {
                    return result(false, "more_recording_needed", format!(
                        "Add one saved recording from {} (lines {}–{}).",
                        missing.label, missing.start_line_id, missing.end_line_id,
                    ));
                }
                if !selection.reference_ready {
                    return result(false, "more_recording_needed",
                        "Select at least one saved recording between three and ten seconds.");
                }
                Some(selection)
            }
            Err(error) => return result(false, "saved_recordings_invalid", format!(
                "Can't use the selected saved recordings. Check the recording library: {error}"
            )),
        }
    } else {
        if !current.can_build {
            return result(false, "more_recording_needed", current.message);
        }
        None
    };

    let (script, source, asr_model_root) = match preflight_assets() {
        Ok(value) => value,
        Err(message) => return result(false, "assets_unavailable", message),
    };
    let python = match resolve_worker_python_command() {
        Some(command) => command,
        None => return result(false, "python_unavailable", worker_python_unavailable_message()),
    };
    // Authoritative gate: direct native Create and Improve must not bypass
    // the advisory Svelte badge. Rechecked again before each Python GPU stage.
    if meeting_blocks_voice_change() {
        return result(false, "voice_busy",
            "Stop the active Meeting before preparing My Voice training.");
    }
    let compute = probe_my_voice_compute_status();
    if compute.state != "cuda_detected" || compute.admission_state != "admitted" {
        return result(false, "gpu_admission_blocked", format!(
            "My Voice SAFE_8GB cannot start: {} Recheck the GPU and free VRAM.",
            compute.detail
        ));
    }
    let Some(safe_budget_mib) = compute.safe_budget_mib else {
        return result(false, "gpu_admission_blocked", "SAFE_8GB memory budget could not be verified.");
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
    // This must occur BEFORE prepare_guided_dataset replaces snapshot.ref and
    // BEFORE old Work is cleared. Checkpoints from an interrupted earlier run
    // cannot be paired safely with a new dataset. Fail closed on any error.
    if let Err(error) = preserve_training_checkpoints(&paths) {
        let _ = fail_voice_lab_build(generation);
        return result(false, "prior_training_recovery_required", format!(
            "Previous training artifacts need protection before a new build: {error}"
        ));
    }
    let (takes, _, _) = accepted_contract();
    let manifest = GuidedDatasetManifest {
        schema_version: VOICE_LAB_SCHEMA_VERSION,
        authorized_voice_confirmed: true,
        takes: selection.as_ref().map(|selected| selected.takes.clone()).unwrap_or(takes),
        held_out_lines: held_out_contract(),
    };
    let preparation = match selection.as_ref() {
        Some(selected) => prepare_guided_dataset_from_archives(
            &project_paths, generation, &manifest, &selected.archive_ids,
        ).map(|_| ()),
        None => prepare_guided_dataset(&project_paths, generation, &manifest).map(|_| ()),
    };
    if let Err(error) = preparation {
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

    let run_identity = match create_identity(&paths, generation) {
        Ok(identity) => identity,
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_identity_failed", error);
        }
    };
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
        .arg("--gpu-budget-mib")
        .arg(safe_budget_mib.to_string())
        .arg("--run-token")
        .arg(&run_identity.token)
        .arg("--run-generation")
        .arg(generation.to_string())
        .current_dir(worker_root())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    let owned_job = match OwnedBuildJob::new() {
        Ok(job) => Arc::new(job),
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_process_guard_failed", error);
        }
    };
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_spawn_failed", format!("VoiceLab could not start the local build process: {error}"));
        }
    };
    let pid = child.id();
    #[cfg(windows)]
    if let Err(error) = owned_job.assign(&child) {
        let _ = terminate_process_tree(pid);
        let _ = child.wait();
        let _ = fail_voice_lab_build(generation);
        return result(false, "build_process_guard_failed", error);
    }
    match process_store().0.lock() {
        Ok(mut process) => {
            process.generation = Some(generation);
            process.pid = Some(pid);
            #[cfg(windows)]
            { process.owned_job = Some(Arc::clone(&owned_job)); }
            process.terminal_message.clear();
        }
        Err(_) => {
            #[cfg(windows)]
            let _ = owned_job.terminate();
            #[cfg(not(windows))]
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

    #[cfg(windows)]
    let worker_job = Arc::clone(&owned_job);
    std::thread::spawn(move || {
        #[cfg(windows)]
        let _job_lifetime = worker_job;
        let exit = child.wait();
        let paths = storage();
        reconcile_phase(&paths);
        let candidate = if exit.as_ref().map(|status| status.success()).unwrap_or(false)
            && child_status(&paths)
                .map(|status| status.phase == "ready_for_review")
                .unwrap_or(false)
        { reviewable_evaluation(&paths) } else { None };
        let ready = candidate.as_ref().is_some_and(|evaluation| {
            commit_completion(&paths, &run_identity, &evaluation.review_id).is_ok()
        });
        if !ready {
            // Never carry a ready status from an interrupted/failed attempt.
            let _ = remove_file_if_present(&completion_file(&paths), "completion");
        }
        // Only interrupted/incomplete runs retain optimizer checkpoint
        // candidates permanently. A completed approved candidate uses the
        // existing immutable VoiceVersion registry, not an unfinished run.
        let recovery = if ready {
            Ok(None)
        } else {
            preserve_training_checkpoints(&paths)
        };
        let cancelling = current_voice_lab_build_snapshot().phase == "cancelling";
        if recovery.is_err() {
            let _ = fail_voice_lab_build(generation);
        } else if ready || cancelling {
            let _ = finish_voice_lab_build(generation);
        } else {
            let _ = fail_voice_lab_build(generation);
        }
        let message = if let Err(error) = recovery {
            format!("Training stopped, but checkpoint preservation needs attention: {error}. Current saved voice remains unchanged.")
        } else if ready {
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
                #[cfg(windows)]
                { process.owned_job = None; }
                process.terminal_message = message;
            }
            signal.notify_all();
        }
        emit_voice_build_runtime_event(event_reason, generation);
    });

    result(true, "building", "VoiceLab started creating My Voice. You can leave this page open while it works.")
}

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
    let process_identity = lock.lock().ok().map(|state| {
        (
            state.pid,
            #[cfg(windows)]
            state.owned_job.clone(),
        )
    });
    let Some(process_identity) = process_identity else {
        return result(false, "cancel_pending", "My Voice cannot verify which training process to stop.");
    };
    let Some(pid) = process_identity.0 else {
        return result(false, "cancel_pending", "VoiceLab is stopping, but the build process could not be addressed yet.");
    };
    #[cfg(windows)]
    let stopped = process_identity.1.as_ref().is_some_and(|job| job.terminate());
    #[cfg(not(windows))]
    let stopped = terminate_process_tree(pid);
    if !stopped {
        return result(false, "cancel_pending", "My Voice could not confirm the owned training process stopped.");
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

// Built-in selection replaces the canonical selected actor. Preserve any
// approved personal actor in a verified, separate store before replacing it.
fn confirmed_builtin_actor(actor_dir: &Path) -> bool {
    let Ok(bytes) = fs::read(actor_dir.join("actor.json")) else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("builtin_voice").and_then(serde_json::Value::as_bool))
        == Some(true)
}

fn voice_selection_recovery_pending(saved_root: &Path) -> bool {
    [
        saved_root.join(".MyVoice.previous"),
        saved_root.join(".MyVoice.builtin-previous"),
    ]
    .iter()
    .any(|path| path.exists())
}

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
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active() {
        return result(false, "voice_busy", "Finish the current My Voice action before switching voices.");
    }
    let _selection_guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before switching voices.");
    }

    let project_paths = ProjectPaths::discover();
    let storage = VoiceLabStoragePaths::from_project_paths(&project_paths);
    let target = storage.approved_actor_dir.clone();
    if recover_selected_actor_if_missing(&storage).is_err() {
        return result(false, "voice_recovery_required",
            "The prior Meeting voice could not be safely restored. Your saved files were kept.");
    }
    if voice_selection_recovery_pending(&storage.saved_root) {
        return result(
            false,
            "voice_recovery_required",
            "Voice switching is unavailable while a previous voice may need recovery. Your saved files were kept.",
        );
    }
    if target.exists() && !approved_voice_actor_ready(&storage) {
        return result(false, "voice_unverified",
            "The current Meeting voice could not be verified. No voice was replaced.");
    }
    if target.join("actor.json").is_file() && !authorized_voice_confirmed {
        return result(
            false,
            "approval_required",
            "Replacing the current Meeting voice needs your explicit confirmation.",
        );
    }

    if target.exists() && !confirmed_builtin_actor(&target) {
        if archive_selected_personal_voice(&storage).is_err() {
            return result(false, "personal_voice_protection_failed",
                "Your personal voice could not be safely saved. The current voice was kept.");
        }
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
pub fn get_voice_lab_actor_versions(after: Option<String>) -> Result<ActorVersionPage, String> {
    actor_version_page(&storage(), after.as_deref())
}

/// Explicit read-only check of the currently selected Meeting actor's exact
/// immutable version identity. Full model SHA-256 hashing is blocking I/O:
/// never run this on Tauri's UI thread or in routine status polling.
#[tauri::command]
pub async fn get_voice_lab_selected_version_match(version_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting()
            || voice_lab_recording_active() || super::voice_lab_preview::quick_voice_preview_active()
        {
            return Err("voice_lab:version_identity_busy".into());
        }
        let _guard = voice_selection_guard()
            .map_err(|_| "voice_lab:version_identity_lock_unavailable".to_string())?;
        // Re-check after lock acquisition: a voice resource may have changed
        // while this request waited for an earlier version selection.
        if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting()
            || voice_lab_recording_active() || super::voice_lab_preview::quick_voice_preview_active()
        {
            return Err("voice_lab:version_identity_busy".into());
        }
        selected_personal_actor_matches_version(&storage(), &version_id)
    })
    .await
    .map_err(|_| "voice_lab:version_identity_worker_failed".to_string())?
}

/// Explicit opt-in restoration from a full immutable and validated actor
/// package. Shares the existing selection lock and Meeting resource guards.
pub fn select_archived_personal_voice_version(
    version_id: String,
    replace_confirmed: bool,
) -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active",
            "Stop Meeting translation before switching voice versions.");
    }
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active()
        || super::voice_lab_preview::quick_voice_preview_active()
    {
        return result(false, "voice_busy",
            "Finish recording, voice creation, or Quick Preview before switching versions.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable",
            "Another voice change is in progress. Try again."),
    };
    if meeting_blocks_voice_change()
        || voice_lab_build_blocks_meeting()
        || voice_lab_recording_active()
        || super::voice_lab_preview::quick_voice_preview_active()
    {
        return result(false, "voice_busy",
            "Finish the current Meeting or My Voice action before switching versions.");
    }
    let paths = storage();
    if recover_selected_actor_if_missing(&paths).is_err()
        || voice_selection_recovery_pending(&paths.saved_root)
    {
        return result(false, "voice_recovery_required",
            "An earlier voice change needs recovery. Your existing voice was not replaced.");
    }
    let selected = &paths.approved_actor_dir;
    if selected.exists() && !approved_voice_actor_ready(&paths) {
        return result(false, "voice_unverified",
            "The current Meeting voice needs recovery before a version can be restored.");
    }
    if !replace_confirmed {
        return result(false, "approval_required",
            "Confirm that you want to use this saved version for future meetings.");
    }
    match activate_archived_personal_voice(&paths, &version_id) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            match archive_selected_personal_voice(&paths) {
                Ok(()) => result(true, "selected",
                    "Saved voice version is now selected for meetings. Other versions were kept."),
                Err(_) => result(true, "selected_archive_attention",
                    "Voice version is selected, but its extra saved personal copy needs attention. The immutable version is still retained."),
            }
        }
        Err(_) => result(false, "version_restore_failed",
            "Couldn't verify or select this voice version. Check My Voice recovery status before trying again."),
    }
}

pub fn select_stored_personal_voice() -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before switching voices.");
    }
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active() {
        return result(false, "voice_busy", "Finish the current My Voice action before switching voices.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before switching voices.");
    }
    let paths = storage();
    if recover_selected_actor_if_missing(&paths).is_err() {
        return result(false, "voice_recovery_required",
            "The prior Meeting voice could not be safely restored. Your saved files were kept.");
    }
    if voice_selection_recovery_pending(&paths.saved_root) {
        return result(false, "voice_recovery_required",
            "An earlier voice change needs recovery. The saved voice was not replaced.");
    }
    match activate_stored_personal_voice(&paths) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            result(true, "selected", "Your saved My Voice is selected for Meeting.")
        }
        Err(_) => result(false, "saved_voice_unavailable",
            "Your saved My Voice could not be selected. The current voice was kept."),
    }
}

pub fn approve_voice_lab_candidate(
    reviewed_line_ids: Vec<u32>,
    quality_confirmed: bool,
    review_id: String,
) -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before selecting My Voice.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    // Recheck after acquiring the selection lock: Meeting may have started
    // while another voice change held this lock.
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before selecting My Voice.");
    }
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Wait for VoiceLab creation to finish before approving My Voice.");
    }
    let paths = storage();
    let ready_for_review = child_status(&paths)
        .map(|status| status.phase == "ready_for_review")
        .unwrap_or(false) && reviewable_evaluation(&paths)
        .is_some_and(|evaluation| completion_matches(&paths, &evaluation.review_id));
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
    if review_id != evaluation.review_id {
        return result(false, "evaluation_candidate_mismatch", "The voice changed. Listen to the current previews before approving.");
    }
    if !evaluation_review_complete(&evaluation, &reviewed_line_ids) {
        return result(
            false,
            "evaluation_listening_required",
            "Finish listening to every My Voice preview before approving it.",
        );
    }
    if !quality_confirmed {
        return result(
            false,
            "evaluation_quality_confirmation_required",
            "Confirm that the voice is clear, natural, and resembles the authorized speaker before using it.",
        );
    }
    if !held_out_wavs_intact(&paths, &evaluation) {
        return result(false, "evaluation_audio_integrity_failed", "Audio changed; rebuild.");
    }
    // Approval changes the actor selected by Meeting. A late-started session
    // must not be allowed to receive an unverified actor change.
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before selecting My Voice.");
    }
    let project_paths = ProjectPaths::discover();
    match promote_voice_actor_candidate(&project_paths) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            let paths = VoiceLabStoragePaths::from_project_paths(&project_paths);
            clear_build_terminal_message();
            let archive = archive_selected_personal_voice(&paths);
            let cleanup = cleanup_approved_build_workspace(&paths);
            if archive.is_err() {
                result(true, "approved_archive_attention",
                    "My Voice is selected, but its separate saved copy could not be verified. Switching voices remains blocked until it is preserved.")
            } else if cleanup.is_err() {
                result(true, "approved_cleanup_attention",
                    "My Voice was saved, but some temporary build files could not be cleared yet.")
            } else {
                result(true, "approved", "My Voice was approved, saved, and selected for Meeting.")
            }
        }
        Err(error) => result(false, "approval_failed", error),
    }
}

#[tauri::command]
pub fn get_voice_lab_evaluation_audio(
    line_id: u32,
    review_id: String,
) -> Result<tauri::ipc::Response, String> {
    let paths = storage();
    let manifest = reviewable_evaluation(&paths)
        .ok_or_else(|| "voice_lab:evaluation_unavailable".to_string())?;
    if review_id != manifest.review_id {
        return Err("voice_lab:evaluation_review_changed".to_string());
    }
    let sample = manifest
        .samples
        .into_iter()
        .find(|sample| sample.line_id == line_id)
        .ok_or_else(|| "voice_lab:evaluation_line_missing".to_string())?;
    let bytes = verified_held_out_wav(&evaluation_dir(&paths), &sample)
        .ok_or_else(|| "voice_lab:evaluation_audio_integrity_failed".to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
#[path = "voice_lab_build_tests.rs"]
mod p1_recording_coverage_tests;

#[cfg(test)]
mod builtin_selection_protection_tests {
    use super::{confirmed_builtin_actor, voice_selection_recovery_pending};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn personal_and_invalid_manifests_are_not_treated_as_builtin() {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit_builtin_guard_{id}"));
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join("actor.json");
        fs::write(&manifest, r#"{"schema_version":1,"engine":"gpt-sovits-v2proplus"}"#).unwrap();
        assert!(!confirmed_builtin_actor(&root));
        fs::write(&manifest, b"invalid json").unwrap();
        assert!(!confirmed_builtin_actor(&root));
        fs::write(&manifest, r#"{"builtin_voice":true}"#).unwrap();
        assert!(confirmed_builtin_actor(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unfinished_voice_replacement_blocks_new_builtin_selection() {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit_voice_recovery_{id}"));
        fs::create_dir_all(&root).unwrap();
        assert!(!voice_selection_recovery_pending(&root));
        fs::create_dir_all(root.join(".MyVoice.builtin-previous")).unwrap();
        assert!(voice_selection_recovery_pending(&root));
        let _ = fs::remove_dir_all(root);
    }
}

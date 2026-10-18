use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::engine::audio::guided_take::{
    active_guided_take_line_id, arm_guided_take, cancel_guided_take, take_guided_audio,
    GuidedTakeReview, GUIDED_TAKE_CHANNELS, GUIDED_TAKE_SAMPLE_RATE_HZ,
};
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::audio::live_segment_writer::write_pcm16_wav;
use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{
    begin_voice_recording_session, clear_runtime_session_if_generation,
    latest_runtime_session_state, mark_runtime_session_cleanup_incomplete,
    revoke_runtime_session_authority, VOICE_RECORDING_OWNER_ID,
};

use super::voice_lab::{training_recovery_status, TrainingRecoveryStatus, create_voice_backup, check_voice_backup, restore_voice_backup, voice_backup_active, BackupReport, archive_guided_take, current_voice_lab_build_snapshot, legacy_protection_status, migrate_legacy_recordings, LegacyMigrationResult, LegacyProtectionStatus, SavedRecordingPage, saved_recordings_page, saved_recording_audio, VoiceLabStoragePaths};

mod guided_lines;
mod storage_transaction;

use guided_lines::{guided_lines, prompt_metadata};
pub(crate) use guided_lines::RecordingSuggestion;
use storage_transaction::{accept_review_take_with_native_rate, discard_review_take};

const MAX_REPLAY_WAV_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct GuidedLineStatus {
    pub line_id: u32,
    pub text: String,
    pub accepted: bool,
    pub level: &'static str,
    pub focus: &'static str,
    pub coverage: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidedRecordingState {
    pub recording_line_id: Option<u32>,
    pub pending_review: Option<GuidedTakeReview>,
    pub lines: Vec<GuidedLineStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidedRecordingActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub recording: GuidedRecordingState,
}

struct PendingDraft {
    line_id: u32,
    path: PathBuf,
    native_rate_path: PathBuf,
    review: GuidedTakeReview,
}

static PENDING_DRAFT: OnceLock<Mutex<Option<PendingDraft>>> = OnceLock::new();

fn draft_store() -> &'static Mutex<Option<PendingDraft>> {
    PENDING_DRAFT.get_or_init(|| Mutex::new(None))
}

fn guided_text(line_id: u32) -> Option<&'static str> {
    guided_lines()
        .iter()
        .find(|(id, _)| *id == line_id)
        .map(|(_, text)| *text)
}

// Heuristic only: a very short take may omit words from the known guided script.
// Never replace the exact guided transcript with guessed ASR or reject this take.
fn unusually_short_for_line(line_id: u32, duration_ms: u64) -> bool {
    let Some(line) = guided_text(line_id) else { return false; };
    let words = line.split_whitespace().count() as u64;
    words >= 8 && duration_ms > 0 && words.saturating_mul(1_000) > duration_ms.saturating_mul(6)
}

fn take_file_name(line_id: u32) -> String {
    format!("take_{line_id:04}.wav")
}

fn storage_paths() -> VoiceLabStoragePaths {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
}

fn draft_path(line_id: u32) -> PathBuf {
    storage_paths()
        .cache_root
        .join("Draft")
        .join(take_file_name(line_id))
}

fn accepted_path(line_id: u32) -> PathBuf {
    storage_paths().takes_dir.join(take_file_name(line_id))
}

fn draft_native_rate_path(line_id: u32) -> PathBuf {
    storage_paths()
        .cache_root
        .join("Draft")
        .join("NativeRate")
        .join(take_file_name(line_id))
}

fn accepted_native_rate_path(line_id: u32) -> PathBuf {
    storage_paths()
        .takes_dir
        .join("NativeRate")
        .join(take_file_name(line_id))
}

fn current_state() -> GuidedRecordingState {
    let pending_review = draft_store()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|draft| draft.review.clone()));
    GuidedRecordingState {
        recording_line_id: active_guided_take_line_id(),
        pending_review,
        lines: guided_lines()
            .iter()
            .map(|(line_id, text)| GuidedLineStatus {
                line_id: *line_id,
                text: (*text).to_string(),
                accepted: accepted_path(*line_id).is_file(),
                level: prompt_metadata(*line_id).map(|m| m.level).unwrap_or("unknown"),
                focus: prompt_metadata(*line_id).map(|m| m.focus).unwrap_or("unknown"),
                coverage: prompt_metadata(*line_id).map(|m| m.coverage).unwrap_or("unknown"),
            })
            .collect(),
    }
}

fn result(ok: bool, state: &str, message: impl Into<String>) -> GuidedRecordingActionResult {
    GuidedRecordingActionResult {
        ok,
        state: state.to_string(),
        message: message.into(),
        recording: current_state(),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptedGuidedRecording {
    pub(crate) line_id: u32,
    pub(crate) text: &'static str,
    pub(crate) path: PathBuf,
}

pub(crate) fn voice_lab_recording_active() -> bool {
    active_guided_take_line_id().is_some()
}

pub(crate) fn guided_training_prompts() -> &'static [(u32, &'static str)] {
    guided_lines()
}

pub(crate) fn guided_training_coverage(line_id: u32) -> Option<&'static str> {
    guided_lines::coverage_for_id(line_id)
}

pub(crate) fn guided_recording_recommendations(
    accepted_line_ids: &[u32],
    required_coverage: Option<&str>,
) -> Vec<RecordingSuggestion> {
    guided_lines::recommend_lines(accepted_line_ids, required_coverage, 3)
}

pub(crate) fn accepted_guided_recordings() -> Vec<AcceptedGuidedRecording> {
    guided_lines()
        .iter()
        .filter_map(|(line_id, text)| {
            let path = accepted_path(*line_id);
            path.is_file().then_some(AcceptedGuidedRecording {
                line_id: *line_id,
                text,
                path,
            })
        })
        .collect()
}

pub(crate) fn voice_lab_recording_blocks_app_exit() -> bool {
    if voice_lab_recording_active() {
        return true;
    }
    draft_store()
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(true)
}

#[tauri::command]
pub fn get_voice_lab_guided_recording_state() -> GuidedRecordingState {
    current_state()
}

pub fn start_voice_lab_guided_take(
    line_id: u32,
    authorized_voice_confirmed: bool,
) -> GuidedRecordingActionResult {
    if voice_backup_active() {
        return result(false, "backup_active", "Finish backing up or restoring My Voice before recording.");
    }
    if !authorized_voice_confirmed {
        return result(false, "authorization_required", "Confirm that you own or are authorized to use this voice before recording.");
    }
    if guided_text(line_id).is_none() {
        return result(false, "invalid_line", "This guided reading line is not part of the current VoiceLab script.");
    }
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Finish or cancel the current VoiceLab build before recording more lines.");
    }
    if draft_store().lock().ok().and_then(|guard| guard.as_ref().map(|draft| draft.line_id)).is_some() {
        return result(false, "review_pending", "Replay, retry, or accept the current take before recording another line.");
    }

    // A missing runtime session snapshot does not prove Meeting audio cleanup
    // released the physical microphone. The user must finish Retry Stop first.
    if super::meeting_session::meeting_stop_cleanup_pending() {
        return result(false, "meeting_cleanup_incomplete",
            "Retry Stop Meeting before recording a My Voice line.");
    }
    let session = begin_voice_recording_session();
    let Some(snapshot) = session.snapshot.as_ref() else {
        return result(false, "runtime_unavailable", "VoiceLab cannot verify microphone ownership right now.");
    };
    if !session.blocker.is_empty() || snapshot.owner_id != VOICE_RECORDING_OWNER_ID {
        return result(false, "microphone_in_use", "Stop Meeting translation or Mic Test before recording a VoiceLab line.");
    }
    let generation = snapshot.generation;
    if let Err(blocker) = arm_guided_take(line_id) {
        let _ = clear_runtime_session_if_generation(generation);
        return result(false, "capture_unavailable", blocker);
    }
    let capture = start_live_capture_runtime(session);
    if !capture.ok {
        cancel_guided_take();
        let _ = clear_runtime_session_if_generation(generation);
        return result(false, "capture_failed", capture.message);
    }
    result(true, "recording", "Recording started. Read the line naturally, then press Stop.")
}

pub fn stop_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    if active_guided_take_line_id() != Some(line_id) {
        return result(false, "line_mismatch", "The requested line is not the guided take currently being recorded.");
    }
    let current = latest_runtime_session_state();
    let Some(snapshot) = current.snapshot.as_ref() else {
        return result(false, "runtime_unavailable", "VoiceLab cannot verify microphone ownership right now.");
    };
    if snapshot.owner_id != VOICE_RECORDING_OWNER_ID {
        return result(false, "owner_conflict", "VoiceLab does not own the active microphone session.");
    }
    let generation = snapshot.generation;
    if snapshot.authority_active {
        let revoked = revoke_runtime_session_authority(
            generation,
            "VoiceLab Stop accepted. Capture authority was revoked before microphone cleanup.",
        );
        if revoked.snapshot.as_ref().map(|value| value.authority_active).unwrap_or(true) {
            return result(false, "stop_failed", "VoiceLab could not revoke recording ownership safely.");
        }
    }

    let stopped = stop_live_capture_runtime();
    if !stopped.ok {
        let _ = mark_runtime_session_cleanup_incomplete(
            generation,
            true,
            "VoiceLab recording authority is revoked, but microphone cleanup still needs attention.",
        );
        return result(false, "stop_failed", stopped.message);
    }
    let captured = take_guided_audio();
    let cleared = clear_runtime_session_if_generation(generation);
    if cleared.has_active_session || cleared.snapshot.is_some() {
        return result(false, "cleanup_unverified", "The microphone stopped, but VoiceLab could not confirm that recording ownership was cleared.");
    }
    let Ok((captured, mut review)) = captured else {
        return result(false, "take_unusable", captured.err().unwrap_or_else(|| "voice_lab:take_unusable".to_string()));
    };
    if captured.line_id != line_id {
        return result(false, "line_mismatch", "The recorded audio does not belong to the requested guided line.");
    }
    if review.quality_blocker.is_empty() && unusually_short_for_line(line_id, review.duration_ms) {
        review.review_advice.push("voice_lab:review:verify_all_words".to_string());
    }
    let draft = draft_path(line_id);
    let native_rate_draft = draft_native_rate_path(line_id);
    // Write native-rate mono PCM16 before the canonical 32 kHz review WAV.
    // Both live in disposable Draft storage until explicit acceptance.
    if let Err(error) = write_pcm16_wav(
        &native_rate_draft,
        captured.source_sample_rate_hz,
        GUIDED_TAKE_CHANNELS,
        &captured.native_rate_samples_mono,
    ) {
        let _ = fs::remove_file(&native_rate_draft);
        let _ = fs::remove_file(&draft);
        return result(false, "draft_write_failed", format!("VoiceLab could not save the native-rate take: {error}"));
    }
    if let Err(error) = write_pcm16_wav(
        &draft,
        GUIDED_TAKE_SAMPLE_RATE_HZ,
        GUIDED_TAKE_CHANNELS,
        &captured.samples_mono,
    ) {
        let _ = fs::remove_file(&native_rate_draft);
        let _ = fs::remove_file(&draft);
        return result(false, "draft_write_failed", format!("VoiceLab could not save the review take: {error}"));
    }
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => {
            let _ = fs::remove_file(&native_rate_draft);
            let _ = fs::remove_file(&draft);
            return result(
                false,
                "draft_state_unavailable",
                "VoiceLab could not retain the review take state; the unowned review audio was removed.",
            );
        }
    };
    *guard = Some(PendingDraft {
        line_id,
        path: draft,
        native_rate_path: native_rate_draft,
        review,
    });
    drop(guard);
    result(true, "needs_review", "Recording stopped. Replay the take, then accept it or retry the line.")
}

#[tauri::command]
pub fn retry_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => return result(false, "draft_state_unavailable", "VoiceLab cannot access the pending review take."),
    };
    let Some(draft) = guard.as_ref() else {
        drop(guard);
        return result(true, "ready", "There is no pending review take to discard.");
    };
    if draft.line_id != line_id {
        drop(guard);
        return result(false, "line_mismatch", "The pending review take belongs to another guided line.");
    }
    let path = draft.path.clone();
    let native_rate_path = draft.native_rate_path.clone();
    if let Err(error) = discard_review_take(&native_rate_path) {
        drop(guard);
        return result(false, "discard_failed", error);
    }
    if let Err(error) = discard_review_take(&path) {
        drop(guard);
        return result(false, "discard_failed", error);
    }
    *guard = None;
    drop(guard);
    result(true, "ready", "The review take was discarded. The previous accepted take, if any, was kept.")
}

#[tauri::command]
pub fn accept_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    if voice_backup_active() {
        return result(false, "backup_active", "Finish backing up or restoring My Voice before saving another recording.");
    }
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Wait for My Voice creation to finish before accepting a new recording.");
    }
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => return result(false, "draft_state_unavailable", "VoiceLab cannot access the pending review take."),
    };
    let Some(draft) = guard.as_ref() else {
        drop(guard);
        return result(false, "no_review", "Record and review this line before accepting it.");
    };
    if draft.line_id != line_id {
        drop(guard);
        return result(false, "line_mismatch", "The pending review take belongs to another guided line.");
    }
    if !draft.review.quality_blocker.is_empty() {
        drop(guard);
        return result(false, "take_unusable", "This take is silent or empty. Retry the line before accepting it.");
    }
    let target = accepted_path(line_id);
    if let Some(parent) = target.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            drop(guard);
            return result(false, "save_failed", format!("VoiceLab could not prepare take storage: {error}"));
        }
    }
    // Preserve the old accepted bytes and the newly approved draft in immutable
    // SavedProject storage BEFORE the legacy active take can be replaced.
    // A failed archive means no destructive active-take change is allowed.
    let storage = storage_paths();
    let canonical_old = accepted_path(line_id);
    let native_old = accepted_native_rate_path(line_id);
    let Some(exact_text) = guided_text(line_id) else {
        drop(guard);
        return result(false, "invalid_line", "The selected guided prompt is no longer available.");
    };
    if fs::symlink_metadata(&canonical_old).is_ok() {
        let old_native = fs::symlink_metadata(&native_old).ok().map(|_| native_old.as_path());
        if let Err(error) = archive_guided_take(
            &storage, line_id, exact_text, &canonical_old, old_native,
        ) {
            drop(guard);
            return result(false, "save_failed", format!(
                "The existing accepted recording could not be archived safely; nothing was replaced: {error}"
            ));
        }
    }
    // Interrupted old backups may be the only surviving previous take.
    // Preserve them before the legacy accept transaction can clear *.previous.
    let previous = canonical_old.with_extension("wav.previous");
    let native_previous = native_old.with_extension("wav.previous");
    match fs::symlink_metadata(&previous) {
        Ok(_) => {
            let old_native = fs::symlink_metadata(&native_previous)
                .ok().map(|_| native_previous.as_path());
            if let Err(error) = archive_guided_take(
                &storage, line_id, exact_text, &previous, old_native,
            ) {
                drop(guard);
                return result(false, "save_failed", format!(
                    "The previous take backup needs protection before replacing it: {error}"
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            drop(guard);
            return result(false, "save_failed",
                "Couldn't check the previous recording backup. Nothing was replaced.");
        }
    }
    if let Err(error) = archive_guided_take(
        &storage, line_id, exact_text, &draft.path, Some(&draft.native_rate_path),
    ) {
        drop(guard);
        return result(false, "save_failed", format!(
            "The new recording could not be archived safely; the previous accepted recording was kept: {error}"
        ));
    }
    if let Err(error) = accept_review_take_with_native_rate(
        &draft.path,
        &target,
        &draft.native_rate_path,
        &accepted_native_rate_path(line_id),
    ) {
        drop(guard);
        return result(false, "save_failed", error);
    }
    *guard = None;
    drop(guard);
    result(true, "accepted", "Take accepted and saved for the Voice Actor dataset.")
}



#[tauri::command]
pub fn list_voice_lab_saved_recordings(after: Option<String>) -> Result<SavedRecordingPage, String> {
    saved_recordings_page(&storage_paths(), after.as_deref())
}

#[tauri::command]
pub fn get_voice_lab_saved_recording_audio(archive_id: String) -> Result<tauri::ipc::Response, String> {
    let bytes = saved_recording_audio(&storage_paths(), &archive_id)?;
    Ok(tauri::ipc::Response::new(bytes))
}


fn backup_operation_allowed() -> Result<(), String> {
    if voice_backup_active()
        || voice_lab_recording_active()
        || current_voice_lab_build_snapshot().active
        || super::voice_lab_preview::quick_voice_preview_active()
        || super::builtin_voice::meeting_blocks_voice_change()
    {
        return Err("Finish Meeting, recording, or voice creation before using backup and restore.".into());
    }
    let draft = draft_store().lock()
        .map_err(|_| "Could not verify pending recording status.".to_string())?;
    if draft.is_some() {
        return Err("Finish reviewing the pending recording before backup or restore.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn create_voice_lab_backup(destination_dir: String) -> Result<BackupReport, String> {
    let _selection_guard = super::voice_lab_build::voice_selection_guard()
        .map_err(|_| "Another voice change or backup is in progress.".to_string())?;
    backup_operation_allowed()?;
    let migration = migrate_legacy_recordings(&storage_paths(), guided_lines());
    if !migration.ok {
        return Err("Older recordings still need protection. Review the saved recording warning before creating a complete backup.".into());
    }
    create_voice_backup(&storage_paths(), &destination_dir)
}

#[tauri::command]
pub fn verify_voice_lab_backup(backup_folder: String) -> Result<BackupReport, String> {
    let _selection_guard = super::voice_lab_build::voice_selection_guard()
        .map_err(|_| "Another voice change or backup is in progress.".to_string())?;
    backup_operation_allowed()?;
    check_voice_backup(&backup_folder)
}

#[tauri::command]
pub fn restore_voice_lab_backup(
    backup_folder: String, confirmed: bool,
) -> Result<BackupReport, String> {
    let _selection_guard = super::voice_lab_build::voice_selection_guard()
        .map_err(|_| "Another voice change or backup is in progress.".to_string())?;
    backup_operation_allowed()?;
    if !confirmed {
        return Err("Confirm that this backup contains your own or an authorized voice.".into());
    }
    restore_voice_backup(&storage_paths(), &backup_folder)
}


#[tauri::command]
pub fn get_voice_lab_training_recovery_status() -> Result<TrainingRecoveryStatus, String> {
    training_recovery_status(&storage_paths())
}

#[tauri::command]
pub fn get_voice_lab_legacy_archive_status() -> LegacyProtectionStatus {
    legacy_protection_status(&storage_paths(), guided_lines())
}

#[tauri::command]
pub fn protect_voice_lab_legacy_recordings() -> Result<LegacyMigrationResult, String> {
    if voice_lab_recording_active() || current_voice_lab_build_snapshot().active {
        return Err("Finish recording or voice creation before protecting previous recordings.".to_string());
    }
    let pending = draft_store().lock()
        .map_err(|_| "Recording status is unavailable.".to_string())?
        .is_some();
    if pending {
        return Err("Review the current recording before protecting earlier recordings.".to_string());
    }
    Ok(migrate_legacy_recordings(&storage_paths(), guided_lines()))
}

#[tauri::command]
pub fn get_voice_lab_guided_take_audio(line_id: u32) -> Result<tauri::ipc::Response, String> {
    if guided_text(line_id).is_none() {
        return Err("voice_lab:invalid_guided_line".to_string());
    }
    let pending_path = draft_store()
        .lock()
        .map_err(|_| "voice_lab:guided_draft_state_unavailable".to_string())?
        .as_ref()
        .filter(|draft| draft.line_id == line_id)
        .map(|draft| draft.path.clone());
    let path = pending_path.unwrap_or_else(|| accepted_path(line_id));
    let metadata = fs::metadata(&path).map_err(|_| "voice_lab:take_audio_missing".to_string())?;
    if !metadata.is_file() || metadata.len() < 44 || metadata.len() > MAX_REPLAY_WAV_BYTES {
        return Err("voice_lab:take_audio_invalid".to_string());
    }
    let bytes = fs::read(path).map_err(|_| "voice_lab:take_audio_read_failed".to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
mod review_advice_tests {
    use super::unusually_short_for_line;

    #[test]
    fn short_take_hint_is_conservative_and_does_not_require_asr() {
        assert!(unusually_short_for_line(31, 1_000));
        assert!(!unusually_short_for_line(31, 4_000));
        assert!(!unusually_short_for_line(2, 100));
        assert!(!unusually_short_for_line(999, 1));
        assert!(!unusually_short_for_line(31, 0));
    }
}

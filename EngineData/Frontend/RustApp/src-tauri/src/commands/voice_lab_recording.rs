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
    latest_runtime_session_state,
    mark_runtime_session_cleanup_incomplete, revoke_runtime_session_authority,
};

use super::voice_lab::{current_voice_lab_build_snapshot, VoiceLabStoragePaths};

mod guided_lines;
mod storage_transaction;

use guided_lines::GUIDED_LINES;
use storage_transaction::{accept_review_take, discard_review_take};

use crate::engine::runtime_state::VOICE_RECORDING_OWNER_ID;
const MAX_REPLAY_WAV_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct GuidedLineStatus {
    pub line_id: u32,
    pub text: String,
    pub accepted: bool,
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
    review: GuidedTakeReview,
}

static PENDING_DRAFT: OnceLock<Mutex<Option<PendingDraft>>> = OnceLock::new();

fn draft_store() -> &'static Mutex<Option<PendingDraft>> {
    PENDING_DRAFT.get_or_init(|| Mutex::new(None))
}

fn guided_text(line_id: u32) -> Option<&'static str> {
    GUIDED_LINES
        .iter()
        .find(|(id, _)| *id == line_id)
        .map(|(_, text)| *text)
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

fn current_state() -> GuidedRecordingState {
    let pending_review = draft_store()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|draft| draft.review.clone()));
    GuidedRecordingState {
        recording_line_id: active_guided_take_line_id(),
        pending_review,
        lines: GUIDED_LINES
            .iter()
            .map(|(line_id, text)| GuidedLineStatus {
                line_id: *line_id,
                text: (*text).to_string(),
                accepted: accepted_path(*line_id).is_file(),
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

pub(crate) fn accepted_guided_recordings() -> Vec<AcceptedGuidedRecording> {
    GUIDED_LINES
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

#[tauri::command]
pub fn start_voice_lab_guided_take(
    line_id: u32,
    authorized_voice_confirmed: bool,
) -> GuidedRecordingActionResult {
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

#[tauri::command]
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
    let Ok((captured, review)) = captured else {
        return result(false, "take_unusable", captured.err().unwrap_or_else(|| "voice_lab:take_unusable".to_string()));
    };
    if captured.line_id != line_id {
        return result(false, "line_mismatch", "The recorded audio does not belong to the requested guided line.");
    }
    let draft = draft_path(line_id);
    if let Err(error) = write_pcm16_wav(
        &draft,
        GUIDED_TAKE_SAMPLE_RATE_HZ,
        GUIDED_TAKE_CHANNELS,
        &captured.samples_mono,
    ) {
        let _ = fs::remove_file(&draft);
        return result(false, "draft_write_failed", format!("VoiceLab could not save the review take: {error}"));
    }
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => {
            let _ = fs::remove_file(&draft);
            return result(
                false,
                "draft_state_unavailable",
                "VoiceLab could not retain the review take state; the unowned review audio was removed.",
            );
        }
    };
    *guard = Some(PendingDraft { line_id, path: draft, review });
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
    if let Err(error) = accept_review_take(&draft.path, &target) {
        drop(guard);
        return result(false, "save_failed", error);
    }
    *guard = None;
    drop(guard);
    result(true, "accepted", "Take accepted and saved for the Voice Actor dataset.")
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

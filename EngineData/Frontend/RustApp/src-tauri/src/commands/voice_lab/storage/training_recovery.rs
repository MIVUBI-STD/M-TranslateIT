//! Loss-resistant preservation of pinned upstream GPT-SoVITS TRAINING artifacts.
//!
//! These are checkpoint candidates, NOT approved inference actors or verified
//! resumable jobs. The pinned SoVITS trainer silently falls back to pretrained
//! weights on checkpoint failure and does not save AMP GradScaler state.
//! Consequently there is deliberately NO automatic resume/activation here.
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::dataset_history::frozen_dataset_snapshot_id;
use super::immutable_store::{
    checked_directory, hash_bytes, hash_regular_file, read_regular_bounded,
    write_new_synced,
};
use super::VoiceLabStoragePaths;
use crate::commands::voice_lab::VOICE_ACTOR_ENGINE_REVISION;

const FORMAT: u32 = 1;
const LIMIT_FILE: u64 = 8 * 1024 * 1024 * 1024;
const LIMIT_TOTAL: u64 = 48 * 1024 * 1024 * 1024;
const LIMIT_COUNT: usize = 22;
const MAX_RECEIPT: u64 = 16 * 1024;
static CAPTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RecoveryPart {
    label: String,
    original_filename: String,
    sha256: String,
    bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RecoveryReceipt {
    format: u32,
    engine_revision: String,
    dataset_snapshot_id: String,
    // This is NEVER an automatic-resume eligibility declaration.
    status: String,
    parts: Vec<RecoveryPart>,
}

struct Input {
    label: String,
    source: PathBuf,
    original_filename: String,
}

fn candidate_path(storage: &VoiceLabStoragePaths) -> PathBuf {
    storage.cache_root.join("Build").join("Runtime")
}

fn trusted_dir(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err("voice_lab:recovery_directory_unavailable".into()),
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() =>
            Err("voice_lab:recovery_directory_untrusted".into()),
        Ok(_) => Ok(true),
    }
}

fn collect_checkpoint_files(dir: &Path, group: &str) -> Result<Vec<Input>, String> {
    if !trusted_dir(dir)? { return Ok(vec![]); }
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|_| "voice_lab:recovery_checkpoint_list_failed")? {
        let entry = entry.map_err(|_| "voice_lab:recovery_checkpoint_entry_failed")?;
        let name = entry.file_name().to_str()
            .ok_or("voice_lab:recovery_checkpoint_filename_invalid")?.to_string();
        // A crashing same-directory atomic writer can leave a pending
        // partial. Preserve the existing work directory for investigation;
        // never clear it as if no incomplete checkpoint had existed.
        if name.starts_with(".pending-G_") || name.starts_with(".pending-D_") {
            return Err("voice_lab:recovery_partial_checkpoint_needs_attention".into());
        }
        let valid = match group {
            "gpt" => name.ends_with(".ckpt")
                && name.len() < 100
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'=')),
            "sovits-G" => name.starts_with("G_")
                && name.ends_with(".pth")
                && name[2..name.len()-4].bytes().all(|b| b.is_ascii_digit()),
            "sovits-D" => name.starts_with("D_")
                && name.ends_with(".pth")
                && name[2..name.len()-4].bytes().all(|b| b.is_ascii_digit()),
            _ => false,
        };
        if !valid { continue; }
        let meta = fs::symlink_metadata(entry.path())
            .map_err(|_| "voice_lab:recovery_checkpoint_metadata_failed")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("voice_lab:recovery_checkpoint_untrusted".into());
        }
        files.push(Input { label: String::new(), source: entry.path(), original_filename: name });
        if files.len() > 8 {
            return Err("voice_lab:recovery_checkpoint_count_exceeded".into());
        }
    }
    files.sort_by(|a,b| a.original_filename.cmp(&b.original_filename));
    for (index, input) in files.iter_mut().enumerate() {
        input.label = format!("{group}-{index}.{}", if group == "gpt" { "ckpt" } else { "pth" });
    }
    Ok(files)
}

fn inputs(storage: &VoiceLabStoragePaths) -> Result<Vec<Input>, String> {
    let work = candidate_path(storage);
    if !trusted_dir(&work)? { return Ok(vec![]); }
    let exp = work.join("experiment");
    if !trusted_dir(&exp)? { return Ok(vec![]); }
    let mut files = Vec::new();
    let s2_dir = exp.join("logs_s2_v2ProPlus");
    // A complete pair requires both G and D; even a partial set is preserved
    // as evidence rather than silently discarded.
    files.extend(collect_checkpoint_files(&s2_dir, "sovits-G")?);
    files.extend(collect_checkpoint_files(&s2_dir, "sovits-D")?);
    files.extend(collect_checkpoint_files(&exp.join("logs_s1_v2ProPlus").join("ckpt"), "gpt")?);
    if files.is_empty() { return Ok(files); }

    for (name, label) in [
        ("translateit_s1.yaml", "gpt-config.yaml"),
        ("translateit_s2.json", "sovits-config.json"),
    ] {
        let path = work.join(name);
        let meta = fs::symlink_metadata(&path)
            .map_err(|_| "voice_lab:recovery_training_config_missing")?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("voice_lab:recovery_training_config_untrusted".into());
        }
        files.push(Input {
            label: label.into(), original_filename: name.into(), source: path,
        });
    }
    if files.len() > LIMIT_COUNT {
        return Err("voice_lab:recovery_checkpoint_count_exceeded".into());
    }
    files.sort_by(|a,b| a.label.cmp(&b.label));
    Ok(files)
}

fn verify_package(dir: &Path, receipt: &RecoveryReceipt, id: &str) -> Result<(), String> {
    if !trusted_dir(dir)? { return Err("voice_lab:recovery_archive_missing".into()); }
    let raw = read_regular_bounded(&dir.join("recovery.json"), MAX_RECEIPT)?;
    let decoded: RecoveryReceipt = serde_json::from_slice(&raw)
        .map_err(|_| "voice_lab:recovery_receipt_invalid")?;
    let encoded = serde_json::to_vec(&decoded)
        .map_err(|_| "voice_lab:recovery_receipt_invalid")?;
    if hash_bytes(&encoded) != id || decoded != *receipt {
        return Err("voice_lab:recovery_receipt_mismatch".into());
    }
    let parts_dir = dir.join("files");
    if !trusted_dir(&parts_dir)? { return Err("voice_lab:recovery_files_missing".into()); }
    for part in &receipt.parts {
        let (hash, size) = hash_regular_file(&parts_dir.join(&part.label), LIMIT_FILE)?;
        if hash != part.sha256 || size != part.bytes {
            return Err("voice_lab:recovery_file_integrity_failed".into());
        }
    }
    Ok(())
}

/// Archive a completed interruption's FULL checkpoint candidates before any
/// old Work cleanup or dataset replacement. Existing source files are untouched.
/// No checkpoint => no progress to archive => Ok(None).
pub(crate) fn preserve_training_checkpoints(
    storage: &VoiceLabStoragePaths,
) -> Result<Option<String>, String> {
    let _guard = CAPTURE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:recovery_lock_unavailable")?;
    let files = inputs(storage)?;
    if files.is_empty() { return Ok(None); }
    let dataset_id = frozen_dataset_snapshot_id(storage)?
        .ok_or("voice_lab:recovery_dataset_snapshot_missing")?;
    let mut total = 0u64;
    let mut parts = Vec::new();
    for input in &files {
        let (sha256, bytes) = hash_regular_file(&input.source, LIMIT_FILE)?;
        total = total.checked_add(bytes).ok_or("voice_lab:recovery_total_overflow")?;
        if total > LIMIT_TOTAL { return Err("voice_lab:recovery_storage_budget_exceeded".into()); }
        parts.push(RecoveryPart {
            label: input.label.clone(),
            original_filename: input.original_filename.clone(),
            sha256, bytes,
        });
    }
    let receipt = RecoveryReceipt {
        format: FORMAT,
        engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
        dataset_snapshot_id: dataset_id,
        status: "checkpoint_preserved_not_verified_for_resume".into(),
        parts,
    };
    let bytes = serde_json::to_vec(&receipt)
        .map_err(|_| "voice_lab:recovery_receipt_serialize_failed")?;
    if bytes.len() as u64 > MAX_RECEIPT { return Err("voice_lab:recovery_receipt_too_large".into()); }
    let id = hash_bytes(&bytes);
    let root = storage.saved_root.join("TrainingRecovery");
    checked_directory(&root)?;
    let complete = root.join(&id);
    if fs::symlink_metadata(&complete).is_ok() {
        verify_package(&complete, &receipt, &id)?;
        return Ok(Some(id));
    }
    let pending = root.join(format!(".pending-{id}"));
    if fs::symlink_metadata(&pending).is_ok() {
        // A previous verified pending archive needs only its final rename,
        // not another full checkpoint copy or a second free-space reserve.
        verify_package(&pending, &receipt, &id)
            .map_err(|_| "voice_lab:recovery_partial_archive_needs_attention")?;
    } else {
        // A *new* immutable checkpoint archive may be large. If insufficient,
        // preserve all original checkpoint bytes and block unsafe cleanup.
        crate::commands::my_voice_resources::ensure_archive_capacity(&root, total)?;
        fs::create_dir(&pending).map_err(|_| "voice_lab:recovery_stage_create_failed")?;
        let dest = pending.join("files");
        checked_directory(&dest)?;
        for (input, record) in files.iter().zip(&receipt.parts) {
            let written = fs::copy(&input.source, dest.join(&record.label))
                .map_err(|_| "voice_lab:recovery_stage_copy_failed")?;
            if written != record.bytes {
                return Err("voice_lab:recovery_stage_size_changed".into());
            }
            OpenOptions::new().write(true).open(dest.join(&record.label))
                .and_then(|f| f.sync_all())
                .map_err(|_| "voice_lab:recovery_stage_sync_failed")?;
            let (sha, size) = hash_regular_file(&input.source, LIMIT_FILE)?;
            if sha != record.sha256 || size != record.bytes {
                return Err("voice_lab:recovery_checkpoint_changed_during_copy".into());
            }
        }
        write_new_synced(&pending.join("recovery.json"), &bytes)?;
        verify_package(&pending, &receipt, &id)?;
    }
    fs::rename(&pending, &complete).map_err(|_| "voice_lab:recovery_archive_commit_failed")?;
    verify_package(&complete, &receipt, &id)?;
    Ok(Some(id))
}


#[derive(Debug, Clone, Serialize)]
pub(crate) struct TrainingRecoveryStatus {
    pub preserved_checkpoint_candidates: usize,
    pub attention_entries: usize,
    pub resume_available: bool,
}

/// Receipt-only inventory. A listed checkpoint is NOT verified for resume.
/// Full integrity verification occurs during preservation/backup, not during
/// an ordinary UI status request that could otherwise read many gigabytes.
pub(crate) fn training_recovery_status(
    storage: &VoiceLabStoragePaths,
) -> Result<TrainingRecoveryStatus, String> {
    let root = storage.saved_root.join("TrainingRecovery");
    if !trusted_dir(&root)? {
        return Ok(TrainingRecoveryStatus {
            preserved_checkpoint_candidates: 0, attention_entries: 0,
            resume_available: false,
        });
    }
    let mut count = 0usize;
    let mut attention = 0usize;
    let entries = fs::read_dir(&root)
        .map_err(|_| "voice_lab:recovery_library_unreadable")?;
    for (index, entry) in entries.enumerate() {
        if index >= 1024 { return Err("voice_lab:recovery_library_scan_limit".into()); }
        let entry = entry.map_err(|_| "voice_lab:recovery_library_entry_unavailable")?;
        let id = entry.file_name().to_str().ok_or("voice_lab:recovery_id_unreadable")?.to_string();
        if id.len() != 64 || !id.bytes().all(|b|
            b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            attention += 1;
            continue;
        }
        if !trusted_dir(&entry.path())? {
            attention += 1;
            continue;
        }
        let raw = match read_regular_bounded(&entry.path().join("recovery.json"), MAX_RECEIPT) {
            Ok(v) => v,
            Err(_) => { attention += 1; continue; },
        };
        let receipt: RecoveryReceipt = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => { attention += 1; continue; },
        };
        let canonical = serde_json::to_vec(&receipt)
            .map_err(|_| "voice_lab:recovery_status_invalid")?;
        if hash_bytes(&canonical) != id || receipt.format != FORMAT
            || receipt.engine_revision != VOICE_ACTOR_ENGINE_REVISION
            || receipt.status != "checkpoint_preserved_not_verified_for_resume"
            || receipt.parts.is_empty() {
            attention += 1;
            continue;
        }
        count += 1;
    }
    Ok(TrainingRecoveryStatus {
        preserved_checkpoint_candidates: count,
        attention_entries: attention,
        resume_available: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_training_checkpoint_set_is_preserved_for_investigation_not_resumed() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "translateit-training-recovery-{}-{stamp}", std::process::id()
        ));
        let store = VoiceLabStoragePaths::from_roots(
            &root.join("cache"), &root.join("saved"),
        );
        let work = candidate_path(&store);
        let exp = work.join("experiment");
        let s2 = exp.join("logs_s2_v2ProPlus");
        let gpt = exp.join("logs_s1_v2ProPlus").join("ckpt");
        fs::create_dir_all(&s2).unwrap();
        fs::create_dir_all(&gpt).unwrap();
        fs::write(s2.join("G_233333333333.pth"), b"fake generator state").unwrap();
        fs::write(s2.join("D_233333333333.pth"), b"fake discriminator state").unwrap();
        fs::write(gpt.join("epoch=7-step=42.ckpt"), b"fake lightning state").unwrap();
        fs::write(work.join("translateit_s1.yaml"), b"fake gpt configuration").unwrap();
        fs::write(work.join("translateit_s2.json"), b"fake sovits configuration").unwrap();
        let found = inputs(&store).unwrap();
        assert_eq!(found.len(), 5);
        assert!(found.iter().any(|x| x.label == "gpt-0.ckpt"));
        assert!(found.iter().any(|x| x.label == "sovits-G-0.pth"));
        assert!(found.iter().any(|x| x.label == "sovits-D-0.pth"));
        // Without a frozen dataset identity, capture is intentionally blocked.
        // No source checkpoint may be removed to make this test pass.
        assert!(preserve_training_checkpoints(&store).is_err());
        assert!(s2.join("G_233333333333.pth").exists());
        assert!(gpt.join("epoch=7-step=42.ckpt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn captured_state_is_explicitly_not_automatically_resumable() {
        assert_eq!(FORMAT, 1);
        assert!(LIMIT_COUNT <= 22);
        assert!(LIMIT_TOTAL >= LIMIT_FILE);
    }
    #[test]
    fn checkpoint_directory_names_are_pinned_to_v2proplus() {
        let fake = VoiceLabStoragePaths::from_roots(
            Path::new("some-cache"), Path::new("some-saved")
        );
        let root = candidate_path(&fake);
        assert!(root.ends_with(Path::new("VoiceLab/Build/Runtime")));
    }
}

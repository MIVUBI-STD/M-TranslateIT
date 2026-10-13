//! Shared immutable data primitives; no general-purpose DB or duplicate runtime.
//! Failure leaves pending data in place for explicit recovery.
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn checked_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| "voice_lab:history_directory_create_failed")?;
    let meta = fs::symlink_metadata(path).map_err(|_| "voice_lab:history_directory_unavailable")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("voice_lab:history_directory_untrusted".into());
    }
    Ok(())
}

pub(super) fn read_regular_bounded(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "voice_lab:history_file_unavailable")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() == 0 || meta.len() > cap {
        return Err("voice_lab:history_file_invalid".into());
    }
    let mut file = File::open(path).map_err(|_| "voice_lab:history_file_open_failed")?;
    let mut out = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut out).map_err(|_| "voice_lab:history_file_read_failed")?;
    if out.len() as u64 != meta.len() {
        return Err("voice_lab:history_file_changed".into());
    }
    Ok(out)
}

pub(super) fn hash_regular_file(path: &Path, cap: u64) -> Result<(String, u64), String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "voice_lab:history_source_missing")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() == 0 || meta.len() > cap {
        return Err("voice_lab:history_source_invalid".into());
    }
    let mut file = File::open(path).map_err(|_| "voice_lab:history_source_open_failed")?;
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk).map_err(|_| "voice_lab:history_source_read_failed")?;
        if read == 0 { break; }
        bytes = bytes.checked_add(read as u64).ok_or("voice_lab:history_source_size_overflow")?;
        if bytes > cap { return Err("voice_lab:history_source_too_large".into()); }
        digest.update(&chunk[..read]);
    }
    if bytes != meta.len() {
        return Err("voice_lab:history_source_changed".into());
    }
    Ok((format!("{digest:x}"), bytes))
}

pub(super) fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| "voice_lab:history_stage_create_failed")?;
    file.write_all(bytes).map_err(|_| "voice_lab:history_stage_write_failed")?;
    file.sync_all().map_err(|_| "voice_lab:history_stage_sync_failed")
}

/// Content-addressed, atomic metadata save. A mismatched existing or pending
/// payload is NOT discarded or overwritten. Store parent is caller-owned.
pub(super) fn persist_manifest(parent: &Path, json: &[u8]) -> Result<String, String> {
    if json.is_empty() || json.len() > 512 * 1024 {
        return Err("voice_lab:history_manifest_size_invalid".into());
    }
    checked_directory(parent)?;
    let id = hash_bytes(json);
    let complete = parent.join(format!("{id}.json"));
    let pending = parent.join(format!(".pending-{id}.json"));
    let verify = |path: &Path| -> Result<(), String> {
        let bytes = read_regular_bounded(path, 512 * 1024)?;
        if bytes != json || hash_bytes(&bytes) != id {
            return Err("voice_lab:history_manifest_integrity_failed".into());
        }
        Ok(())
    };
    if fs::symlink_metadata(&complete).is_ok() {
        verify(&complete)?;
        return Ok(id);
    }
    if fs::symlink_metadata(&pending).is_ok() {
        verify(&pending)?;
    } else {
        write_new_synced(&pending, json)?;
        verify(&pending)?;
    }
    fs::rename(&pending, &complete).map_err(|_| "voice_lab:history_manifest_commit_failed")?;
    verify(&complete)?;
    Ok(id)
}

pub(super) fn verify_manifest_by_id(parent: &Path, id: &str) -> Result<Vec<u8>, String> {
    if !valid_id(id) { return Err("voice_lab:history_id_invalid".into()); }
    checked_directory(parent)?;
    let data = read_regular_bounded(&parent.join(format!("{id}.json")), 512 * 1024)?;
    if hash_bytes(&data) != id { return Err("voice_lab:history_manifest_corrupt".into()); }
    Ok(data)
}

pub(super) fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

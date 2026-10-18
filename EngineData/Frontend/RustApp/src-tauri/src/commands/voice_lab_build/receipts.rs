//! My Voice ephemeral build receipts: one run identity and one completed result.
 //! A "ready_for_review" worker JSON alone is NOT a completed training result.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::voice_lab::VoiceLabStoragePaths;

const FORMAT: u32 = 1;
const MAX_JSON: u64 = 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RunIdentity {
    format: u32,
    pub(super) generation: u64,
    pub(super) token: String,
    dataset_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletionReceipt {
    format: u32,
    run_token: String,
    run_generation: u64,
    review_id: String,
}

pub(super) fn identity_file(paths: &VoiceLabStoragePaths) -> std::path::PathBuf {
    paths.cache_root.join("Build").join("run_identity.json")
}

pub(super) fn manual_stop_file(paths: &VoiceLabStoragePaths) -> std::path::PathBuf {
    paths.cache_root.join("Build").join("manual_stop.json")
}

pub(super) fn request_manual_stop(
    paths: &VoiceLabStoragePaths, generation: u64,
) -> Result<(), String> {
    let identity = read_identity(paths).ok_or("my_voice:manual_stop_run_identity_unavailable")?;
    if identity.generation != generation {
        return Err("my_voice:manual_stop_generation_mismatch".into());
    }
    let payload = serde_json::json!({
        "run_token": identity.token,
        "run_generation": identity.generation,
        "kind": "manual_cooperative_stop",
    });
    let data = serde_json::to_vec(&payload)
        .map_err(|_| "my_voice:manual_stop_signal_encode_failed")?;
    let file = manual_stop_file(paths);
    match write_new_synced(&file, &data) {
        Ok(()) => Ok(()),
        Err(_) => {
            // The same run may have requested Stop already. Only exactly
            // matching bytes are accepted, not a stale or malformed file.
            let old = fs::symlink_metadata(&file)
                .map_err(|_| "my_voice:manual_stop_signal_write_failed")?;
            if old.is_file() && !old.file_type().is_symlink() && old.len() <= MAX_JSON
                && fs::read(file).ok().as_deref() == Some(data.as_slice())
            {
                Ok(())
            } else {
                Err("my_voice:manual_stop_signal_conflict".into())
            }
        }
    }
}

pub(super) fn completion_file(paths: &VoiceLabStoragePaths) -> std::path::PathBuf {
    paths.cache_root.join("Build").join("completion.json")
}

fn sha_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_new_synced(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| "my_voice:run_receipt_create_failed")?;
    file.write_all(data).and_then(|_| file.sync_all())
        .map_err(|_| "my_voice:run_receipt_sync_failed".to_string())
}

pub(super) fn create_identity(paths: &VoiceLabStoragePaths, generation: u64) -> Result<RunIdentity, String> {
    if generation == 0 { return Err("my_voice:run_generation_invalid".into()); }
    let dataset = fs::read(paths.build_dataset_dir.join("dataset.json"))
        .map_err(|_| "my_voice:run_dataset_missing")?;
    if dataset.is_empty() || dataset.len() > 64 * 1024 {
        return Err("my_voice:run_dataset_invalid".into());
    }
    let dataset_sha256 = sha_hex(&dataset);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_| "my_voice:run_clock_unavailable")?.as_nanos();
    // Fresh across repeated builds/restarts. A run token prevents stale status
    // misrouting, but is not an authentication or malicious-tamper signature.
    let token = sha_hex(
        format!("{}:{}:{}:{}",
            std::process::id(), generation, nanos, dataset_sha256
        ).as_bytes()
    );
    let identity = RunIdentity { format: FORMAT, generation, token, dataset_sha256 };
    let encoded = serde_json::to_vec(&identity).map_err(|_| "my_voice:run_receipt_encode_failed")?;
    write_new_synced(&identity_file(paths), &encoded)?;
    Ok(identity)
}

pub(super) fn read_identity(paths: &VoiceLabStoragePaths) -> Option<RunIdentity> {
    let file = identity_file(paths);
    let metadata = fs::symlink_metadata(&file).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_JSON { return None; }
    let identity: RunIdentity = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    if identity.format != FORMAT || identity.generation == 0
        || identity.token.len() != 64 || identity.dataset_sha256.len() != 64
        || !identity.token.bytes().all(|b| b.is_ascii_hexdigit())
        || !identity.dataset_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let dataset = fs::read(paths.build_dataset_dir.join("dataset.json")).ok()?;
    if dataset.is_empty() || dataset.len() > 64 * 1024 || sha_hex(&dataset) != identity.dataset_sha256 {
        return None;
    }
    Some(identity)
}

pub(super) fn commit_completion(
    paths: &VoiceLabStoragePaths, identity: &RunIdentity, review_id: &str
) -> Result<(), String> {
    if review_id.len() != 64 || !review_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("my_voice:completion_review_invalid".into());
    }
    let current = read_identity(paths).ok_or("my_voice:completion_identity_missing")?;
    if current.token != identity.token || current.generation != identity.generation {
        return Err("my_voice:completion_run_mismatch".into());
    }
    let completion = CompletionReceipt {
        format: FORMAT,
        run_token: identity.token.clone(),
        run_generation: identity.generation,
        review_id: review_id.to_string(),
    };
    let bytes = serde_json::to_vec(&completion).map_err(|_| "my_voice:completion_encode_failed")?;
    write_new_synced(&completion_file(paths), &bytes)
}

pub(super) fn completion_matches(paths: &VoiceLabStoragePaths, review_id: &str) -> bool {
    let Some(identity) = read_identity(paths) else { return false; };
    let file = completion_file(paths);
    let Ok(meta) = fs::symlink_metadata(&file) else { return false; };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JSON { return false; }
    let Ok(bytes) = fs::read(file) else { return false; };
    let Ok(done) = serde_json::from_slice::<CompletionReceipt>(&bytes) else { return false; };
    done.format == FORMAT && done.run_token == identity.token
        && done.run_generation == identity.generation && done.review_id == review_id
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_receipts_require_exact_hash_shape() {
        assert_ne!(sha_hex(b"sample-A"), sha_hex(b"sample-B"));
        assert_eq!(sha_hex(b"sample-A").len(), 64);
    }
}

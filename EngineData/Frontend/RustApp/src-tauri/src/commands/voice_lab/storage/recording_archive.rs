//! Immutable, content-addressed copies of accepted My Voice recordings.
//! Existing cache Takes remain the active GPT-SoVITS input until a separately
//! validated migration changes the read authority. Nothing in this module
//! deletes accepted audio, existing archive entries, or incomplete staging.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::{inspect_canonical_wav, sha256_digest, VoiceLabStoragePaths};

const ARCHIVE_VERSION: u32 = 1;
const MAX_CANONICAL_BYTES: u64 = 4 * 1024 * 1024 + 4096;
const MAX_NATIVE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 16 * 1024;
static ARCHIVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ArchivedTake {
    schema_version: u32,
    line_id: u32,
    exact_text: String,
    canonical_sha256: String,
    native_rate_sha256: Option<String>,
}

fn sha256_hex(bytes: &[u8]) -> Result<String, String> {
    let digest = sha256_digest(bytes).ok_or("voice_lab:archive_sha256_unavailable")?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(result)
}

fn bounded_regular_bytes(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:archive_source_metadata_failed:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_file() || !(44..=cap).contains(&meta.len()) {
        return Err("voice_lab:archive_source_invalid".to_string());
    }
    let mut file = File::open(path)
        .map_err(|error| format!("voice_lab:archive_source_open_failed:{error}"))?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("voice_lab:archive_source_read_failed:{error}"))?;
    if bytes.len() as u64 != meta.len() {
        return Err("voice_lab:archive_source_changed".to_string());
    }
    Ok(bytes)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut target = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("voice_lab:archive_stage_create_failed:{error}"))?;
    target.write_all(bytes)
        .map_err(|error| format!("voice_lab:archive_stage_write_failed:{error}"))?;
    target.sync_all()
        .map_err(|error| format!("voice_lab:archive_stage_sync_failed:{error}"))
}

fn verified_archive(dir: &Path, expected: &ArchivedTake) -> Result<(), String> {
    let meta = fs::symlink_metadata(dir)
        .map_err(|error| format!("voice_lab:archive_missing:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("voice_lab:archive_directory_invalid".to_string());
    }
    let receipt = bounded_regular_bytes(&dir.join("take.json"), MAX_RECEIPT_BYTES)?;
    let parsed = serde_json::from_slice::<ArchivedTake>(&receipt)
        .map_err(|_| "voice_lab:archive_receipt_invalid")?;
    if &parsed != expected {
        return Err("voice_lab:archive_receipt_mismatch".to_string());
    }
    let canonical = dir.join("canonical.wav");
    inspect_canonical_wav(&canonical)?;
    let canonical_bytes = bounded_regular_bytes(&canonical, MAX_CANONICAL_BYTES)?;
    if sha256_hex(&canonical_bytes)? != expected.canonical_sha256 {
        return Err("voice_lab:archive_canonical_hash_mismatch".to_string());
    }
    let native_path = dir.join("native-rate.wav");
    match expected.native_rate_sha256.as_ref() {
        Some(hash) => {
            let native_bytes = bounded_regular_bytes(&native_path, MAX_NATIVE_BYTES)?;
            if sha256_hex(&native_bytes)? != *hash {
                return Err("voice_lab:archive_native_hash_mismatch".to_string());
            }
        }
        None => {
            if fs::symlink_metadata(native_path).is_ok() {
                return Err("voice_lab:archive_unexpected_native_audio".to_string());
            }
        }
    }
    Ok(())
}

fn checked_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("voice_lab:archive_directory_create_failed:{error}"))?;
    let meta = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:archive_directory_metadata_failed:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("voice_lab:archive_directory_untrusted".to_string());
    }
    Ok(())
}


struct ArchiveInputs {
    receipt: ArchivedTake,
    receipt_bytes: Vec<u8>,
    id: String,
    canonical_bytes: Vec<u8>,
    native_bytes: Option<Vec<u8>>,
}

fn archive_inputs(
    line_id: u32, exact_text: &str, canonical_path: &Path, native_rate_path: Option<&Path>,
) -> Result<ArchiveInputs, String> {
    if line_id == 0 || exact_text.trim().is_empty() {
        return Err("voice_lab:archive_prompt_invalid".to_string());
    }
    inspect_canonical_wav(canonical_path)?;
    let canonical_bytes = bounded_regular_bytes(canonical_path, MAX_CANONICAL_BYTES)?;
    let native_bytes = native_rate_path.map(|path| bounded_regular_bytes(path, MAX_NATIVE_BYTES)).transpose()?;
    let receipt = ArchivedTake {
        schema_version: ARCHIVE_VERSION, line_id,
        exact_text: exact_text.to_string(),
        canonical_sha256: sha256_hex(&canonical_bytes)?,
        native_rate_sha256: native_bytes.as_ref().map(|v| sha256_hex(v)).transpose()?,
    };
    let receipt_bytes = serde_json::to_vec(&receipt)
        .map_err(|_| "voice_lab:archive_receipt_serialize_failed")?;
    let id = sha256_hex(&receipt_bytes)?;
    Ok(ArchiveInputs { receipt, receipt_bytes, id, canonical_bytes, native_bytes })
}

/// Read-only verification, never generates or repairs the archive on a status read.
pub(crate) fn guided_take_is_archived(
    storage: &VoiceLabStoragePaths, line_id: u32, exact_text: &str,
    canonical_path: &Path, native_rate_path: Option<&Path>,
) -> Result<bool, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let prepared = archive_inputs(line_id, exact_text, canonical_path, native_rate_path)?;
    let shard = storage.saved_root.join("Recordings").join(&prepared.id[..2]);
    let complete = shard.join(&prepared.id);
    match fs::symlink_metadata(&complete) {
        Ok(_) => {
            verified_archive(&complete, &prepared.receipt)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let pending = shard.join(format!(".pending-{}", prepared.id));
            match fs::symlink_metadata(&pending) {
                Ok(_) => {
                    verified_archive(&pending, &prepared.receipt)
                        .map_err(|_| "voice_lab:archive_pending_recovery_required")?;
                    Ok(false)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(format!("voice_lab:archive_pending_metadata_failed:{error}")),
            }
        }
        Err(error) => Err(format!("voice_lab:archive_metadata_failed:{error}")),
    }
}

/// Archive a guided take BEFORE permitting the legacy accepted-take replacement.
///
/// Idempotent on exact line/text/audio bytes. A different recording of the
/// same line produces a different archive identity and never replaces a prior
/// entry. An incomplete archive remains for explicit recovery, not deletion.
pub(crate) fn archive_guided_take(
    storage: &VoiceLabStoragePaths,
    line_id: u32,
    exact_text: &str,
    canonical_path: &Path,
    native_rate_path: Option<&Path>,
) -> Result<String, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let ArchiveInputs { receipt, receipt_bytes, id, canonical_bytes, native_bytes } =
        archive_inputs(line_id, exact_text, canonical_path, native_rate_path)?;
    let root = storage.saved_root.join("Recordings");
    checked_dir(&root)?;
    let shard: PathBuf = root.join(&id[..2]);
    checked_dir(&shard)?;
    let complete = shard.join(&id);
    if fs::symlink_metadata(&complete).is_ok() {
        verified_archive(&complete, &receipt)?;
        return Ok(id);
    }
    let pending = shard.join(format!(".pending-{id}"));
    if fs::symlink_metadata(&pending).is_ok() {
        // A fully synced staging directory can be promoted after a crash.
        // An incomplete one must not be overwritten or assumed disposable.
        verified_archive(&pending, &receipt)
            .map_err(|_| "voice_lab:archive_pending_recovery_required".to_string())?;
        fs::rename(&pending, &complete)
            .map_err(|error| format!("voice_lab:archive_recovery_commit_failed:{error}"))?;
        verified_archive(&complete, &receipt)?;
        return Ok(id);
    }
    fs::create_dir(&pending)
        .map_err(|error| format!("voice_lab:archive_pending_create_failed:{error}"))?;
    write_new_synced(&pending.join("canonical.wav"), &canonical_bytes)?;
    if let Some(native) = native_bytes.as_ref() {
        write_new_synced(&pending.join("native-rate.wav"), native)?;
    }
    write_new_synced(&pending.join("take.json"), &receipt_bytes)?;
    verified_archive(&pending, &receipt)?;
    fs::rename(&pending, &complete)
        .map_err(|error| format!("voice_lab:archive_commit_failed:{error}"))?;
    verified_archive(&complete, &receipt)?;
    Ok(id)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn wav(seed: u8) -> Vec<u8> {
        let data = vec![seed; 64_000];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&64_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        wav
    }

    #[test]
    fn immutable_history_keeps_both_retake_versions_and_source_unchanged() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-{}-{unique}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let canonical = storage.takes_dir.join("take_0001.wav");
        let first = wav(22);
        fs::write(&canonical, &first).unwrap();
        let a = archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap();
        assert_eq!(a, archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap());
        fs::write(&canonical, wav(44)).unwrap();
        let b = archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap();
        assert_ne!(a, b);
        assert_eq!(fs::read(storage.saved_root.join("Recordings").join(&a[..2]).join(&a).join("canonical.wav")).unwrap(), first);
        assert_eq!(fs::read(&canonical).unwrap(), wav(44));
        assert!(storage.saved_root.join("Recordings").join(&b[..2]).join(&b).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupted_archive_cannot_be_treated_as_saved() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-corrupt-{}-{unique}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let source = storage.takes_dir.join("take_0002.wav");
        fs::write(&source, wav(33)).unwrap();
        let id = archive_guided_take(&storage, 2, "Thanks for joining today.", &source, None).unwrap();
        let item = storage.saved_root.join("Recordings").join(&id[..2]).join(&id).join("canonical.wav");
        fs::write(item, wav(12)).unwrap();
        assert!(archive_guided_take(&storage, 2, "Thanks for joining today.", &source, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

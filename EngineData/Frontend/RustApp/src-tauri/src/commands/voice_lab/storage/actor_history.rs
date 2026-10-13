//! Immutable, content-addressed GPT-SoVITS actor package history.
//! A version is a *saved inference package*, NOT an optimizer training checkpoint.
//! No deletion, no activation, and no implicit change to the Meeting voice.
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::immutable_store::{
    checked_directory, hash_bytes, hash_regular_file, read_regular_bounded, valid_id,
    write_new_synced,
};
use super::{
    validate_actor_package, VoiceLabStoragePaths, ACTOR_MANIFEST_FILE, GPT_WEIGHT_FILE,
    REFERENCE_WAV_FILE, SOVITS_WEIGHT_FILE, VOICE_ACTOR_ENGINE_REVISION,
};
const HISTORY_SCHEMA: u32 = 1;
const MAX_WEIGHT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_REFERENCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
static VOICE_HISTORY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileFingerprint {
    name: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VersionReceipt {
    schema_version: u32,
    engine_revision: String,
    files: Vec<FileFingerprint>,
}

fn voice_root(storage: &VoiceLabStoragePaths) -> PathBuf {
    storage.saved_root.join("VoiceVersions")
}

fn expected_files() -> [(&'static str, u64); 4] {
    [
        (ACTOR_MANIFEST_FILE, MAX_MANIFEST_BYTES),
        (GPT_WEIGHT_FILE, MAX_WEIGHT_BYTES),
        (SOVITS_WEIGHT_FILE, MAX_WEIGHT_BYTES),
        (REFERENCE_WAV_FILE, MAX_REFERENCE_BYTES),
    ]
}

fn fingerprint(source: &Path) -> Result<VersionReceipt, String> {
    let source_meta = fs::symlink_metadata(source)
        .map_err(|_| "voice_lab:version_source_unavailable")?;
    if source_meta.file_type().is_symlink() || !source_meta.is_dir() {
        return Err("voice_lab:version_source_invalid".into());
    }
    validate_actor_package(source)?;
    let mut files = Vec::new();
    for (name, limit) in expected_files() {
        let (sha256, bytes) = hash_regular_file(&source.join(name), limit)?;
        files.push(FileFingerprint { name: name.into(), sha256, bytes });
    }
    Ok(VersionReceipt {
        schema_version: HISTORY_SCHEMA,
        engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
        files,
    })
}

fn verify_version(dir: &Path, expected: &VersionReceipt, id: &str) -> Result<(), String> {
    let folder_meta = fs::symlink_metadata(dir)
        .map_err(|_| "voice_lab:version_directory_unavailable")?;
    if folder_meta.file_type().is_symlink() || !folder_meta.is_dir() {
        return Err("voice_lab:version_directory_untrusted".into());
    }
    let receipt_bytes = read_regular_bounded(&dir.join("version.json"), MAX_MANIFEST_BYTES)?;
    let stored = serde_json::from_slice::<VersionReceipt>(&receipt_bytes)
        .map_err(|_| "voice_lab:version_receipt_invalid")?;
    let canonical = serde_json::to_vec(&stored).map_err(|_| "voice_lab:version_receipt_invalid")?;
    if hash_bytes(&canonical) != id || stored != *expected
        || stored.engine_revision != VOICE_ACTOR_ENGINE_REVISION
        || stored.schema_version != HISTORY_SCHEMA
    {
        return Err("voice_lab:version_receipt_mismatch".into());
    }
    let expected_names = expected_files();
    if expected.files.len() != expected_names.len() {
        return Err("voice_lab:version_file_list_invalid".into());
    }
    for (file, (name, limit)) in expected.files.iter().zip(expected_names) {
        if file.name != name || !valid_id(&file.sha256) {
            return Err("voice_lab:version_file_identity_invalid".into());
        }
        let (hash, bytes) = hash_regular_file(&dir.join(name), limit)?;
        if hash != file.sha256 || bytes != file.bytes {
            return Err("voice_lab:version_file_integrity_failed".into());
        }
    }
    validate_actor_package(dir)?;
    Ok(())
}

/// This call MUST complete before an actor is replaced or a promotion backup
/// is removed. A failed/corrupt/pending version blocks the replacement.
/// Duplicate packages are verified and reused, never overwritten.
pub(super) fn preserve_actor_version(
    storage: &VoiceLabStoragePaths,
    source: &Path,
) -> Result<String, String> {
    let _lock = VOICE_HISTORY_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:version_lock_unavailable")?;
    let receipt = fingerprint(source)?;
    let receipt_bytes = serde_json::to_vec(&receipt)
        .map_err(|_| "voice_lab:version_receipt_serialize_failed")?;
    let id = hash_bytes(&receipt_bytes);
    let root = voice_root(storage);
    checked_directory(&root)?;
    let shard = root.join(&id[..2]);
    checked_directory(&shard)?;
    let saved = shard.join(&id);
    if fs::symlink_metadata(&saved).is_ok() {
        verify_version(&saved, &receipt, &id)?;
        return Ok(id);
    }
    let pending = shard.join(format!(".pending-{id}"));
    if fs::symlink_metadata(&pending).is_ok() {
        verify_version(&pending, &receipt, &id)
            .map_err(|_| "voice_lab:version_pending_needs_recovery".to_string())?;
    } else {
        fs::create_dir(&pending).map_err(|_| "voice_lab:version_stage_create_failed")?;
        for file in &receipt.files {
            let from = source.join(&file.name);
            let to = pending.join(&file.name);
            let copied = fs::copy(&from, &to)
                .map_err(|_| "voice_lab:version_copy_failed")?;
            if copied != file.bytes {
                return Err("voice_lab:version_copy_size_mismatch".into());
            }
            File::open(&to).and_then(|f| f.sync_all())
                .map_err(|_| "voice_lab:version_sync_failed")?;
        }
        // If source bytes changed while copying, never publish this version.
        if fingerprint(source)? != receipt {
            return Err("voice_lab:version_source_changed".into());
        }
        write_new_synced(&pending.join("version.json"), &receipt_bytes)?;
        verify_version(&pending, &receipt, &id)?;
    }
    fs::rename(&pending, &saved).map_err(|_| "voice_lab:version_commit_failed")?;
    verify_version(&saved, &receipt, &id)?;
    Ok(id)
}

/// Returns only verified packages. Intended for explicit restore/selection
/// integration later; restoring must use the existing Meeting resource guards.
#[allow(dead_code)]
pub(super) fn verify_actor_version(
    storage: &VoiceLabStoragePaths,
    id: &str,
) -> Result<(), String> {
    if !valid_id(id) { return Err("voice_lab:version_id_invalid".into()); }
    let root = voice_root(storage);
    // Neither the library root nor its shard may redirect via symlink.
    let root_meta = fs::symlink_metadata(&root)
        .map_err(|_| "voice_lab:version_library_unavailable")?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return Err("voice_lab:version_library_untrusted".into());
    }
    let shard = root.join(&id[..2]);
    // Do not let symlinked shard bypass the application's saved storage root.
    let meta = fs::symlink_metadata(&shard).map_err(|_| "voice_lab:version_shard_missing")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("voice_lab:version_shard_invalid".into());
    }
    let dir = shard.join(id);
    let bytes = read_regular_bounded(&dir.join("version.json"), MAX_MANIFEST_BYTES)?;
    let receipt = serde_json::from_slice::<VersionReceipt>(&bytes)
        .map_err(|_| "voice_lab:version_receipt_invalid")?;
    verify_version(&dir, &receipt, id)
}


const PAGE_SIZE: usize = 12;
const MAX_VERSION_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ActorVersionItem {
    pub version_id: String,
    pub engine_revision: String,
    // Listing is receipt-only. Full large-weight verification happens
    // immediately before activation, never pretended by the list UI.
    pub verification_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ActorVersionPage {
    pub items: Vec<ActorVersionItem>,
    pub next_cursor: Option<String>,
    pub attention_entries: usize,
}

/// Fast metadata-only listing, sorted by immutable content ID. A pending,
/// invalid or unreadable entry is reported as attention, never as selectable.
pub(crate) fn actor_version_page(
    storage: &VoiceLabStoragePaths,
    after: Option<&str>,
) -> Result<ActorVersionPage, String> {
    let after = after.unwrap_or("");
    if !after.is_empty() && !valid_id(after) {
        return Err("voice_lab:version_cursor_invalid".into());
    }
    let root = voice_root(storage);
    match fs::symlink_metadata(&root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActorVersionPage {
                items: vec![], next_cursor: None, attention_entries: 0,
            });
        }
        Err(_) => return Err("voice_lab:version_library_unavailable".into()),
        Ok(_) => {
            let info = fs::symlink_metadata(&root)
                .map_err(|_| "voice_lab:version_library_unavailable")?;
            if !info.is_dir() || info.file_type().is_symlink() {
                return Err("voice_lab:version_library_untrusted".into());
            }
        }
    }
    let mut ids = Vec::new();
    let mut attention = 0usize;
    let mut count = 0usize;
    for shard in fs::read_dir(&root).map_err(|_| "voice_lab:version_library_unreadable")? {
        let shard = shard.map_err(|_| "voice_lab:version_shard_entry_unreadable")?;
        let label = shard.file_name().to_string_lossy().to_string();
        let info = fs::symlink_metadata(shard.path())
            .map_err(|_| "voice_lab:version_shard_metadata_unavailable")?;
        if label.len() != 2
            || !label.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !info.is_dir() || info.file_type().is_symlink()
        {
            attention += 1;
            continue;
        }
        for item in fs::read_dir(shard.path()).map_err(|_| "voice_lab:version_shard_unreadable")? {
            let item = item.map_err(|_| "voice_lab:version_entry_unreadable")?;
            count += 1;
            if count > MAX_VERSION_ENTRIES {
                return Err("voice_lab:version_library_scan_limit".into());
            }
            let id = item.file_name().to_string_lossy().to_string();
            if !valid_id(&id) || !id.starts_with(&label) {
                attention += 1;
                continue;
            }
            // A symlinked version directory cannot expose external files.
            let info = fs::symlink_metadata(item.path())
                .map_err(|_| "voice_lab:version_entry_metadata_unavailable")?;
            if !info.is_dir() || info.file_type().is_symlink() {
                attention += 1;
                continue;
            }
            let bytes = match read_regular_bounded(&item.path().join("version.json"), MAX_MANIFEST_BYTES) {
                Ok(bytes) => bytes,
                Err(_) => { attention += 1; continue; }
            };
            let data = match serde_json::from_slice::<VersionReceipt>(&bytes) {
                Ok(data) => data,
                Err(_) => { attention += 1; continue; }
            };
            if data.schema_version != HISTORY_SCHEMA
                || data.engine_revision != VOICE_ACTOR_ENGINE_REVISION
                || data.files.len() != expected_files().len()
                || data.files.iter().zip(expected_files()).any(|(actual, (name, _))|
                    actual.name != name || !valid_id(&actual.sha256) || actual.bytes == 0)
                || hash_bytes(&serde_json::to_vec(&data)
                    .map_err(|_| "voice_lab:version_receipt_invalid")?) != id
            {
                attention += 1;
                continue;
            }
            ids.push(id);
        }
    }
    ids.sort();
    let candidates: Vec<String> = ids.into_iter()
        .filter(|id| id.as_str() > after)
        .collect();
    let has_more = candidates.len() > PAGE_SIZE;
    let items: Vec<ActorVersionItem> = candidates.iter().take(PAGE_SIZE)
        .map(|id| ActorVersionItem {
            version_id: id.clone(),
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
            verification_required: true,
        })
        .collect();
    let next_cursor = if has_more { items.last().map(|v| v.version_id.clone()) } else { None };
    Ok(ActorVersionPage { items, next_cursor, attention_entries: attention })
}

/// Copy a specific verified actor into the caller's staging directory. Both
/// original and staging must match the immutable receipt before the caller
/// may promote it to the legacy selected-voice location.
pub(super) fn prepare_version_for_activation(
    storage: &VoiceLabStoragePaths,
    id: &str,
    staging: &Path,
) -> Result<(), String> {
    verify_actor_version(storage, id)?;
    let source = voice_root(storage).join(&id[..2]).join(id);
    for (name, _) in expected_files() {
        fs::copy(source.join(name), staging.join(name))
            .map_err(|_| "voice_lab:version_restore_copy_failed")?;
    }
    let receipt_bytes = read_regular_bounded(&source.join("version.json"), MAX_MANIFEST_BYTES)?;
    let receipt: VersionReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|_| "voice_lab:version_receipt_invalid")?;
    // Validate the copied output against every source hash, not only its shape.
    validate_actor_package(staging)?;
    for (actual, (name, cap)) in receipt.files.iter().zip(expected_files()) {
        let (sha, bytes) = hash_regular_file(&staging.join(name), cap)?;
        if sha != actual.sha256 || bytes != actual.bytes {
            return Err("voice_lab:version_restore_integrity_failed".into());
        }
    }
    verify_actor_version(storage, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_actor(path: &Path, marker: &[u8]) {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join(GPT_WEIGHT_FILE), marker).unwrap();
        fs::write(path.join(SOVITS_WEIGHT_FILE), b"test-sovits").unwrap();
        let samples = vec![0_u8; 4 * 64_000];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&64_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(&samples);
        fs::write(path.join(REFERENCE_WAV_FILE), wav).unwrap();
        fs::write(path.join(ACTOR_MANIFEST_FILE), serde_json::json!({
            "schema_version": 1,
            "engine": "gpt-sovits-v2proplus",
            "engine_revision": VOICE_ACTOR_ENGINE_REVISION,
            "gpt_weight_file": GPT_WEIGHT_FILE,
            "sovits_weight_file": SOVITS_WEIGHT_FILE,
            "reference_wav_file": REFERENCE_WAV_FILE,
            "reference_text": "Good morning everyone.",
            "reference_duration_ms": 4000,
            "held_out_evaluation_complete": true
        }).to_string()).unwrap();
    }


    #[test]
    fn pages_are_metadata_only_but_restore_requires_full_weight_hashes() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("translateit-actor-paging-{}-{stamp}", std::process::id()));
        let paths = VoiceLabStoragePaths::from_roots(&dir.join("cache"), &dir.join("saved"));
        let source = dir.join("actor");
        let mut identities = Vec::new();
        for seed in 1_u8..=14 {
            fake_actor(&source, &[seed, 41]);
            identities.push(preserve_actor_version(&paths, &source).unwrap());
        }
        let page1 = actor_version_page(&paths, None).unwrap();
        assert_eq!(page1.items.len(), PAGE_SIZE);
        assert!(page1.items.iter().all(|entry| entry.verification_required));
        let page2 = actor_version_page(&paths, page1.next_cursor.as_deref()).unwrap();
        assert_eq!(page2.items.len(), 2);
        assert!(page2.next_cursor.is_none());
        let id = &identities[0];
        let staging = dir.join("stage");
        fs::create_dir_all(&staging).unwrap();
        prepare_version_for_activation(&paths, id, &staging).unwrap();
        assert!(validate_actor_package(&staging).is_ok());
        // A metadata-only listing may still show this version, but activation
        // must reject the exact file if its weight bytes were changed.
        fs::write(voice_root(&paths).join(&id[..2]).join(id).join(GPT_WEIGHT_FILE),
            b"tampered").unwrap();
        assert!(verify_actor_version(&paths, id).is_err());
        let stage2 = dir.join("stage2");
        fs::create_dir_all(&stage2).unwrap();
        assert!(prepare_version_for_activation(&paths, id, &stage2).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn different_approved_actors_remain_individually_recoverable() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("translateit-actor-versions-{}-{stamp}", std::process::id()));
        let paths = VoiceLabStoragePaths::from_roots(&dir.join("cache"), &dir.join("saved"));
        let source = dir.join("actor");
        fake_actor(&source, b"first actor");
        let first = preserve_actor_version(&paths, &source).unwrap();
        assert_eq!(first, preserve_actor_version(&paths, &source).unwrap());
        fake_actor(&source, b"second actor");
        let second = preserve_actor_version(&paths, &source).unwrap();
        assert_ne!(first, second);
        verify_actor_version(&paths, &first).unwrap();
        verify_actor_version(&paths, &second).unwrap();
        // A corrupted archived weight is never accepted as a verified version.
        fs::write(voice_root(&paths).join(&first[..2]).join(&first).join(GPT_WEIGHT_FILE),
            b"corrupt weight").unwrap();
        assert!(verify_actor_version(&paths, &first).is_err());
        assert!(verify_actor_version(&paths, &second).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn package_id_is_stable_and_sensitive_to_weight_digest() {
        let sample = |hash: String| VersionReceipt {
            schema_version: HISTORY_SCHEMA,
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
            files: vec![FileFingerprint { name: GPT_WEIGHT_FILE.into(), sha256: hash, bytes: 42 }],
        };
        let first = serde_json::to_vec(&sample("a".repeat(64))).unwrap();
        assert_eq!(hash_bytes(&first), hash_bytes(&first));
        assert_ne!(hash_bytes(&first), hash_bytes(&serde_json::to_vec(&sample("b".repeat(64))).unwrap()));
    }
}

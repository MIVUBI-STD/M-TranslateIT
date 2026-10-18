//! Explicit, verified, folder-format backups of permanent My Voice data.
//! Export never deletes source data. Import is additive and NEVER activates
//! or overwrites the current Meeting voice or an existing archive object.
//! The caller owns permission/resource checks and preservation of any active
//! personal inference actor before exporting the version archive.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::{Mutex, OnceLock};

use super::immutable_store::{checked_directory, hash_regular_file, read_regular_bounded};
use super::VoiceLabStoragePaths;
use crate::commands::voice_lab::{
    GuidedDatasetManifest, VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};

const FORMAT: u32 = 1;
const MANIFEST: &str = "my-voice-backup.json";
const CONTENT: &str = "data";
const MAX_ENTRIES: usize = 50_000;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024 * 1024;
const MANIFEST_LIMIT: u64 = 16 * 1024 * 1024;
const ROOTS: &[&str] = &["Recordings", "DatasetSnapshots", "TrainingLineage", "VoiceVersions", "TrainingRecovery"];
static BACKUP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn exclusive() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    BACKUP_LOCK.get_or_init(|| Mutex::new(())).try_lock()
        .map_err(|_| "voice_backup:another_backup_or_restore_in_progress".into())
}

pub(crate) fn backup_active() -> bool {
    BACKUP_LOCK.get().is_some_and(|lock| lock.try_lock().is_err())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BackupEntry {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    schema_version: u32,
    engine_revision: String,
    entries: Vec<BackupEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupReport {
    pub ok: bool,
    pub message: String,
    pub backup_folder: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub newly_imported: usize,
    pub already_present: usize,
}


fn valid_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_recovery_file(name: &str) -> bool {
    matches!(name, "gpt-config.yaml" | "sovits-config.json")
        || (name.starts_with("gpt-") && name.ends_with(".ckpt")
            && name[4..name.len()-5].parse::<u8>().is_ok())
        || (name.starts_with("sovits-G-") && name.ends_with(".pth")
            && name[9..name.len()-4].parse::<u8>().is_ok())
        || (name.starts_with("sovits-D-") && name.ends_with(".pth")
            && name[9..name.len()-4].parse::<u8>().is_ok())
}

fn safe_relative(path: &str) -> bool {
    if path.is_empty() || path.contains('\\') || path.contains(':') || path.contains('\0') {
        return false;
    }
    let components: Vec<_> = Path::new(path).components().collect();
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != components.len()
        || !components.iter().all(|p| matches!(p, Component::Normal(_)))
        || parts.iter().any(|part| part.is_empty() || *part == "." || *part == ".."
            || part.ends_with('.') || part.ends_with(' '))
    {
        return false;
    }
    match parts.as_slice() {
        ["DatasetSnapshots" | "TrainingLineage", filename] =>
            filename.strip_suffix(".json").is_some_and(valid_hex),
        ["Recordings", shard, id, filename] =>
            valid_hex(id) && shard.to_string() == id[..2].to_string()
                && matches!(filename.to_string().as_str(), "take.json" | "canonical.wav" | "native-rate.wav"),
        ["VoiceVersions", shard, id, filename] =>
            valid_hex(id) && shard.to_string() == id[..2].to_string()
                && matches!(filename.to_string().as_str(), "version.json" | "actor.json" | "gpt.ckpt" | "sovits.pth" | "reference.wav"),
        ["TrainingRecovery", id, "recovery.json"] => valid_hex(id),
        ["TrainingRecovery", id, "files", filename] =>
            valid_hex(id) && valid_recovery_file(filename),
        _ => false,
    }
}

fn safe_directory_relative(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["Recordings" | "VoiceVersions", shard] =>
            shard.len() == 2 && shard.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        ["Recordings" | "VoiceVersions", shard, id] =>
            valid_hex(id) && shard.to_string() == id[..2].to_string(),
        ["TrainingRecovery", id] => valid_hex(id),
        ["TrainingRecovery", id, "files"] => valid_hex(id),
        _ => false,
    }
}

fn trust_dir(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "voice_backup:directory_unavailable")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("voice_backup:directory_untrusted".into());
    }
    Ok(())
}

fn require_existing_directory(path: &str) -> Result<PathBuf, String> {
    if path.len() > 4096 || path.contains('\0') || !Path::new(path).is_absolute() {
        return Err("voice_backup:choose_absolute_existing_folder".into());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| "voice_backup:folder_not_accessible")?;
    trust_dir(&canonical)?;
    Ok(canonical)
}

fn sync_copy(source: &Path, destination: &Path, expected: &BackupEntry) -> Result<(), String> {
    let (sha, size) = hash_regular_file(source, MAX_FILE_BYTES)?;
    if size != expected.bytes || sha != expected.sha256 {
        return Err("voice_backup:source_changed_or_corrupt".into());
    }
    let mut input = File::open(source).map_err(|_| "voice_backup:source_open_failed")?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(destination)
        .map_err(|_| "voice_backup:destination_exists_or_unavailable")?;
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buf).map_err(|_| "voice_backup:source_read_failed")?;
        if read == 0 { break; }
        output.write_all(&buf[..read]).map_err(|_| "voice_backup:copy_failed")?;
    }
    output.sync_all().map_err(|_| "voice_backup:sync_failed")?;
    drop(output); // Close the destination before reopening it for verification.
    let (dest_hash, dest_size) = hash_regular_file(destination, MAX_FILE_BYTES)?;
    if dest_hash != expected.sha256 || dest_size != expected.bytes {
        return Err("voice_backup:copy_verification_failed".into());
    }
    Ok(())
}

fn walk_files(root: &Path, relative_root: &str) -> Result<Vec<BackupEntry>, String> {
    let mut output = Vec::new();
    let mut stack = vec![(root.to_path_buf(), relative_root.to_string())];
    while let Some((dir, rel)) = stack.pop() {
        trust_dir(&dir)?;
        for child in fs::read_dir(&dir).map_err(|_| "voice_backup:read_directory_failed")? {
            let child = child.map_err(|_| "voice_backup:directory_entry_failed")?;
            let name = child.file_name().to_str()
                .ok_or("voice_backup:non_unicode_filename")?.to_string();
            let child_rel = format!("{rel}/{name}");
            if name.starts_with(".pending-") {
                return Err("voice_backup:unrecognized_or_pending_source".into());
            }
            let path = child.path();
            let meta = fs::symlink_metadata(&path)
                .map_err(|_| "voice_backup:source_metadata_unavailable")?;
            if meta.file_type().is_symlink() {
                return Err("voice_backup:symlink_in_source".into());
            }
            if meta.is_dir() {
                if !safe_directory_relative(&child_rel) {
                    return Err("voice_backup:unrecognized_source_directory".into());
                }
                stack.push((path, child_rel));
            } else if meta.is_file() {
                if !safe_relative(&child_rel) {
                    return Err("voice_backup:unrecognized_source_file".into());
                }
                if output.len() >= MAX_ENTRIES {
                    return Err("voice_backup:too_many_files".into());
                }
                let (sha256, bytes) = hash_regular_file(&path, MAX_FILE_BYTES)?;
                output.push(BackupEntry { path: child_rel, bytes, sha256 });
            } else {
                return Err("voice_backup:unsupported_source_entry".into());
            }
        }
    }
    Ok(output)
}

fn source_entries(storage: &VoiceLabStoragePaths) -> Result<Vec<BackupEntry>, String> {
    trust_dir(&storage.saved_root)?;
    let mut entries = Vec::new();
    for &name in ROOTS {
        let dir = storage.saved_root.join(name);
        match fs::symlink_metadata(&dir) {
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err("voice_backup:source_root_unreadable".into()),
            Ok(_) => entries.extend(walk_files(&dir, name)?),
        }
    }
    entries.sort_by(|a,b| a.path.cmp(&b.path));
    if entries.is_empty() {
        return Err("voice_backup:no_saved_recordings_or_versions".into());
    }
    validate_entries(&entries)?;
    Ok(entries)
}

fn validate_entries(entries: &[BackupEntry]) -> Result<u64, String> {
    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err("voice_backup:entry_count_invalid".into());
    }
    let mut prior = "";
    let mut total = 0_u64;
    for item in entries {
        if !safe_relative(&item.path) || !valid_hex(&item.sha256)
            || item.bytes == 0 || item.bytes > MAX_FILE_BYTES
            || item.path.as_str() <= prior
        {
            return Err("voice_backup:manifest_entry_invalid_or_duplicate".into());
        }
        prior = &item.path;
        total = total.checked_add(item.bytes)
            .ok_or("voice_backup:size_overflow")?;
        if total > MAX_TOTAL_BYTES {
            return Err("voice_backup:too_large".into());
        }
    }
    Ok(total)
}

fn manifest_bytes(entries: Vec<BackupEntry>) -> Result<Vec<u8>, String> {
    let data = BackupManifest {
        schema_version: FORMAT,
        engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
        entries,
    };
    let bytes = serde_json::to_vec(&data).map_err(|_| "voice_backup:manifest_serialize_failed")?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err("voice_backup:manifest_too_large".into());
    }
    Ok(bytes)
}


/// Validate relationships between files, not merely their independent hashes.
/// Incomplete snapshots or model packages are not advertised as restorable.
fn check_references(root: &Path, entries: &[BackupEntry]) -> Result<(), String> {
    let map: std::collections::HashMap<&str, &BackupEntry> =
        entries.iter().map(|item| (item.path.as_str(), item)).collect();
    // Only payloads referenced by their immutable receipts may be exported
    // or accepted from a backup. A hash-consistent orphan is not a restorable
    // recording, actor or training checkpoint.
    let mut referenced_payloads = std::collections::HashSet::<String>::new();
    for entry in entries {
        let rel = entry.path.as_str();
        if rel.ends_with("/take.json") {
            let id = rel.split('/').nth(2).ok_or("voice_backup:take_id_missing")?;
            if entry.sha256 != id {
                return Err("voice_backup:take_receipt_identity_invalid".into());
            }
            let data = read_regular_bounded(&root.join(rel), 64 * 1024)?;
            let json: serde_json::Value = serde_json::from_slice(&data)
                .map_err(|_| "voice_backup:take_receipt_invalid")?;
            let canonical = json.get("canonical_sha256").and_then(|v| v.as_str())
                .ok_or("voice_backup:take_reference_invalid")?;
            let prefix = rel.strip_suffix("take.json").ok_or("voice_backup:take_reference_invalid")?;
            let canonical_path = format!("{prefix}canonical.wav");
            let file = map.get(canonical_path.as_str())
                .ok_or("voice_backup:take_canonical_missing")?;
            if canonical != file.sha256 {
                return Err("voice_backup:take_canonical_mismatch".into());
            }
            referenced_payloads.insert(canonical_path);
            if let Some(native) = json.get("native_rate_sha256").and_then(|v| v.as_str()) {
                let native_path = format!("{prefix}native-rate.wav");
                let sidecar = map.get(native_path.as_str())
                    .ok_or("voice_backup:take_native_missing")?;
                if native != sidecar.sha256 {
                    return Err("voice_backup:take_native_mismatch".into());
                }
                referenced_payloads.insert(native_path);
            }
        }
        if rel.ends_with("/version.json") && rel.starts_with("VoiceVersions/") {
            let id = rel.split('/').nth(2).ok_or("voice_backup:version_id_missing")?;
            if entry.sha256 != id {
                return Err("voice_backup:version_receipt_identity_invalid".into());
            }
            let data = read_regular_bounded(&root.join(rel), 64 * 1024)?;
            let json: serde_json::Value = serde_json::from_slice(&data)
                .map_err(|_| "voice_backup:version_receipt_invalid")?;
            if json.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
                || json.get("engine_revision").and_then(|v| v.as_str())
                    != Some(VOICE_ACTOR_ENGINE_REVISION)
            {
                return Err("voice_backup:version_receipt_unsupported".into());
            }
            let files = json.get("files").and_then(|v| v.as_array())
                .ok_or("voice_backup:version_files_missing")?;
            if files.len() != 4 { return Err("voice_backup:version_file_count_invalid".into()); }
            let prefix = rel.strip_suffix("version.json")
                .ok_or("voice_backup:version_reference_invalid")?;
            for name in ["actor.json", "gpt.ckpt", "sovits.pth", "reference.wav"] {
                let referenced = files.iter().find(|v| v.get("name").and_then(|f| f.as_str()) == Some(name))
                    .ok_or("voice_backup:version_file_missing")?;
                let expected = referenced.get("sha256").and_then(|f| f.as_str())
                    .ok_or("voice_backup:version_hash_missing")?;
                let expected_bytes = referenced.get("bytes").and_then(|f| f.as_u64())
                    .ok_or("voice_backup:version_size_missing")?;
                let file_path = format!("{prefix}{name}");
                let actual = map.get(file_path.as_str())
                    .ok_or("voice_backup:version_object_missing")?;
                if actual.sha256 != expected || actual.bytes != expected_bytes {
                    return Err("voice_backup:version_hash_or_size_mismatch".into());
                }
                referenced_payloads.insert(file_path);
            }
            // Verify this actor is actually usable by the same native owner
            // that will validate it on a future explicit version selection.
            super::validate_actor_package(&root.join(prefix))
                .map_err(|_| "voice_backup:actor_package_invalid")?;
        }
        if rel.starts_with("DatasetSnapshots/") && rel.ends_with(".json") {
            // Immutable dataset snapshots are addressed by the SHA-256 of
            // their exact manifest bytes. A self-consistent backup index is
            // not sufficient if the original immutable ID was forged.
            let id = rel.strip_prefix("DatasetSnapshots/")
                .and_then(|path| path.strip_suffix(".json"))
                .ok_or("voice_backup:dataset_snapshot_id_invalid")?;
            if !valid_hex(id) || entry.sha256 != id {
                return Err("voice_backup:dataset_snapshot_identity_invalid".into());
            }
            let data = read_regular_bounded(&root.join(rel), 512 * 1024)?;
            let json: serde_json::Value = serde_json::from_slice(&data)
                .map_err(|_| "voice_backup:dataset_snapshot_invalid")?;
            // Backup linkage must agree with the same frozen manifest and
            // receipt contract used by dataset_history::verify_snapshot.
            let source: GuidedDatasetManifest = serde_json::from_value(
                json.get("source_manifest").cloned()
                    .ok_or("voice_backup:dataset_manifest_missing")?
            ).map_err(|_| "voice_backup:dataset_manifest_invalid")?;
            if json.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
                || json.get("engine_revision").and_then(|v| v.as_str())
                    != Some(VOICE_ACTOR_ENGINE_REVISION)
                || source.schema_version != VOICE_LAB_SCHEMA_VERSION
                || source.takes.len() > 128
                || super::validate_guided_dataset_manifest(&source).is_err()
            {
                return Err("voice_backup:dataset_contract_invalid".into());
            }
            let takes = json.get("audio").and_then(|v| v.as_array())
                .ok_or("voice_backup:dataset_audio_links_missing")?;
            if takes.len() != source.takes.len() {
                return Err("voice_backup:dataset_audio_count_mismatch".into());
            }
            for (take, expected) in takes.iter().zip(&source.takes) {
                if take.get("line_id").and_then(|v| v.as_u64()) != Some(u64::from(expected.line_id))
                    || take.get("wav_file").and_then(|v| v.as_str()) != Some(expected.wav_file.as_str())
                {
                    return Err("voice_backup:dataset_take_binding_mismatch".into());
                }
                let id = take.get("archive_id").and_then(|v| v.as_str())
                    .ok_or("voice_backup:dataset_take_id_missing")?;
                if !valid_hex(id) { return Err("voice_backup:dataset_take_id_invalid".into()); }
                let wanted = format!("Recordings/{}/{id}/take.json", &id[..2]);
                if !map.contains_key(wanted.as_str()) {
                    return Err("voice_backup:dataset_source_recording_missing".into());
                }
                let saved = read_regular_bounded(&root.join(&wanted), 16 * 1024)?;
                let receipt: serde_json::Value = serde_json::from_slice(&saved)
                    .map_err(|_| "voice_backup:dataset_take_receipt_invalid")?;
                if receipt.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
                    || receipt.get("line_id").and_then(|v| v.as_u64()) != Some(u64::from(expected.line_id))
                    || receipt.get("exact_text").and_then(|v| v.as_str()) != Some(expected.exact_text.as_str())
                {
                    return Err("voice_backup:dataset_archive_prompt_mismatch".into());
                }
            }
        }
        if rel.starts_with("TrainingRecovery/") && rel.ends_with("/recovery.json") {
            let data = read_regular_bounded(&root.join(rel), 16 * 1024)?;
            let json: serde_json::Value = serde_json::from_slice(&data)
                .map_err(|_| "voice_backup:recovery_receipt_invalid")?;
            let pieces: Vec<&str> = rel.split('/').collect();
            let id = pieces.get(1).copied()
                .ok_or("voice_backup:recovery_id_missing")?;
            if !valid_hex(id) || entry.sha256 != id {
                return Err("voice_backup:recovery_receipt_identity_invalid".into());
            }
            if json.get("format").and_then(|v| v.as_u64()) != Some(1)
                || json.get("engine_revision").and_then(|v| v.as_str()) != Some(VOICE_ACTOR_ENGINE_REVISION)
                || json.get("status").and_then(|v| v.as_str())
                    != Some("checkpoint_preserved_not_verified_for_resume")
            {
                return Err("voice_backup:recovery_receipt_unsupported".into());
            }
            let snapshot = json.get("dataset_snapshot_id").and_then(|v| v.as_str())
                .ok_or("voice_backup:recovery_dataset_id_missing")?;
            if !valid_hex(snapshot)
                || !map.contains_key(format!("DatasetSnapshots/{snapshot}.json").as_str())
            {
                return Err("voice_backup:recovery_dataset_missing".into());
            }
            let files = json.get("parts").and_then(|v| v.as_array())
                .ok_or("voice_backup:recovery_parts_missing")?;
            if files.is_empty() || files.len() > 22 {
                return Err("voice_backup:recovery_parts_invalid".into());
            }
            let mut seen = std::collections::HashSet::new();
            for part in files {
                let label = part.get("label").and_then(|v| v.as_str())
                    .ok_or("voice_backup:recovery_file_name_missing")?;
                let sha = part.get("sha256").and_then(|v| v.as_str())
                    .ok_or("voice_backup:recovery_file_sha_missing")?;
                let bytes = part.get("bytes").and_then(|v| v.as_u64())
                    .ok_or("voice_backup:recovery_file_size_missing")?;
                if !valid_recovery_file(label) || !seen.insert(label) || !valid_hex(sha) {
                    return Err("voice_backup:recovery_file_invalid".into());
                }
                let file_path = format!("TrainingRecovery/{id}/files/{label}");
                let file = map.get(file_path.as_str())
                    .ok_or("voice_backup:recovery_file_missing")?;
                if file.sha256 != sha || file.bytes != bytes {
                    return Err("voice_backup:recovery_file_changed".into());
                }
                referenced_payloads.insert(file_path);
            }
        }
        if rel.starts_with("TrainingLineage/") && rel.ends_with(".json") {
            let id = rel.strip_prefix("TrainingLineage/")
                .and_then(|path| path.strip_suffix(".json"))
                .ok_or("voice_backup:training_lineage_id_invalid")?;
            if !valid_hex(id) || entry.sha256 != id {
                return Err("voice_backup:training_lineage_identity_invalid".into());
            }
            let data = read_regular_bounded(&root.join(rel), 512 * 1024)?;
            let json: serde_json::Value = serde_json::from_slice(&data)
                .map_err(|_| "voice_backup:training_lineage_invalid")?;
            if json.get("schema_version").and_then(|v| v.as_u64()) != Some(1)
                || json.get("engine_revision").and_then(|v| v.as_str())
                    != Some(VOICE_ACTOR_ENGINE_REVISION)
            {
                return Err("voice_backup:training_lineage_contract_invalid".into());
            }
            let dataset = json.get("dataset_snapshot_id").and_then(|v| v.as_str())
                .ok_or("voice_backup:lineage_dataset_missing")?;
            let voice = json.get("actor_version_id").and_then(|v| v.as_str())
                .ok_or("voice_backup:lineage_actor_missing")?;
            if !valid_hex(dataset) || !valid_hex(voice)
                || !map.contains_key(&format!("DatasetSnapshots/{dataset}.json")[..])
                || !map.contains_key(&format!("VoiceVersions/{}/{voice}/version.json", &voice[..2])[..])
            {
                return Err("voice_backup:lineage_references_missing".into());
            }
        }
    }
    for entry in entries {
        let parts: Vec<&str> = entry.path.split('/').collect();
        let is_payload = matches!(
            parts.as_slice(),
            ["Recordings", _, _, "canonical.wav" | "native-rate.wav"]
                | ["VoiceVersions", _, _, "actor.json" | "gpt.ckpt" | "sovits.pth" | "reference.wav"]
                | ["TrainingRecovery", _, "files", _]
        );
        if is_payload && !referenced_payloads.contains(&entry.path) {
            return Err("voice_backup:unreferenced_archive_payload".into());
        }
    }
    Ok(())
}

fn read_manifest(backup: &Path) -> Result<(BackupManifest, u64), String> {
    trust_dir(backup)?;
    let bytes = read_regular_bounded(&backup.join(MANIFEST), MANIFEST_LIMIT)?;
    let manifest: BackupManifest = serde_json::from_slice(&bytes)
        .map_err(|_| "voice_backup:manifest_invalid")?;
    if manifest.schema_version != FORMAT || manifest.engine_revision != VOICE_ACTOR_ENGINE_REVISION {
        return Err("voice_backup:unsupported_engine_or_format".into());
    }
    let total = validate_entries(&manifest.entries)?;
    let data = backup.join(CONTENT);
    trust_dir(&data)?;
    for entry in &manifest.entries {
        let path = data.join(&entry.path);
        // Validate the complete existing path chain; symlinked intermediate
        // directories and leaf files must not escape the backup root.
        let mut cursor = data.clone();
        let parts: Vec<&str> = entry.path.split('/').collect();
        for part in &parts[..parts.len()-1] {
            cursor.push(part);
            trust_dir(&cursor)?;
        }
        let (sha, size) = hash_regular_file(&path, MAX_FILE_BYTES)?;
        if sha != entry.sha256 || size != entry.bytes {
            return Err("voice_backup:backup_file_integrity_failed".into());
        }
    }
    check_references(&data, &manifest.entries)?;
    Ok((manifest, total))
}

/// A user chooses an *existing* destination folder. The app creates its own
/// unique child backup folder, never overwrites or deletes previous backups.
pub(crate) fn export_backup(
    storage: &VoiceLabStoragePaths, destination_dir: &str,
) -> Result<BackupReport, String> {
    let _operation = exclusive()?;
    let destination = require_existing_directory(destination_dir)?;
    let saved = fs::canonicalize(&storage.saved_root)
        .map_err(|_| "voice_backup:source_unavailable")?;
    if destination.starts_with(&saved) {
        return Err("voice_backup:destination_inside_source".into());
    }
    let entries = source_entries(storage)?;
    let total = validate_entries(&entries)?;
    let manifest = manifest_bytes(entries.clone())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_| "voice_backup:clock_unavailable")?.as_nanos();
    let name = format!("TranslateIT-MyVoice-{stamp}");
    let pending = destination.join(format!(".{name}.pending"));
    let committed = destination.join(&name);
    fs::create_dir(&pending).map_err(|_| "voice_backup:cannot_create_new_backup")?;
    let tree = pending.join(CONTENT);
    checked_directory(&tree)?;
    for entry in &entries {
        let from = storage.saved_root.join(&entry.path);
        let to = tree.join(&entry.path);
        checked_directory(to.parent().ok_or("voice_backup:parent_missing")?)?;
        sync_copy(&from, &to, entry)?;
    }
    let index = pending.join(MANIFEST);
    let mut file = OpenOptions::new().write(true).create_new(true).open(&index)
        .map_err(|_| "voice_backup:manifest_create_failed")?;
    file.write_all(&manifest).map_err(|_| "voice_backup:manifest_write_failed")?;
    file.sync_all().map_err(|_| "voice_backup:manifest_sync_failed")?;
    // Close the manifest before renaming its containing directory on Windows.
    drop(file);
    let _ = read_manifest(&pending)?;
    fs::rename(&pending, &committed).map_err(|_| "voice_backup:finalize_failed")?;
    let _ = read_manifest(&committed)?;
    Ok(BackupReport {
        ok: true,
        message: "Backup files were copied and verified. Keep a copy on a separate drive.".into(),
        backup_folder: Some(committed.to_string_lossy().to_string()),
        file_count: entries.len(), total_bytes: total,
        newly_imported: 0, already_present: 0,
    })
}

/// Full read-only integrity check. Does not import or select any voice.
pub(crate) fn verify_backup(source_dir: &str) -> Result<BackupReport, String> {
    let _operation = exclusive()?;
    let backup = require_existing_directory(source_dir)?;
    let (manifest, total) = read_manifest(&backup)?;
    Ok(BackupReport {
        ok: true, message: "Backup content matches its file manifest; it has not been restored.".into(),
        backup_folder: Some(backup.to_string_lossy().to_string()),
        file_count: manifest.entries.len(), total_bytes: total,
        newly_imported: 0, already_present: 0,
    })
}

/// A restore never copies to the mutable selected MyVoice/PersonalVoice paths.
/// It merges only immutable archive files: existing identical content is reused,
/// any conflicting item blocks the entire import BEFORE the first write.
pub(crate) fn restore_backup(
    storage: &VoiceLabStoragePaths, source_dir: &str,
) -> Result<BackupReport, String> {
    let _operation = exclusive()?;
    let backup = require_existing_directory(source_dir)?;
    checked_directory(&storage.saved_root)?;
    let saved = fs::canonicalize(&storage.saved_root)
        .map_err(|_| "voice_backup:destination_unavailable")?;
    if backup.starts_with(&saved) || saved.starts_with(&backup) {
        return Err("voice_backup:source_overlaps_user_data".into());
    }
    let (manifest, total) = read_manifest(&backup)?;
    let mut existing = 0usize;
    for entry in &manifest.entries {
        let mut ancestor = saved.clone();
        for part in entry.path.split('/').take(entry.path.split('/').count() - 1) {
            ancestor.push(part);
            if fs::symlink_metadata(&ancestor).is_ok() {
                trust_dir(&ancestor)?;
            }
        }
        let target = saved.join(&entry.path);
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                let (sha, bytes) = hash_regular_file(&target, MAX_FILE_BYTES)?;
                if sha != entry.sha256 || bytes != entry.bytes {
                    return Err("voice_backup:existing_archive_conflict".into());
                }
                existing += 1;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err("voice_backup:destination_metadata_unknown".into()),
        }
    }
    let mut imported = 0usize;
    for (index, entry) in manifest.entries.iter().enumerate() {
        let target = saved.join(&entry.path);
        if fs::symlink_metadata(&target).is_ok() { continue; }
        // The distinct, per-file staging name remains visible on interruption.
        // Never delete or replace a prior incomplete import automatically.
        let parent = target.parent().ok_or("voice_backup:parent_missing")?;
        checked_directory(parent)?;
        let mut ancestor = saved.clone();
        for part in entry.path.split('/').take(entry.path.split('/').count() - 1) {
            ancestor.push(part);
            trust_dir(&ancestor)?;
        }
        let pending = target.with_extension(format!("restore-pending-{index}"));
        let source = backup.join(CONTENT).join(&entry.path);
        match fs::symlink_metadata(&pending) {
            Ok(_) => {
                let (sha, bytes) = hash_regular_file(&pending, MAX_FILE_BYTES)?;
                if sha != entry.sha256 || bytes != entry.bytes {
                    return Err("voice_backup:incomplete_restore_pending_file".into());
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                sync_copy(&source, &pending, entry)?;
            }
            Err(_) => return Err("voice_backup:restore_pending_metadata_unknown".into()),
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err("voice_backup:archive_changed_during_restore".into());
        }
        fs::rename(&pending, &target)
            .map_err(|_| "voice_backup:archive_restore_commit_failed")?;
        let (sha, bytes) = hash_regular_file(&target, MAX_FILE_BYTES)?;
        if sha != entry.sha256 || bytes != entry.bytes {
            return Err("voice_backup:archive_restore_verify_failed".into());
        }
        imported += 1;
    }
    Ok(BackupReport {
        ok: true,
        message: "Saved recordings and voice versions were imported. The current Meeting voice was not changed.".into(),
        backup_folder: Some(backup.to_string_lossy().to_string()),
        file_count: manifest.entries.len(), total_bytes: total,
        newly_imported: imported, already_present: existing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(path: &str) -> BackupEntry {
        BackupEntry { path: path.into(), bytes: 12, sha256: "a".repeat(64) }
    }
    #[test]
    fn backup_paths_only_allow_known_archive_objects() {
        let digest = "a".repeat(64);
        assert!(safe_relative(&format!("Recordings/aa/{digest}/canonical.wav")));
        assert!(safe_relative(&format!("VoiceVersions/aa/{digest}/gpt.ckpt")));
        assert!(safe_relative(&format!("DatasetSnapshots/{digest}.json")));
        assert!(safe_relative(&format!("TrainingRecovery/{digest}/recovery.json")));
        assert!(safe_relative(&format!("TrainingRecovery/{digest}/files/sovits-G-0.pth")));
        for bad in ["../MyVoice/actor.json", "Recordings/../voice.wav",
            "Recordings\\fake.wav", "/Recordings/evil", "C:/Recordings/file.wav",
            "Unknown/file.wav", "Recordings//file.wav", "Recordings/./file.wav",
            "Recordings/ab/other/canonical.wav", "MyVoice/actor.json"] {
            assert!(!safe_relative(bad), "{bad}");
        }
    }

    #[test]
    fn orphan_archive_payloads_are_not_valid_backups() {
        let id = "a".repeat(64);
        for path in [
            format!("Recordings/aa/{id}/canonical.wav"),
            format!("Recordings/aa/{id}/native-rate.wav"),
            format!("VoiceVersions/aa/{id}/gpt.ckpt"),
            format!("TrainingRecovery/{id}/files/gpt-0.ckpt"),
        ] {
            let failure = check_references(Path::new("."), &[item(&path)]).unwrap_err();
            assert_eq!(failure, "voice_backup:unreferenced_archive_payload", "{path}");
        }
    }

    #[test]
    fn immutable_archive_receipt_and_manifest_ids_must_match_file_digest() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "translateit-backup-identity-{}-{stamp}", std::process::id()
        ));
        let id = "a".repeat(64);
        for (path, error) in [
            (format!("Recordings/aa/{id}/take.json"),
                "voice_backup:take_receipt_identity_invalid"),
            (format!("VoiceVersions/aa/{id}/version.json"),
                "voice_backup:version_receipt_identity_invalid"),
            (format!("DatasetSnapshots/{id}.json"),
                "voice_backup:dataset_snapshot_identity_invalid"),
            (format!("TrainingLineage/{id}.json"),
                "voice_backup:training_lineage_identity_invalid"),
        ] {
            let file = root.join(&path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, b"{}").unwrap();
            let (sha256, bytes) = hash_regular_file(&file, MAX_FILE_BYTES).unwrap();
            assert_ne!(sha256, id);
            let entry = BackupEntry { path, bytes, sha256 };
            assert_eq!(check_references(&root, &[entry]).unwrap_err(), error);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_rejects_version_receipts_with_unsupported_model_contracts() {
        use super::super::immutable_store::hash_bytes;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "translateit-backup-actor-schema-{}-{stamp}", std::process::id()
        ));
        for (schema, revision) in [
            (2_u64, VOICE_ACTOR_ENGINE_REVISION),
            (1_u64, "unsupported-engine"),
        ] {
            let files = ["actor.json", "gpt.ckpt", "sovits.pth", "reference.wav"]
                .into_iter().map(|name| serde_json::json!({
                    "name": name,
                    "sha256": "a".repeat(64),
                    "bytes": 5
                })).collect::<Vec<_>>();
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schema_version": schema,
                "engine_revision": revision,
                "files": files,
            })).unwrap();
            let id = hash_bytes(&bytes);
            let relative = format!("VoiceVersions/{}/{id}/version.json", &id[..2]);
            let file = root.join(&relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, &bytes).unwrap();
            let (sha256, size) = hash_regular_file(&file, MAX_FILE_BYTES).unwrap();
            let entry = BackupEntry { path: relative, bytes: size, sha256 };
            assert_eq!(
                check_references(&root, &[entry]).unwrap_err(),
                "voice_backup:version_receipt_unsupported"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_rejects_snapshot_prompt_mismatch_even_with_valid_archive_hashes() {
        use super::super::immutable_store::hash_bytes;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "translateit-backup-snapshot-binding-{}-{stamp}", std::process::id()
        ));
        let put = |relative: &str, data: &[u8]| -> BackupEntry {
            let file = root.join(relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, data).unwrap();
            let (sha256, bytes) = hash_regular_file(&file, MAX_FILE_BYTES).unwrap();
            BackupEntry { path: relative.into(), bytes, sha256 }
        };
        let canonical = b"canonical-wav-fixture";
        let canonical_sha = hash_bytes(canonical);
        let receipt = format!(
            "{{\"schema_version\":1,\"line_id\":1,\"exact_text\":\"Hello world.\",\"canonical_sha256\":\"{canonical_sha}\",\"native_rate_sha256\":null}}"
        );
        let take_id = hash_bytes(receipt.as_bytes());
        let prefix = format!("Recordings/{}/{take_id}/", &take_id[..2]);
        let canonical_entry = put(&format!("{prefix}canonical.wav"), canonical);
        let receipt_entry = put(&format!("{prefix}take.json"), receipt.as_bytes());
        let snapshot = |text: &str, line: u32| serde_json::json!({
            "schema_version": 1,
            "engine_revision": VOICE_ACTOR_ENGINE_REVISION,
            "source_manifest": {
                "schema_version": VOICE_LAB_SCHEMA_VERSION,
                "authorized_voice_confirmed": true,
                "takes": [{ "line_id": 1, "exact_text": text, "wav_file": "take_0001.wav" }],
                "held_out_lines": [{ "line_id": 1001, "exact_text": "Different review sentence." }]
            },
            "audio": [{ "line_id": line, "wav_file": "take_0001.wav", "archive_id": take_id }],
            "from_saved_archive": false
        });
        let good = serde_json::to_vec(&snapshot("Hello world.", 1)).unwrap();
        let good_entry = put(&format!("DatasetSnapshots/{}.json", hash_bytes(&good)), &good);
        assert!(check_references(&root, &[
            canonical_entry.clone(), receipt_entry.clone(), good_entry,
        ]).is_ok());
        let bad_text = serde_json::to_vec(&snapshot("Different prompt.", 1)).unwrap();
        let bad_text_entry = put(&format!("DatasetSnapshots/{}.json", hash_bytes(&bad_text)), &bad_text);
        assert_eq!(check_references(&root, &[
            canonical_entry.clone(), receipt_entry.clone(), bad_text_entry,
        ]).unwrap_err(), "voice_backup:dataset_archive_prompt_mismatch");
        let bad_line = serde_json::to_vec(&snapshot("Hello world.", 2)).unwrap();
        let bad_line_entry = put(&format!("DatasetSnapshots/{}.json", hash_bytes(&bad_line)), &bad_line);
        assert_eq!(check_references(&root, &[
            canonical_entry, receipt_entry, bad_line_entry,
        ]).unwrap_err(), "voice_backup:dataset_take_binding_mismatch");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_rejects_unsupported_training_lineage_even_with_valid_content_id() {
        use super::super::immutable_store::hash_bytes;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "translateit-backup-lineage-contract-{}-{stamp}", std::process::id()
        ));
        for (schema, revision) in [
            (9_u64, VOICE_ACTOR_ENGINE_REVISION),
            (1_u64, "unsupported-engine"),
        ] {
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schema_version": schema,
                "engine_revision": revision,
                "dataset_snapshot_id": "a".repeat(64),
                "actor_version_id": "b".repeat(64),
            })).unwrap();
            let id = hash_bytes(&bytes);
            let relative = format!("TrainingLineage/{id}.json");
            let path = root.join(&relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &bytes).unwrap();
            let (sha256, size) = hash_regular_file(&path, MAX_FILE_BYTES).unwrap();
            let entry = BackupEntry { path: relative, bytes: size, sha256 };
            assert_eq!(
                check_references(&root, &[entry]).unwrap_err(),
                "voice_backup:training_lineage_contract_invalid"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn complete_backup_restores_archives_without_touching_selected_meeting_voice() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-backup-{}-{stamp}", std::process::id()));
        let source = VoiceLabStoragePaths::from_roots(&root.join("source-cache"), &root.join("source-saved"));
        let target = VoiceLabStoragePaths::from_roots(&root.join("target-cache"), &root.join("target-saved"));
        let exports = root.join("external-drive");
        fs::create_dir_all(&source.takes_dir).unwrap();
        fs::create_dir_all(&exports).unwrap();
        let wav = source.takes_dir.join("take_0001.wav");
        let payload = vec![24_u8; 4 * 64_000];
        let mut audio = Vec::new();
        audio.extend_from_slice(b"RIFF");
        audio.extend_from_slice(&(36 + payload.len() as u32).to_le_bytes());
        audio.extend_from_slice(b"WAVEfmt ");
        audio.extend_from_slice(&16_u32.to_le_bytes());
        audio.extend_from_slice(&1_u16.to_le_bytes());
        audio.extend_from_slice(&1_u16.to_le_bytes());
        audio.extend_from_slice(&32_000_u32.to_le_bytes());
        audio.extend_from_slice(&64_000_u32.to_le_bytes());
        audio.extend_from_slice(&2_u16.to_le_bytes());
        audio.extend_from_slice(&16_u16.to_le_bytes());
        audio.extend_from_slice(b"data");
        audio.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        audio.extend_from_slice(&payload);
        fs::write(&wav, audio).unwrap();
        let id = super::super::recording_archive::archive_guided_take(
            &source, 1, "Good morning, everyone.", &wav, None,
        ).unwrap();
        let exported = export_backup(&source, exports.to_str().unwrap()).unwrap();
        let backup_path = exported.backup_folder.as_ref().unwrap();
        assert!(verify_backup(backup_path).unwrap().ok);
        let imported = restore_backup(&target, backup_path).unwrap();
        assert_eq!(imported.newly_imported, 2);
        assert_eq!(imported.already_present, 0);
        assert!(!target.approved_actor_dir.exists());
        let imported_audio = super::super::recording_archive::saved_recording_audio(&target, &id).unwrap();
        assert!(!imported_audio.is_empty());
        let twice = restore_backup(&target, backup_path).unwrap();
        assert_eq!(twice.newly_imported, 0);
        assert_eq!(twice.already_present, 2);
        assert!(wav.exists());
        // Tampered backup cannot be imported or called verified.
        let original = Path::new(backup_path).join(CONTENT)
            .join(format!("Recordings/{}/{id}/canonical.wav", &id[..2]));
        fs::write(original, b"tampered").unwrap();
        assert!(verify_backup(backup_path).is_err());
        assert!(restore_backup(&target, backup_path).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manifest_entries_sorted_unique_and_hash_bound() {
        let digest = "a".repeat(64);
        let a = format!("DatasetSnapshots/{digest}.json");
        let b = format!("VoiceVersions/aa/{digest}/actor.json");
        assert!(validate_entries(&[item(&a), item(&b)]).is_ok());
        assert!(validate_entries(&[item(&b), item(&a)]).is_err());
        assert!(validate_entries(&[item(&a), item(&a)]).is_err());
    }
}

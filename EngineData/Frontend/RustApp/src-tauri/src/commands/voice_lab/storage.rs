use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::engine::paths::ProjectPaths;

mod recording_archive;
pub(crate) use recording_archive::archive_guided_take;

#[cfg(target_os = "windows")]
#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptHash(
        algorithm: *mut std::ffi::c_void,
        secret: *mut u8,
        secret_len: u32,
        input: *mut u8,
        input_len: u32,
        output: *mut u8,
        output_len: u32,
    ) -> i32;
}

/// Native Windows CNG SHA-256, shared by held-out review and permanent audio.
/// Non-Windows targets fail closed; the supported desktop runtime is Windows.
#[cfg(target_os = "windows")]
pub(crate) fn sha256_digest(bytes: &[u8]) -> Option<[u8; 32]> {
    let len = u32::try_from(bytes.len()).ok()?;
    let mut digest = [0_u8; 32];
    // SAFETY: Windows CNG does not modify input and the 0x41 pseudo-handle is
    // SHA-256 on supported Windows; buffers and bounded lengths remain valid.
    let status = unsafe {
        BCryptHash(
            0x41_usize as *mut std::ffi::c_void,
            std::ptr::null_mut(),
            0,
            bytes.as_ptr() as *mut u8,
            len,
            digest.as_mut_ptr(),
            digest.len() as u32,
        )
    };
    (status == 0).then_some(digest)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn sha256_digest(_bytes: &[u8]) -> Option<[u8; 32]> {
    None
}

use super::{
    GuidedDatasetManifest, VoiceActorPackageManifest, ACTOR_MANIFEST_FILE,
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE,
    DATASET_MANIFEST_FILE, GPT_WEIGHT_FILE, MAX_MANIFEST_BYTES, MAX_REFERENCE_MS,
    MIN_REFERENCE_MS, REFERENCE_WAV_FILE, SOVITS_WEIGHT_FILE, VOICE_ACTOR_ENGINE,
    VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};

#[derive(Debug, Clone)]
pub struct VoiceLabStoragePaths {
    pub cache_root: PathBuf,
    pub takes_dir: PathBuf,
    pub build_dataset_dir: PathBuf,
    pub candidate_actor_dir: PathBuf,
    pub saved_root: PathBuf,
    pub approved_actor_dir: PathBuf,
    pub stored_personal_actor_dir: PathBuf,
}

impl VoiceLabStoragePaths {
    pub fn from_project_paths(paths: &ProjectPaths) -> Self {
        Self::from_roots(Path::new(&paths.user_cache_dir), Path::new(&paths.user_saved_dir))
    }

    pub(super) fn from_roots(cache_root: &Path, saved_root: &Path) -> Self {
        let cache_root = cache_root.join("VoiceLab");
        let saved_root = saved_root.join("VoiceLab");
        Self {
            takes_dir: cache_root.join("Takes"),
            build_dataset_dir: cache_root.join("Build").join("Dataset"),
            candidate_actor_dir: cache_root.join("Candidate"),
            approved_actor_dir: saved_root.join("MyVoice"),
            stored_personal_actor_dir: saved_root.join("PersonalVoice"),
            cache_root,
            saved_root,
        }
    }
}

// The worker keeps using the canonical MyVoice path. A separate validated
// PersonalVoice copy preserves an approved actor across built-in selection.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ActorKind {
    Missing,
    Builtin(Option<String>),
    Personal,
    Unverified,
}

pub(crate) struct SelectedVoiceInfo {
    pub kind: String,
    pub builtin_id: Option<String>,
    pub personal_available: bool,
}

fn actor_kind(path: &Path) -> ActorKind {
    if !path.exists() { return ActorKind::Missing; }
    if validate_actor_package(path).is_err() { return ActorKind::Unverified; }
    let Ok(bytes) = fs::read(path.join(ACTOR_MANIFEST_FILE)) else {
        return ActorKind::Unverified;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return ActorKind::Unverified;
    };
    if json.get("builtin_voice").and_then(serde_json::Value::as_bool) == Some(true) {
        let id = json.get("builtin_voice_id").and_then(serde_json::Value::as_str)
            .filter(|id| matches!(*id, "MaleVoice" | "FemaleVoice"))
            .map(str::to_string);
        ActorKind::Builtin(id)
    } else {
        // Legacy approved voices have no built-in marker; keep them as personal.
        ActorKind::Personal
    }
}

fn personal_next(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.saved_root.join(".PersonalVoice.next")
}
fn personal_previous(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.saved_root.join(".PersonalVoice.previous")
}

fn recover_personal_archive(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    let current = &paths.stored_personal_actor_dir;
    let prev = personal_previous(paths);
    let next = personal_next(paths);
    if current.exists() && !matches!(actor_kind(current), ActorKind::Personal) {
        return Err("voice_lab:personal_archive_unverified".to_string());
    }
    if prev.exists() {
        if !matches!(actor_kind(&prev), ActorKind::Personal) {
            return Err("voice_lab:personal_archive_previous_unverified".to_string());
        }
        if current.exists() {
            // A complete destination supersedes an older validated backup.
            fs::remove_dir_all(&prev)
                .map_err(|e| format!("voice_lab:archive_previous_cleanup_failed:{e}"))?;
        } else {
            // Interrupted rename: restore the previous complete personal actor.
            fs::rename(&prev, current)
                .map_err(|e| format!("voice_lab:archive_recovery_failed:{e}"))?;
        }
    }
    if next.exists() {
        // .next never became authoritative. The selected/previous copy survives.
        fs::remove_dir_all(&next)
            .map_err(|e| format!("voice_lab:archive_stage_cleanup_failed:{e}"))?;
    }
    Ok(())
}

// Recover only a single, fully validated interrupted active-actor backup.
// Do not delete backup data or choose between multiple possible actors.
pub(crate) fn recover_selected_actor_if_missing(
    paths: &VoiceLabStoragePaths,
) -> Result<bool, String> {
    let candidates = [
        paths.saved_root.join(".MyVoice.previous"),
        paths.saved_root.join(".MyVoice.builtin-previous"),
    ];
    let pending: Vec<&PathBuf> = candidates.iter().filter(|path| path.exists()).collect();
    if pending.is_empty() {
        return Ok(false);
    }
    if paths.approved_actor_dir.exists() || pending.len() != 1 {
        return Err("voice_lab:actor_recovery_ambiguous".to_string());
    }
    let backup = pending[0];
    let metadata = fs::symlink_metadata(backup)
        .map_err(|error| format!("voice_lab:actor_recovery_metadata_failed:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("voice_lab:actor_recovery_backup_not_directory".to_string());
    }
    validate_actor_package(backup)?;
    fs::rename(backup, &paths.approved_actor_dir)
        .map_err(|error| format!("voice_lab:actor_recovery_rename_failed:{error}"))?;
    Ok(true)
}

pub(crate) fn selected_voice_info(paths: &VoiceLabStoragePaths) -> SelectedVoiceInfo {
    let active = actor_kind(&paths.approved_actor_dir);
    let stored = actor_kind(&paths.stored_personal_actor_dir);
    let pending = actor_kind(&personal_previous(paths));
    let personal_available = matches!(active, ActorKind::Personal)
        || matches!(stored, ActorKind::Personal)
        || matches!(pending, ActorKind::Personal);
    let (kind, builtin_id) = match active {
        ActorKind::Builtin(id) => ("built_in".to_string(), id),
        ActorKind::Personal => ("personal".to_string(), None),
        ActorKind::Missing => ("none".to_string(), None),
        ActorKind::Unverified => ("unknown".to_string(), None),
    };
    SelectedVoiceInfo { kind, builtin_id, personal_available }
}

fn copy_actor_files(source: &Path, target: &Path) -> Result<(), String> {
    validate_actor_package(source)?;
    fs::create_dir_all(target)
        .map_err(|e| format!("voice_lab:actor_stage_create_failed:{e}"))?;
    for name in [ACTOR_MANIFEST_FILE, GPT_WEIGHT_FILE, SOVITS_WEIGHT_FILE, REFERENCE_WAV_FILE] {
        fs::copy(source.join(name), target.join(name))
            .map_err(|e| format!("voice_lab:actor_stage_copy_failed:{e}"))?;
    }
    validate_actor_package(target)?;
    Ok(())
}

pub(crate) fn archive_selected_personal_voice(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    match actor_kind(&paths.approved_actor_dir) {
        ActorKind::Missing | ActorKind::Builtin(_) => return Ok(()),
        ActorKind::Unverified => return Err("voice_lab:selected_actor_unverified".to_string()),
        ActorKind::Personal => {}
    }
    fs::create_dir_all(&paths.saved_root)
        .map_err(|e| format!("voice_lab:archive_root_unavailable:{e}"))?;
    recover_personal_archive(paths)?;
    let current = &paths.stored_personal_actor_dir;
    let next = personal_next(paths);
    let prev = personal_previous(paths);
    if let Err(error) = copy_actor_files(&paths.approved_actor_dir, &next) {
        let _ = fs::remove_dir_all(&next);
        return Err(error);
    }
    if current.exists() {
        fs::rename(current, &prev)
            .map_err(|e| format!("voice_lab:archive_backup_failed:{e}"))?;
    }
    if let Err(error) = fs::rename(&next, current) {
        if prev.exists() && !current.exists() {
            fs::rename(&prev, current)
                .map_err(|e| format!("voice_lab:archive_commit_failed:{error};rollback_failed:{e}"))?;
        }
        return Err(format!("voice_lab:archive_commit_failed:{error}"));
    }
    // The staged actor was validated before the rename.
    if prev.exists() { let _ = fs::remove_dir_all(prev); }
    Ok(())
}

pub(crate) fn activate_stored_personal_voice(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    match actor_kind(&paths.approved_actor_dir) {
        ActorKind::Personal => return Ok(()),
        ActorKind::Unverified => return Err("voice_lab:selected_actor_unverified".to_string()),
        ActorKind::Missing | ActorKind::Builtin(_) => {}
    }
    recover_personal_archive(paths)?;
    let source = &paths.stored_personal_actor_dir;
    if !matches!(actor_kind(source), ActorKind::Personal) {
        return Err("voice_lab:personal_actor_not_available".to_string());
    }
    // Copy to the legacy active actor path without consuming the saved copy.
    super::super::builtin_voice::replace_directory_atomically(
        &paths.approved_actor_dir,
        |staging| copy_actor_files(source, staging),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanonicalWavInfo {
    duration_ms: u64,
}



const MAX_NATIVE_RATE_SNAPSHOT_BYTES: u64 = 25 * 1024 * 1024;

static PROMOTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn promotion_lock() -> &'static Mutex<()> {
    PROMOTION_LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn canonical_take_file_name(line_id: u32) -> String {
    format!("take_{line_id:04}.wav")
}

pub(super) fn validate_guided_dataset_manifest(manifest: &GuidedDatasetManifest) -> Result<(), String> {
    if manifest.schema_version != VOICE_LAB_SCHEMA_VERSION {
        return Err("voice_lab:unsupported_dataset_schema".to_string());
    }
    if !manifest.authorized_voice_confirmed {
        return Err("voice_lab:voice_authorization_required".to_string());
    }
    if manifest.takes.is_empty() {
        return Err("voice_lab:no_accepted_takes".to_string());
    }
    if manifest.held_out_lines.is_empty() {
        return Err("voice_lab:no_held_out_evaluation_lines".to_string());
    }

    let mut training_ids = HashSet::new();
    let mut training_text = HashSet::new();
    let mut training_files = HashSet::new();
    for take in &manifest.takes {
        let text = take.exact_text.trim();
        if take.line_id == 0 || text.is_empty() {
            return Err("voice_lab:invalid_guided_take".to_string());
        }
        if take.wav_file != canonical_take_file_name(take.line_id) {
            return Err("voice_lab:noncanonical_take_filename".to_string());
        }
        if !training_ids.insert(take.line_id)
            || !training_text.insert(text.to_string())
            || !training_files.insert(take.wav_file.clone())
        {
            return Err("voice_lab:duplicate_guided_take".to_string());
        }
    }

    let mut held_out_ids = HashSet::new();
    let mut held_out_text = HashSet::new();
    for line in &manifest.held_out_lines {
        let text = line.exact_text.trim();
        if line.line_id == 0 || text.is_empty() {
            return Err("voice_lab:invalid_held_out_line".to_string());
        }
        if !held_out_ids.insert(line.line_id) || !held_out_text.insert(text.to_string()) {
            return Err("voice_lab:duplicate_held_out_line".to_string());
        }
        if training_ids.contains(&line.line_id) || training_text.contains(text) {
            return Err("voice_lab:held_out_line_used_for_training".to_string());
        }
    }
    Ok(())
}

pub(super) fn prepare_guided_dataset_at(
    storage: &VoiceLabStoragePaths,
    manifest: &GuidedDatasetManifest,
) -> Result<PathBuf, String> {
    validate_guided_dataset_manifest(manifest)?;
    fs::create_dir_all(&storage.takes_dir)
        .map_err(|error| format!("voice_lab:takes_directory_unavailable:{error}"))?;

    for take in &manifest.takes {
        inspect_canonical_wav(&storage.takes_dir.join(&take.wav_file))?;
    }

    if storage.build_dataset_dir.exists() {
        fs::remove_dir_all(&storage.build_dataset_dir)
            .map_err(|error| format!("voice_lab:build_dataset_cleanup_failed:{error}"))?;
    }
    fs::create_dir_all(&storage.build_dataset_dir)
        .map_err(|error| format!("voice_lab:build_dataset_create_failed:{error}"))?;

    // Retain only bounded regular native-rate files in this frozen build,
    // without changing canonical GPT-SoVITS input or blocking legacy takes.
    let native_source_root = storage.takes_dir.join("NativeRate");
    let native_root_valid = fs::symlink_metadata(&native_source_root)
        .map(|meta| meta.is_dir() && !meta.file_type().is_symlink())
        .unwrap_or(false);
    let native_dataset_root = storage.build_dataset_dir.join("NativeRate");
    for take in &manifest.takes {
        let source = storage.takes_dir.join(&take.wav_file);
        let target = storage.build_dataset_dir.join(&take.wav_file);
        fs::copy(&source, &target)
            .map_err(|error| format!("voice_lab:build_take_copy_failed:{error}"))?;
        inspect_canonical_wav(&target)?;

        if native_root_valid {
            let native = native_source_root.join(&take.wav_file);
            let native_valid = fs::symlink_metadata(&native)
                .map(|meta| {
                    meta.is_file()
                        && !meta.file_type().is_symlink()
                        && (44..=MAX_NATIVE_RATE_SNAPSHOT_BYTES).contains(&meta.len())
                })
                .unwrap_or(false);
            if native_valid {
                fs::create_dir_all(&native_dataset_root)
                    .map_err(|error| format!("voice_lab:native_dataset_dir_failed:{error}"))?;
                fs::copy(&native, native_dataset_root.join(&take.wav_file))
                    .map_err(|error| format!("voice_lab:native_dataset_copy_failed:{error}"))?;
            }
        }
    }

    let manifest_path = storage.build_dataset_dir.join(DATASET_MANIFEST_FILE);
    write_json(&manifest_path, manifest)?;
    Ok(manifest_path)
}

pub(super) fn validate_actor_package(dir: &Path) -> Result<VoiceActorPackageManifest, String> {
    let manifest_path = dir.join(ACTOR_MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("voice_lab:actor_manifest_missing:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("voice_lab:actor_manifest_not_regular_file".to_string());
    }
    if metadata.len() == 0 || metadata.len() > MAX_MANIFEST_BYTES {
        return Err("voice_lab:actor_manifest_size_invalid".to_string());
    }
    let bytes = fs::read(&manifest_path)
        .map_err(|error| format!("voice_lab:actor_manifest_read_failed:{error}"))?;
    let manifest = serde_json::from_slice::<VoiceActorPackageManifest>(&bytes)
        .map_err(|error| format!("voice_lab:actor_manifest_invalid_json:{error}"))?;

    if manifest.schema_version != VOICE_LAB_SCHEMA_VERSION
        || manifest.engine != VOICE_ACTOR_ENGINE
        || manifest.engine_revision != VOICE_ACTOR_ENGINE_REVISION
    {
        return Err("voice_lab:actor_engine_contract_mismatch".to_string());
    }
    if manifest.gpt_weight_file != GPT_WEIGHT_FILE
        || manifest.sovits_weight_file != SOVITS_WEIGHT_FILE
        || manifest.reference_wav_file != REFERENCE_WAV_FILE
    {
        return Err("voice_lab:actor_package_filename_mismatch".to_string());
    }
    if manifest.reference_text.trim().is_empty() {
        return Err("voice_lab:actor_reference_text_missing".to_string());
    }
    if !manifest.held_out_evaluation_complete {
        return Err("voice_lab:actor_evaluation_incomplete".to_string());
    }

    validate_nonempty_regular_file(&dir.join(GPT_WEIGHT_FILE), "gpt_weight")?;
    validate_nonempty_regular_file(&dir.join(SOVITS_WEIGHT_FILE), "sovits_weight")?;
    let wav = inspect_canonical_wav(&dir.join(REFERENCE_WAV_FILE))?;
    if wav.duration_ms < MIN_REFERENCE_MS
        || wav.duration_ms > MAX_REFERENCE_MS
        || manifest.reference_duration_ms != wav.duration_ms
    {
        return Err("voice_lab:actor_reference_duration_invalid".to_string());
    }
    Ok(manifest)
}

fn validate_nonempty_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:{label}_missing:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(format!("voice_lab:{label}_invalid"));
    }
    Ok(())
}

pub(super) fn promote_voice_actor_candidate_at(storage: &VoiceLabStoragePaths) -> Result<(), String> {
    let _guard = promotion_lock()
        .lock()
        .map_err(|_| "voice_lab:promotion_state_unavailable".to_string())?;
    fs::create_dir_all(&storage.saved_root)
        .map_err(|error| format!("voice_lab:saved_root_unavailable:{error}"))?;
    recover_interrupted_promotion(storage)?;
    validate_actor_package(&storage.candidate_actor_dir)?;

    let staging = storage.saved_root.join(".MyVoice.next");
    let previous = storage.saved_root.join(".MyVoice.previous");
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("voice_lab:staging_cleanup_failed:{error}"))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|error| format!("voice_lab:staging_create_failed:{error}"))?;

    for file_name in [ACTOR_MANIFEST_FILE, GPT_WEIGHT_FILE, SOVITS_WEIGHT_FILE, REFERENCE_WAV_FILE] {
        fs::copy(storage.candidate_actor_dir.join(file_name), staging.join(file_name))
            .map_err(|error| format!("voice_lab:actor_stage_copy_failed:{error}"))?;
    }
    validate_actor_package(&staging)?;

    if storage.approved_actor_dir.exists() {
        if previous.exists() {
            fs::remove_dir_all(&previous)
                .map_err(|error| format!("voice_lab:previous_cleanup_failed:{error}"))?;
        }
        fs::rename(&storage.approved_actor_dir, &previous)
            .map_err(|error| format!("voice_lab:approved_actor_backup_failed:{error}"))?;
    }

    if let Err(error) = fs::rename(&staging, &storage.approved_actor_dir) {
        if previous.exists() && !storage.approved_actor_dir.exists() {
            if let Err(rollback_error) = fs::rename(&previous, &storage.approved_actor_dir) {
                return Err(format!(
                    "voice_lab:actor_promotion_failed:{error};rollback_failed:{rollback_error}"
                ));
            }
        }
        return Err(format!("voice_lab:actor_promotion_failed:{error}"));
    }

    if let Err(validation_error) = validate_actor_package(&storage.approved_actor_dir) {
        let remove_result = fs::remove_dir_all(&storage.approved_actor_dir);
        if let Err(remove_error) = remove_result {
            return Err(format!(
                "voice_lab:promoted_actor_validation_failed:{validation_error};rollback_remove_failed:{remove_error}"
            ));
        }
        if previous.exists() {
            if let Err(rollback_error) = fs::rename(&previous, &storage.approved_actor_dir) {
                return Err(format!(
                    "voice_lab:promoted_actor_validation_failed:{validation_error};rollback_failed:{rollback_error}"
                ));
            }
        }
        return Err(format!(
            "voice_lab:promoted_actor_validation_failed:{validation_error}"
        ));
    }
    if previous.exists() {
        let _ = fs::remove_dir_all(previous);
    }
    Ok(())
}

fn recover_interrupted_promotion(storage: &VoiceLabStoragePaths) -> Result<(), String> {
    let staging = storage.saved_root.join(".MyVoice.next");
    let previous = storage.saved_root.join(".MyVoice.previous");

    if storage.approved_actor_dir.exists() {
        validate_actor_package(&storage.approved_actor_dir)?;
        if staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|error| format!("voice_lab:stale_staging_cleanup_failed:{error}"))?;
        }
        if previous.exists() {
            fs::remove_dir_all(&previous)
                .map_err(|error| format!("voice_lab:stale_previous_cleanup_failed:{error}"))?;
        }
        return Ok(());
    }

    if previous.exists() {
        validate_actor_package(&previous)?;
        fs::rename(&previous, &storage.approved_actor_dir)
            .map_err(|error| format!("voice_lab:promotion_recovery_failed:{error}"))?;
    }
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("voice_lab:stale_staging_cleanup_failed:{error}"))?;
    }
    Ok(())
}

fn inspect_canonical_wav(path: &Path) -> Result<CanonicalWavInfo, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:wav_missing:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() < 44 {
        return Err("voice_lab:wav_invalid_file".to_string());
    }

    let mut file = File::open(path).map_err(|error| format!("voice_lab:wav_open_failed:{error}"))?;
    let mut riff = [0u8; 12];
    file.read_exact(&mut riff)
        .map_err(|error| format!("voice_lab:wav_header_read_failed:{error}"))?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err("voice_lab:wav_not_pcm_container".to_string());
    }

    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data_size: Option<u64> = None;
    loop {
        let position = file
            .stream_position()
            .map_err(|error| format!("voice_lab:wav_seek_failed:{error}"))?;
        if position.saturating_add(8) > metadata.len() {
            break;
        }
        let mut header = [0u8; 8];
        file.read_exact(&mut header)
            .map_err(|error| format!("voice_lab:wav_chunk_header_failed:{error}"))?;
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
        let start = file
            .stream_position()
            .map_err(|error| format!("voice_lab:wav_seek_failed:{error}"))?;
        if start.saturating_add(size) > metadata.len() {
            return Err("voice_lab:wav_chunk_out_of_bounds".to_string());
        }

        if &header[0..4] == b"fmt " {
            if size < 16 {
                return Err("voice_lab:wav_format_chunk_too_small".to_string());
            }
            let mut fmt = [0u8; 16];
            file.read_exact(&mut fmt)
                .map_err(|error| format!("voice_lab:wav_format_read_failed:{error}"))?;
            format = Some((
                u16::from_le_bytes([fmt[0], fmt[1]]),
                u16::from_le_bytes([fmt[2], fmt[3]]),
                u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]),
                u16::from_le_bytes([fmt[14], fmt[15]]),
            ));
        } else if &header[0..4] == b"data" {
            data_size = Some(size);
        }

        file.seek(SeekFrom::Start(start.saturating_add(size.saturating_add(size % 2))))
            .map_err(|error| format!("voice_lab:wav_seek_failed:{error}"))?;
        if format.is_some() && data_size.is_some() {
            break;
        }
    }

    let Some((audio_format, channels, sample_rate, bits_per_sample)) = format else {
        return Err("voice_lab:wav_format_missing".to_string());
    };
    let Some(data_size) = data_size else {
        return Err("voice_lab:wav_data_missing".to_string());
    };
    if audio_format != 1
        || channels != CANONICAL_CHANNELS
        || sample_rate != CANONICAL_SAMPLE_RATE
        || bits_per_sample != CANONICAL_BITS_PER_SAMPLE
    {
        return Err("voice_lab:wav_not_canonical_pcm16_32khz_mono".to_string());
    }
    if data_size == 0 {
        return Err("voice_lab:wav_empty".to_string());
    }
    let bytes_per_second = u64::from(sample_rate)
        .saturating_mul(u64::from(channels))
        .saturating_mul(u64::from(bits_per_sample / 8));
    let duration_ms = data_size.saturating_mul(1_000) / bytes_per_second;
    if duration_ms == 0 {
        return Err("voice_lab:wav_too_short".to_string());
    }
    Ok(CanonicalWavInfo { duration_ms })
}

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "voice_lab:manifest_parent_missing".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("voice_lab:manifest_parent_create_failed:{error}"))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("voice_lab:manifest_serialize_failed:{error}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("voice_lab:manifest_size_invalid".to_string());
    }
    fs::write(path, bytes).map_err(|error| format!("voice_lab:manifest_write_failed:{error}"))
}

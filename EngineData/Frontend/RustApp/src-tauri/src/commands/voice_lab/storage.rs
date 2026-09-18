use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::engine::paths::ProjectPaths;

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
            cache_root,
            saved_root,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanonicalWavInfo {
    duration_ms: u64,
}



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

    for take in &manifest.takes {
        let source = storage.takes_dir.join(&take.wav_file);
        let target = storage.build_dataset_dir.join(&take.wav_file);
        fs::copy(&source, &target)
            .map_err(|error| format!("voice_lab:build_take_copy_failed:{error}"))?;
        inspect_canonical_wav(&target)?;
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

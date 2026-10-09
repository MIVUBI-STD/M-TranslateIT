use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};

const BUILTIN_VOICE_IDS: &[&str] = &["MaleVoice", "FemaleVoice"];
const BUILTIN_GPT_WEIGHT: &str = "GPTSoVITS/Source/GPT_SoVITS/pretrained_models/s1v3.ckpt";
const BUILTIN_SOVITS_WEIGHT: &str =
    "GPTSoVITS/Source/GPT_SoVITS/pretrained_models/v2Pro/s2Gv2ProPlus.pth";
const STAGING_DIR: &str = ".MyVoice.builtin-next";
const PREVIOUS_DIR: &str = ".MyVoice.builtin-previous";

static BUILTIN_SWAP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn swap_lock() -> &'static Mutex<()> {
    BUILTIN_SWAP_LOCK.get_or_init(|| Mutex::new(()))
}

pub fn is_supported_builtin_voice(voice_id: &str) -> bool {
    BUILTIN_VOICE_IDS.contains(&voice_id)
}

pub fn meeting_blocks_voice_change() -> bool {
    let report = latest_runtime_session_state();
    match report.snapshot {
        Some(snapshot) => snapshot.owner_id == APPLICATION_MEETING_OWNER_ID,
        None => report.has_active_session,
    }
}

pub fn install_builtin_voice(
    project_paths: &ProjectPaths,
    voice_id: &str,
    target_dir: &Path,
    expected_engine_revision: &str,
) -> Result<(), String> {
    if !is_supported_builtin_voice(voice_id) {
        return Err("builtin_voice:unknown_voice".to_string());
    }

    let voice_root = PathBuf::from(project_paths.voice_runtime_dir.clone());
    let engine_revision = fs::read_to_string(
        voice_root.join("GPTSoVITS/Source/TRANSLATEIT_GPTSOVITS_REVISION.txt"),
    )
    .map_err(|_| "builtin_voice:engine_revision_unavailable".to_string())?;
    let engine_revision = engine_revision.trim();
    if engine_revision.is_empty() || engine_revision != expected_engine_revision {
        return Err("builtin_voice:engine_revision_mismatch".to_string());
    }

    let builtin_dir = voice_root.join("BuiltInVoices").join(voice_id);
    let gpt_weight = voice_root.join(BUILTIN_GPT_WEIGHT);
    let sovits_weight = voice_root.join(BUILTIN_SOVITS_WEIGHT);
    replace_directory_atomically(target_dir, |staging| {
        provision_builtin_voice(
            &builtin_dir,
            &gpt_weight,
            &sovits_weight,
            staging,
            engine_revision,
        )
    })
}

fn wav_duration_ms(path: &Path) -> Result<u64, String> {
    let bytes = fs::read(path).map_err(|_| "builtin_voice:wav_unreadable".to_string())?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("builtin_voice:wav_header_invalid".to_string());
    }

    let mut offset = 12usize;
    let mut byte_rate = 0u32;
    let mut data_len = 0u32;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            if offset + 8 + 16 > bytes.len() {
                return Err("builtin_voice:wav_fmt_truncated".to_string());
            }
            byte_rate = u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap());
        }
        if id == b"data" {
            let remaining = bytes.len() - offset - 8;
            data_len = remaining.min(size) as u32;
            break;
        }
        offset += 8 + size + (size & 1);
    }
    if byte_rate == 0 || data_len == 0 {
        return Err("builtin_voice:wav_format_missing".to_string());
    }
    Ok((u64::from(data_len) * 1_000) / u64::from(byte_rate))
}

fn reference_text_from_source(source_txt: &Path) -> Result<String, String> {
    let body = fs::read_to_string(source_txt)
        .map_err(|_| "builtin_voice:source_text_unreadable".to_string())?;
    body.lines()
        .find_map(|line| line.strip_prefix("reference_text: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "builtin_voice:reference_text_missing".to_string())
}

fn provision_builtin_voice(
    builtin_dir: &Path,
    gpt_weight_src: &Path,
    sovits_weight_src: &Path,
    target_dir: &Path,
    engine_revision: &str,
) -> Result<(), String> {
    let src_wav = builtin_dir.join("reference.wav");
    let src_source = builtin_dir.join("REFERENCE_SOURCE.txt");
    for path in [gpt_weight_src, sovits_weight_src, &src_wav, &src_source] {
        if !path.is_file() {
            return Err(format!("builtin_voice:asset_missing:{}", path.display()));
        }
    }

    let duration_ms = wav_duration_ms(&src_wav)?;
    if !(2_000..=15_000).contains(&duration_ms) {
        return Err(format!(
            "builtin_voice:reference_duration_out_of_range:{duration_ms}"
        ));
    }
    let reference_text = reference_text_from_source(&src_source)?;

    fs::create_dir_all(target_dir)
        .map_err(|_| "builtin_voice:target_create_failed".to_string())?;
    fs::copy(&src_wav, target_dir.join("reference.wav"))
        .map_err(|_| "builtin_voice:reference_copy_failed".to_string())?;
    fs::copy(gpt_weight_src, target_dir.join("gpt.ckpt"))
        .map_err(|_| "builtin_voice:gpt_copy_failed".to_string())?;
    fs::copy(sovits_weight_src, target_dir.join("sovits.pth"))
        .map_err(|_| "builtin_voice:sovits_copy_failed".to_string())?;

    let actor = serde_json::json!({
        "schema_version": 1,
        "engine": "gpt-sovits-v2proplus",
        "engine_revision": engine_revision,
        "gpt_weight_file": "gpt.ckpt",
        "sovits_weight_file": "sovits.pth",
        "reference_wav_file": "reference.wav",
        "reference_text": reference_text,
        "reference_duration_ms": duration_ms,
        "held_out_evaluation_complete": true,
        "builtin_voice": true,
    });
    fs::write(
        target_dir.join("actor.json"),
        serde_json::to_vec_pretty(&actor)
            .map_err(|_| "builtin_voice:actor_serialize_failed".to_string())?,
    )
    .map_err(|_| "builtin_voice:actor_write_failed".to_string())?;
    Ok(())
}

fn recover_interrupted_swap(
    target_dir: &Path,
    staging: &Path,
    previous: &Path,
) -> Result<(), String> {
    if target_dir.exists() {
        if staging.exists() {
            fs::remove_dir_all(staging)
                .map_err(|_| "builtin_voice:stale_staging_cleanup_failed".to_string())?;
        }
        if previous.exists() {
            fs::remove_dir_all(previous)
                .map_err(|_| "builtin_voice:stale_previous_cleanup_failed".to_string())?;
        }
        return Ok(());
    }

    if previous.exists() {
        fs::rename(previous, target_dir)
            .map_err(|_| "builtin_voice:swap_recovery_failed".to_string())?;
    }
    if staging.exists() {
        fs::remove_dir_all(staging)
            .map_err(|_| "builtin_voice:stale_staging_cleanup_failed".to_string())?;
    }
    Ok(())
}

pub fn replace_directory_atomically<F>(target_dir: &Path, build_staging: F) -> Result<(), String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let _guard = swap_lock()
        .lock()
        .map_err(|_| "builtin_voice:swap_lock_unavailable".to_string())?;
    let parent = target_dir
        .parent()
        .ok_or_else(|| "builtin_voice:target_parent_missing".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|_| "builtin_voice:target_parent_unavailable".to_string())?;
    let staging = parent.join(STAGING_DIR);
    let previous = parent.join(PREVIOUS_DIR);
    recover_interrupted_swap(target_dir, &staging, &previous)?;

    fs::create_dir_all(&staging)
        .map_err(|_| "builtin_voice:staging_create_failed".to_string())?;
    if let Err(error) = build_staging(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    if target_dir.exists() {
        if previous.exists() {
            fs::remove_dir_all(&previous)
                .map_err(|_| "builtin_voice:previous_cleanup_failed".to_string())?;
        }
        fs::rename(target_dir, &previous)
            .map_err(|_| "builtin_voice:current_backup_failed".to_string())?;
    }

    if let Err(error) = fs::rename(&staging, target_dir) {
        if previous.exists() && !target_dir.exists() {
            if fs::rename(&previous, target_dir).is_err() {
                return Err(format!("builtin_voice:swap_failed:{error};rollback_failed"));
            }
        }
        return Err(format!("builtin_voice:swap_failed:{error}"));
    }

    if previous.exists() {
        let _ = fs::remove_dir_all(previous);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit_builtin_{label}_{now}"))
    }

    fn write_wav(path: &Path) {
        let data = vec![0u8; 320_000];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&64_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&data);
        fs::write(path, bytes).expect("write wav");
    }

    fn write_assets(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let builtin = root.join("MaleVoice");
        fs::create_dir_all(&builtin).expect("create builtin");
        write_wav(&builtin.join("reference.wav"));
        fs::write(
            builtin.join("REFERENCE_SOURCE.txt"),
            "reference_text: Hello world\n",
        )
        .expect("source text");
        let gpt = root.join("gpt.ckpt");
        let sovits = root.join("sovits.pth");
        fs::write(&gpt, b"G").expect("gpt");
        fs::write(&sovits, b"S").expect("sovits");
        (builtin, gpt, sovits)
    }

    #[test]
    fn provision_writes_exact_builtin_manifest() {
        let root = root("manifest");
        let (builtin, gpt, sovits) = write_assets(&root);
        let target = root.join("target");
        provision_builtin_voice(&builtin, &gpt, &sovits, &target, "rev-test")
            .expect("provision");
        let actor: serde_json::Value = serde_json::from_slice(
            &fs::read(target.join("actor.json")).expect("actor"),
        )
        .expect("actor json");
        assert_eq!(actor["builtin_voice"], true);
        assert_eq!(actor["reference_text"], "Hello world");
        assert_eq!(actor["reference_duration_ms"], 5_000);
        assert_eq!(actor["engine_revision"], "rev-test");
        assert!(target.join("gpt.ckpt").is_file());
        assert!(target.join("sovits.pth").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_staging_preserves_existing_selected_voice() {
        let root = root("preserve");
        let target = root.join("saved").join("MyVoice");
        fs::create_dir_all(&target).expect("target");
        fs::write(target.join("current.marker"), b"old").expect("marker");

        let error = replace_directory_atomically(&target, |_staging| {
            Err("builtin_voice:test_failure".to_string())
        })
        .expect_err("must fail");

        assert_eq!(error, "builtin_voice:test_failure");
        assert_eq!(fs::read(target.join("current.marker")).expect("old voice"), b"old");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn successful_swap_replaces_old_voice_as_one_directory() {
        let root = root("swap");
        let target = root.join("saved").join("MyVoice");
        fs::create_dir_all(&target).expect("target");
        fs::write(target.join("current.marker"), b"old").expect("marker");

        replace_directory_atomically(&target, |staging| {
            fs::write(staging.join("actor.json"), b"new").map_err(|error| error.to_string())
        })
        .expect("swap");

        assert!(!target.join("current.marker").exists());
        assert_eq!(fs::read(target.join("actor.json")).expect("new voice"), b"new");
        assert!(!root.join("saved").join(PREVIOUS_DIR).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_swap_restores_previous_voice_before_retry() {
        let root = root("recovery");
        let parent = root.join("saved");
        let target = parent.join("MyVoice");
        let previous = parent.join(PREVIOUS_DIR);
        let staging = parent.join(STAGING_DIR);
        fs::create_dir_all(&previous).expect("previous");
        fs::write(previous.join("current.marker"), b"old").expect("marker");
        fs::create_dir_all(&staging).expect("staging");
        fs::write(staging.join("partial.marker"), b"partial").expect("partial");

        recover_interrupted_swap(&target, &staging, &previous).expect("recover");
        assert_eq!(fs::read(target.join("current.marker")).expect("restored"), b"old");
        assert!(!staging.exists());
        assert!(!previous.exists());
        let _ = fs::remove_dir_all(root);
    }
}

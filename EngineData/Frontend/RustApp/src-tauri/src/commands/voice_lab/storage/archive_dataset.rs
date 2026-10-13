//! Explicit, bounded rebuild selection from previously verified saved takes.
//! Never changes a saved recording or the current selected Meeting actor.
use std::collections::HashSet;
use std::fs;

use super::immutable_store::write_new_synced;
use super::recording_archive::archived_take_for_training;
use super::{dataset_history, inspect_canonical_wav, validate_guided_dataset_manifest, write_json, VoiceLabStoragePaths};
use crate::commands::voice_lab::{GuidedDatasetManifest, GuidedTakeContract, VOICE_LAB_SCHEMA_VERSION};

const MAX_SELECTED_TAKES: usize = 128;

pub(crate) struct ArchivedSelection {
    pub takes: Vec<GuidedTakeContract>,
    pub archive_ids: Vec<String>,
    pub duration_ms: u64,
    pub reference_ready: bool,
}

/// Selection order becomes deterministic by prompt ID. Repeat recordings
/// of one prompt must be intentionally resolved to exactly one take per run.
pub(crate) fn choose_archived_takes(
    storage: &VoiceLabStoragePaths,
    archive_ids: &[String],
    prompts: &[(u32, &str)],
) -> Result<ArchivedSelection, String> {
    if archive_ids.len() < 2 || archive_ids.len() > MAX_SELECTED_TAKES {
        return Err("voice_lab:improvement_select_2_to_128_recordings".into());
    }
    let mut selected = Vec::with_capacity(archive_ids.len());
    let mut prompt_ids = HashSet::new();
    let mut unique_archives = HashSet::new();
    for id in archive_ids {
        if !unique_archives.insert(id.as_str()) {
            return Err("voice_lab:improvement_duplicate_recording".into());
        }
        let take = archived_take_for_training(storage, id)?;
        let expected = prompts.iter().find(|(line_id, _)| *line_id == take.line_id)
            .map(|(_, text)| *text)
            .ok_or("voice_lab:improvement_unknown_training_prompt")?;
        if take.exact_text != expected {
            return Err("voice_lab:improvement_transcript_mismatch".into());
        }
        if !prompt_ids.insert(take.line_id) {
            return Err("voice_lab:improvement_one_recording_per_line".into());
        }
        selected.push((take.line_id, id.clone(), take.duration_ms, expected.to_string()));
    }
    selected.sort_by_key(|item| item.0);
    let duration_ms = selected.iter().fold(0_u64, |total, x| total.saturating_add(x.2));
    let reference_ready = selected.iter().any(|entry| (3_000..=10_000).contains(&entry.2));
    Ok(ArchivedSelection {
        takes: selected.iter().map(|(line_id, _, _, text)| GuidedTakeContract {
            line_id: *line_id,
            exact_text: text.clone(),
            wav_file: format!("take_{line_id:04}.wav"),
        }).collect(),
        archive_ids: selected.into_iter().map(|(_, id, _, _)| id).collect(),
        duration_ms,
        reference_ready,
    })
}

/// Freeze only the chosen immutable audio, not the mutable current-take path.
/// The existing GPT-SoVITS flat WAV layout and dataset.json stay identical.
pub(super) fn prepare_archived_dataset(
    storage: &VoiceLabStoragePaths,
    manifest: &GuidedDatasetManifest,
    archive_ids: &[String],
) -> Result<(), String> {
    validate_guided_dataset_manifest(manifest)?;
    if archive_ids.len() != manifest.takes.len() || archive_ids.len() > MAX_SELECTED_TAKES {
        return Err("voice_lab:improvement_archive_manifest_count_mismatch".into());
    }
    if storage.build_dataset_dir.exists() {
        fs::remove_dir_all(&storage.build_dataset_dir)
            .map_err(|_| "voice_lab:improvement_temp_dataset_unavailable")?;
    }
    fs::create_dir_all(&storage.build_dataset_dir)
        .map_err(|_| "voice_lab:improvement_temp_dataset_create_failed")?;
    let mut all_ids = HashSet::new();
    for (take, id) in manifest.takes.iter().zip(archive_ids) {
        if !all_ids.insert(id.as_str()) {
            return Err("voice_lab:improvement_duplicate_archive_id".into());
        }
        let source = archived_take_for_training(storage, id)?;
        if source.line_id != take.line_id || source.exact_text != take.exact_text {
            return Err("voice_lab:improvement_archive_prompt_changed".into());
        }
        let target = storage.build_dataset_dir.join(&take.wav_file);
        write_new_synced(&target, &source.canonical_audio)?;
        inspect_canonical_wav(&target)?;
        if let Some(native) = source.native_audio {
            let dir = storage.build_dataset_dir.join("NativeRate");
            fs::create_dir_all(&dir)
                .map_err(|_| "voice_lab:improvement_native_dir_failed")?;
            write_new_synced(&dir.join(&take.wav_file), &native)?;
        }
    }
    write_json(&storage.build_dataset_dir.join("dataset.json"), manifest)?;
    dataset_history::preserve_frozen_dataset_with_archives(storage, manifest, archive_ids)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_cap_is_explicit_not_a_six_hundred_take_promise() {
        assert_eq!(MAX_SELECTED_TAKES, 128);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn archive_selection_is_sorted_frozen_and_not_bound_to_mutable_cache_takes() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-rebuild-{}-{ts}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let prompts = [(1_u32,"Good morning."), (25_u32,"What do you think?")];
        let create = |id: u32, seed: u8| -> String {
            let samples = vec![seed; 4 * 64_000];
            let mut wav=Vec::new();
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
            let path=storage.takes_dir.join(format!("take_{id:04}.wav"));
            fs::write(&path, wav).unwrap();
            super::super::recording_archive::archive_guided_take(
                &storage, id, prompts.iter().find(|p|p.0==id).unwrap().1, &path, None,
            ).unwrap()
        };
        let a=create(1, 33);
        let b=create(25, 48);
        let chosen = choose_archived_takes(&storage, &[b.clone(),a.clone()], &prompts).unwrap();
        assert_eq!(chosen.takes[0].line_id,1);
        assert_eq!(chosen.archive_ids, vec![a.clone(),b.clone()]);
        assert!(choose_archived_takes(&storage, &[a.clone(),a.clone()], &prompts).is_err());
        let extra=create(1, 55);
        assert!(choose_archived_takes(&storage, &[a.clone(),extra.clone()], &prompts).is_err());
        let manifest = GuidedDatasetManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION, authorized_voice_confirmed: true,
            takes: chosen.takes,
            held_out_lines: vec![crate::commands::voice_lab::GuidedEvaluationLineContract {
                line_id: 1001, exact_text: "Never used to train.".into(),
            }],
        };
        prepare_archived_dataset(&storage, &manifest, &chosen.archive_ids).unwrap();
        assert_eq!(dataset_history::archived_frozen_review_matches(&storage).unwrap(), Some(true));
        // Current accepted recording changed after freeze; the archived review
        // must still refer to the original chosen immutable take.
        create(1, 80);
        assert_eq!(dataset_history::archived_frozen_review_matches(&storage).unwrap(), Some(true));
        fs::write(storage.build_dataset_dir.join("take_0001.wav"), b"tampered").unwrap();
        assert!(!dataset_history::archived_frozen_review_matches(&storage).unwrap_or(Some(false)).unwrap_or(false));
        fs::remove_dir_all(root).unwrap();
    }
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::voice_lab::{
    GuidedEvaluationLineContract, VoiceLabStoragePaths, VOICE_ACTOR_ENGINE,
    VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};

pub(super) const MAX_EVALUATION_WAV_BYTES: u64 = 16 * 1024 * 1024;

const HELD_OUT_LINES: &[(u32, &str)] = &[
    (1001, "Please confirm the final schedule before we send the update to the client."),
    (1002, "The system should remain clear and natural during a longer technical discussion."),
    (1003, "I can review the latest results tomorrow morning and share my decision with the team."),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLabEvaluationSample {
    pub line_id: u32,
    pub exact_text: String,
    pub wav_file: String,
    pub speaker_similarity: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct EvaluationManifest {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    pub(super) samples: Vec<VoiceLabEvaluationSample>,
}

pub(super) fn evaluation_dir(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Evaluation")
}

pub(super) fn held_out_contract() -> Vec<GuidedEvaluationLineContract> {
    HELD_OUT_LINES
        .iter()
        .map(|(line_id, exact_text)| GuidedEvaluationLineContract {
            line_id: *line_id,
            exact_text: (*exact_text).to_string(),
        })
        .collect()
}

pub(super) fn evaluation_manifest(paths: &VoiceLabStoragePaths) -> Option<EvaluationManifest> {
    let root = evaluation_dir(paths);
    let bytes = fs::read(root.join("evaluation.json")).ok()?;
    if bytes.is_empty() || bytes.len() > 128 * 1024 {
        return None;
    }
    let manifest = serde_json::from_slice::<EvaluationManifest>(&bytes).ok()?;
    if manifest.schema_version != VOICE_LAB_SCHEMA_VERSION
        || manifest.engine != VOICE_ACTOR_ENGINE
        || manifest.engine_revision != VOICE_ACTOR_ENGINE_REVISION
        || manifest.samples.len() != HELD_OUT_LINES.len()
    {
        return None;
    }
    for (expected_line_id, expected_text) in HELD_OUT_LINES {
        let matches = manifest
            .samples
            .iter()
            .filter(|sample| {
                sample.line_id == *expected_line_id
                    && sample.exact_text.trim() == expected_text.trim()
            })
            .count();
        if matches != 1 {
            return None;
        }
    }
    for sample in &manifest.samples {
        if !sample.speaker_similarity.is_finite()
            || sample.wav_file.trim().is_empty()
            || Path::new(&sample.wav_file).file_name().and_then(|name| name.to_str())
                != Some(sample.wav_file.as_str())
        {
            return None;
        }
        let wav = root.join(&sample.wav_file);
        let metadata = fs::symlink_metadata(wav).ok()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() < 44
            || metadata.len() > MAX_EVALUATION_WAV_BYTES
        {
            return None;
        }
    }
    Some(manifest)
}

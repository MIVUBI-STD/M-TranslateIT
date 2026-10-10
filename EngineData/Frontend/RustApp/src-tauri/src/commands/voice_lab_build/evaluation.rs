use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::super::voice_lab::{
    GuidedDatasetManifest, GuidedEvaluationLineContract, GuidedTakeContract, VoiceLabStoragePaths,
    VOICE_ACTOR_ENGINE, VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};

pub(super) const MAX_EVALUATION_WAV_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FROZEN_TAKE_WAV_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DATASET_MANIFEST_BYTES: u64 = 64 * 1024;
pub(super) const EVALUATION_SELECTION_METHOD: &str =
    "held_out_artifacts_then_mean_wer_then_max_wer_then_similarity_tiebreak";

const HELD_OUT_LINES: &[(u32, &str)] = &[
    (1001, "Please confirm the final schedule before we send the update to the client."),
    (1002, "The system should remain clear and natural during a longer technical discussion."),
    (1003, "I can review the latest results tomorrow morning and share my decision with the team."),
    (1004, "Do not restart the service until the backup is complete and the result is verified."),
    (1005, "Can everyone hear the translated voice clearly during this meeting?"),
    (1006, "The maintenance window starts at seven thirty in the evening on September twenty first."),
    (1007, "I said fifteen, not fifty, so please correct the invoice before approval."),
    (1008, "Before we continue, summarize the current status, the remaining risk, and the next action."),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLabEvaluationSample {
    pub line_id: u32,
    pub exact_text: String,
    pub wav_file: String,
    pub speaker_similarity: f64,
    pub intelligibility_text: String,
    pub intelligibility_wer: f64,
    pub artifact_flags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct EvaluationManifest {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    pub(super) selection_method: String,
    pub(super) selected_candidate_id: String,
    pub(super) review_id: String,
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

// A review can outlive the accepted takes used to train it. Compare against
// the actual frozen build dataset, not a UI revision or only filenames/sizes.
// This stays in the existing evaluation owner and does not mutate recordings.
fn same_bounded_wav_bytes(source: &Path, frozen: &Path) -> bool {
    let (Ok(a), Ok(b)) = (fs::symlink_metadata(source), fs::symlink_metadata(frozen)) else {
        return false;
    };
    if !a.is_file() || !b.is_file() || a.file_type().is_symlink() || b.file_type().is_symlink()
        || a.len() < 44 || a.len() > MAX_FROZEN_TAKE_WAV_BYTES || a.len() != b.len()
    {
        return false;
    }
    let (Ok(mut a_file), Ok(mut b_file)) = (File::open(source), File::open(frozen)) else {
        return false;
    };
    let mut a_buffer = [0_u8; 8192];
    let mut b_buffer = [0_u8; 8192];
    loop {
        let Ok(count) = a_file.read(&mut a_buffer) else {
            return false;
        };
        if count == 0 {
            return matches!(b_file.read(&mut b_buffer), Ok(0));
        }
        if b_file.read_exact(&mut b_buffer[..count]).is_err()
            || a_buffer[..count] != b_buffer[..count]
        {
            return false;
        }
    }
}

pub(super) fn frozen_dataset_matches_current(
    takes_dir: &Path,
    dataset_dir: &Path,
    current_takes: &[GuidedTakeContract],
) -> bool {
    let dataset_manifest = dataset_dir.join("dataset.json");
    let Ok(info) = fs::symlink_metadata(&dataset_manifest) else {
        return false;
    };
    if !info.is_file() || info.file_type().is_symlink()
        || info.len() == 0 || info.len() > MAX_DATASET_MANIFEST_BYTES
    {
        return false;
    }
    let Ok(bytes) = fs::read(dataset_manifest) else {
        return false;
    };
    let Ok(frozen) = serde_json::from_slice::<GuidedDatasetManifest>(&bytes) else {
        return false;
    };
    if frozen.schema_version != VOICE_LAB_SCHEMA_VERSION
        || !frozen.authorized_voice_confirmed
        || frozen.held_out_lines != held_out_contract()
        || frozen.takes.is_empty()
        || frozen.takes != current_takes
    {
        return false;
    }
    frozen.takes.iter().all(|take| {
        same_bounded_wav_bytes(
            &takes_dir.join(&take.wav_file),
            &dataset_dir.join(&take.wav_file),
        )
    })
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
        || manifest.selection_method != EVALUATION_SELECTION_METHOD
        || manifest.selected_candidate_id.trim().is_empty()
        || manifest.review_id.len() != 64
        || !manifest.review_id.bytes().all(|byte| byte.is_ascii_hexdigit())
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
            || !sample.intelligibility_wer.is_finite()
            || sample.intelligibility_wer < 0.0
            || sample.intelligibility_text.trim().is_empty()
            || sample.artifact_flags.iter().any(|flag| {
                !matches!(flag.as_str(), "clipping" | "unexpected_silence")
            })
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


pub(super) fn reviewable_evaluation(
    paths: &VoiceLabStoragePaths,
    current_takes: &[GuidedTakeContract],
) -> Option<EvaluationManifest> {
    let evaluation = evaluation_manifest(paths)?;
    let actor_bytes = fs::read(paths.candidate_actor_dir.join("actor.json")).ok()?;
    let actor_json = serde_json::from_slice::<serde_json::Value>(&actor_bytes).ok()?;
    if !candidate_selection_matches_actor_json(&evaluation, &actor_json)
        || !frozen_dataset_matches_current(&paths.takes_dir, &paths.build_dataset_dir, current_takes)
    {
        return None;
    }
    Some(evaluation)
}

pub(super) fn candidate_selection_matches_actor_json(
    manifest: &EvaluationManifest,
    actor_json: &serde_json::Value,
) -> bool {
    actor_json
        .get("candidate_selection")
        .and_then(|value| value.as_object())
        .and_then(|selection| {
            Some((
                selection.get("method")?.as_str()?,
                selection.get("candidate_id")?.as_str()?,
            ))
        })
        .map(|(method, candidate_id)| {
            method == manifest.selection_method && candidate_id == manifest.selected_candidate_id
        })
        .unwrap_or(false)
}


pub(super) fn evaluation_review_complete(
    manifest: &EvaluationManifest,
    reviewed_line_ids: &[u32],
) -> bool {
    use std::collections::BTreeSet;

    let expected = manifest
        .samples
        .iter()
        .map(|sample| sample.line_id)
        .collect::<BTreeSet<_>>();
    let reviewed = reviewed_line_ids.iter().copied().collect::<BTreeSet<_>>();
    reviewed.len() == reviewed_line_ids.len() && reviewed == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> EvaluationManifest {
        EvaluationManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            engine: VOICE_ACTOR_ENGINE.to_string(),
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.to_string(),
            selection_method: EVALUATION_SELECTION_METHOD.to_string(),
            selected_candidate_id: "s8-g15".to_string(),
            review_id: "a".repeat(64),
            samples: HELD_OUT_LINES
                .iter()
                .map(|(line_id, text)| VoiceLabEvaluationSample {
                    line_id: *line_id,
                    exact_text: (*text).to_string(),
                    wav_file: format!("held_out_{line_id}.wav"),
                    speaker_similarity: 0.9,
                    intelligibility_text: (*text).to_string(),
                    intelligibility_wer: 0.0,
                    artifact_flags: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn trained_review_requires_the_exact_frozen_recordings() {
        let root = std::env::temp_dir().join(format!(
            "translateit-frozen-take-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
        ));
        let takes_dir = root.join("Takes");
        let dataset_dir = root.join("Dataset");
        fs::create_dir_all(&takes_dir).expect("takes");
        fs::create_dir_all(&dataset_dir).expect("dataset");
        let filename = "take_0001.wav";
        let take = GuidedTakeContract {
            line_id: 1,
            exact_text: "Good morning, everyone.".into(),
            wav_file: filename.into(),
        };
        let snapshot = GuidedDatasetManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            authorized_voice_confirmed: true,
            takes: vec![take.clone()],
            held_out_lines: held_out_contract(),
        };
        fs::write(dataset_dir.join("dataset.json"), serde_json::to_vec(&snapshot).expect("json"))
            .expect("snapshot");
        let first = vec![1_u8; 64];
        fs::write(dataset_dir.join(filename), &first).expect("frozen audio");
        fs::write(takes_dir.join(filename), &first).expect("accepted audio");
        assert!(frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take.clone()]));

        // A replacement with exactly the same WAV length must still invalidate review.
        let second = vec![2_u8; 64];
        fs::write(takes_dir.join(filename), &second).expect("replace audio");
        assert!(!frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take.clone()]));
        fs::write(takes_dir.join(filename), &first).expect("restore audio");

        let added = GuidedTakeContract {
            line_id: 2,
            exact_text: "Thanks for joining today.".into(),
            wav_file: "take_0002.wav".into(),
        };
        assert!(!frozen_dataset_matches_current(
            &takes_dir,
            &dataset_dir,
            &[take.clone(), added],
        ));
        fs::remove_file(takes_dir.join(filename)).expect("remove accepted audio");
        assert!(!frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take]));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn review_identity_is_specific_and_well_formed() {
        let manifest = manifest();
        assert_eq!(manifest.review_id.len(), 64);
        assert_ne!(manifest.review_id, "b".repeat(64));
    }

    #[test]
    fn candidate_selection_binding_rejects_mismatched_actor() {
        let manifest = manifest();
        let matching = serde_json::json!({
            "candidate_selection": {
                "method": EVALUATION_SELECTION_METHOD,
                "candidate_id": "s8-g15"
            }
        });
        assert!(candidate_selection_matches_actor_json(&manifest, &matching));
        let stale = serde_json::json!({
            "candidate_selection": {
                "method": EVALUATION_SELECTION_METHOD,
                "candidate_id": "s6-g10"
            }
        });
        assert!(!candidate_selection_matches_actor_json(&manifest, &stale));
    }

    #[test]
    fn review_completion_requires_every_exact_held_out_line_once() {
        let manifest = manifest();
        let ids = HELD_OUT_LINES.iter().map(|(line_id, _)| *line_id).collect::<Vec<_>>();
        assert!(evaluation_review_complete(&manifest, &ids));
        assert!(!evaluation_review_complete(&manifest, &ids[..ids.len() - 1]));
        let mut duplicate = ids.clone();
        duplicate.push(ids[0]);
        assert!(!evaluation_review_complete(&manifest, &duplicate));
    }
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::latest_runtime_session_state;

mod storage;

pub use storage::VoiceLabStoragePaths;
use storage::{prepare_guided_dataset_at, promote_voice_actor_candidate_at, validate_actor_package};
#[cfg(test)]
use storage::{
    canonical_take_file_name, validate_guided_dataset_manifest, write_json,
};

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";
pub(super) const VOICE_LAB_SCHEMA_VERSION: u32 = 1;
pub(super) const VOICE_ACTOR_ENGINE: &str = "gpt-sovits-v2proplus";
pub(super) const VOICE_ACTOR_ENGINE_REVISION: &str = "d523079fc05d9a8028d6085bffe4a2757c32abb6";
const ACTOR_MANIFEST_FILE: &str = "actor.json";
const GPT_WEIGHT_FILE: &str = "gpt.ckpt";
const SOVITS_WEIGHT_FILE: &str = "sovits.pth";
const REFERENCE_WAV_FILE: &str = "reference.wav";
const DATASET_MANIFEST_FILE: &str = "dataset.json";
const CANONICAL_SAMPLE_RATE: u32 = 32_000;
const CANONICAL_CHANNELS: u16 = 1;
const CANONICAL_BITS_PER_SAMPLE: u16 = 16;
const MIN_REFERENCE_MS: u64 = 3_000;
const MAX_REFERENCE_MS: u64 = 10_000;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidedTakeContract {
    pub line_id: u32,
    pub exact_text: String,
    pub wav_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidedEvaluationLineContract {
    pub line_id: u32,
    pub exact_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidedDatasetManifest {
    pub schema_version: u32,
    pub authorized_voice_confirmed: bool,
    pub takes: Vec<GuidedTakeContract>,
    pub held_out_lines: Vec<GuidedEvaluationLineContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceActorPackageManifest {
    pub schema_version: u32,
    pub engine: String,
    pub engine_revision: String,
    pub gpt_weight_file: String,
    pub sovits_weight_file: String,
    pub reference_wav_file: String,
    pub reference_text: String,
    pub reference_duration_ms: u64,
    pub held_out_evaluation_complete: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VoiceLabBuildSnapshot {
    pub active: bool,
    pub generation: Option<u64>,
    pub phase: String,
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildPhase {
    Preparing,
    Training,
    Evaluating,
    Cancelling,
}

impl BuildPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Training => "training",
            Self::Evaluating => "evaluating",
            Self::Cancelling => "cancelling",
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveBuild {
    generation: u64,
    phase: BuildPhase,
    cancel_requested: bool,
}

#[derive(Debug, Default)]
struct BuildLifecycle {
    next_generation: u64,
    active: Option<ActiveBuild>,
}

impl BuildLifecycle {
    fn snapshot(&self) -> VoiceLabBuildSnapshot {
        match self.active.as_ref() {
            Some(active) => VoiceLabBuildSnapshot {
                active: true,
                generation: Some(active.generation),
                phase: active.phase.as_str().to_string(),
                cancel_requested: active.cancel_requested,
            },
            None => idle_snapshot(),
        }
    }

    fn begin(&mut self) -> Result<VoiceLabBuildSnapshot, String> {
        if self.active.is_some() {
            return Err("voice_lab:build_already_active".to_string());
        }
        self.next_generation = self.next_generation.saturating_add(1).max(1);
        self.active = Some(ActiveBuild {
            generation: self.next_generation,
            phase: BuildPhase::Preparing,
            cancel_requested: false,
        });
        Ok(self.snapshot())
    }

    fn transition(
        &mut self,
        generation: u64,
        expected: BuildPhase,
        next: BuildPhase,
    ) -> Result<VoiceLabBuildSnapshot, String> {
        let Some(active) = self.active.as_mut() else {
            return Err("voice_lab:no_active_build".to_string());
        };
        if active.generation != generation {
            return Err("voice_lab:stale_build_generation".to_string());
        }
        if active.phase != expected || active.cancel_requested {
            return Err("voice_lab:invalid_build_transition".to_string());
        }
        active.phase = next;
        Ok(self.snapshot())
    }

    fn cancel(&mut self, generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
        let Some(active) = self.active.as_mut() else {
            return Err("voice_lab:no_active_build".to_string());
        };
        if active.generation != generation {
            return Err("voice_lab:stale_build_generation".to_string());
        }
        if active.phase != BuildPhase::Cancelling {
            active.cancel_requested = true;
            active.phase = BuildPhase::Cancelling;
        }
        Ok(self.snapshot())
    }

    fn finish(&mut self, generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
        let Some(active) = self.active.as_ref() else {
            return Err("voice_lab:no_active_build".to_string());
        };
        if active.generation != generation {
            return Err("voice_lab:stale_build_generation".to_string());
        }
        if !matches!(active.phase, BuildPhase::Evaluating | BuildPhase::Cancelling) {
            return Err("voice_lab:invalid_build_transition".to_string());
        }
        self.active = None;
        Ok(self.snapshot())
    }

    fn fail(&mut self, generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
        let Some(active) = self.active.as_ref() else {
            return Err("voice_lab:no_active_build".to_string());
        };
        if active.generation != generation {
            return Err("voice_lab:stale_build_generation".to_string());
        }
        self.active = None;
        Ok(self.snapshot())
    }

    fn blocks_meeting(&self) -> bool {
        self.active.is_some()
    }
}

static BUILD_STATE: OnceLock<Mutex<BuildLifecycle>> = OnceLock::new();

fn build_store() -> &'static Mutex<BuildLifecycle> {
    BUILD_STATE.get_or_init(|| Mutex::new(BuildLifecycle::default()))
}



fn idle_snapshot() -> VoiceLabBuildSnapshot {
    VoiceLabBuildSnapshot {
        active: false,
        generation: None,
        phase: "idle".to_string(),
        cancel_requested: false,
    }
}

fn meeting_blocks_voice_lab() -> bool {
    let report = latest_runtime_session_state();
    match report.snapshot {
        Some(snapshot) => snapshot.owner_id == APPLICATION_MEETING_OWNER_ID,
        None => report.has_active_session,
    }
}

pub fn current_voice_lab_build_snapshot() -> VoiceLabBuildSnapshot {
    build_store()
        .lock()
        .map(|state| state.snapshot())
        .unwrap_or_else(|_| VoiceLabBuildSnapshot {
            active: true,
            generation: None,
            phase: "state_unavailable".to_string(),
            cancel_requested: true,
        })
}

pub fn voice_lab_build_blocks_meeting() -> bool {
    build_store()
        .lock()
        .map(|state| state.blocks_meeting())
        .unwrap_or(true)
}

pub fn begin_voice_lab_build() -> Result<VoiceLabBuildSnapshot, String> {
    if meeting_blocks_voice_lab() {
        return Err("voice_lab:meeting_active".to_string());
    }
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .begin()
}

pub fn mark_voice_lab_build_training(generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .transition(generation, BuildPhase::Preparing, BuildPhase::Training)
}

pub fn mark_voice_lab_build_evaluating(generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .transition(generation, BuildPhase::Training, BuildPhase::Evaluating)
}

pub fn request_voice_lab_build_cancel(generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .cancel(generation)
}

pub fn finish_voice_lab_build(generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .finish(generation)
}

pub fn fail_voice_lab_build(generation: u64) -> Result<VoiceLabBuildSnapshot, String> {
    build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?
        .fail(generation)
}

pub fn prepare_guided_dataset(
    paths: &ProjectPaths,
    generation: u64,
    manifest: &GuidedDatasetManifest,
) -> Result<PathBuf, String> {
    ensure_build_generation(generation, BuildPhase::Preparing)?;
    prepare_guided_dataset_at(&VoiceLabStoragePaths::from_project_paths(paths), manifest)
}

pub fn promote_voice_actor_candidate(paths: &ProjectPaths) -> Result<(), String> {
    if meeting_blocks_voice_lab() {
        return Err("voice_lab:meeting_active".to_string());
    }
    if voice_lab_build_blocks_meeting() {
        return Err("voice_lab:build_active".to_string());
    }
    promote_voice_actor_candidate_at(&VoiceLabStoragePaths::from_project_paths(paths))
}

pub fn approved_voice_actor_ready(paths: &ProjectPaths) -> bool {
    validate_actor_package(&VoiceLabStoragePaths::from_project_paths(paths).approved_actor_dir).is_ok()
}

fn ensure_build_generation(generation: u64, phase: BuildPhase) -> Result<(), String> {
    let guard = build_store()
        .lock()
        .map_err(|_| "voice_lab:build_state_unavailable".to_string())?;
    let Some(active) = guard.active.as_ref() else {
        return Err("voice_lab:no_active_build".to_string());
    };
    if active.generation != generation {
        return Err("voice_lab:stale_build_generation".to_string());
    }
    if active.phase != phase || active.cancel_requested {
        return Err("voice_lab:invalid_build_transition".to_string());
    }
    Ok(())
}



















#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit_voicelab_{label}_{now}"))
    }

    fn write_wav(path: &Path, duration_ms: u64) {
        let samples = u64::from(CANONICAL_SAMPLE_RATE) * duration_ms / 1_000;
        let data_size = samples.saturating_mul(2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_size as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36u32.saturating_add(data_size)).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&CANONICAL_CHANNELS.to_le_bytes());
        bytes.extend_from_slice(&CANONICAL_SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&(CANONICAL_SAMPLE_RATE * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&CANONICAL_BITS_PER_SAMPLE.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize(44 + data_size as usize, 0);
        fs::create_dir_all(path.parent().expect("wav parent")).expect("create wav parent");
        fs::write(path, bytes).expect("write wav");
    }

    fn dataset() -> GuidedDatasetManifest {
        GuidedDatasetManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            authorized_voice_confirmed: true,
            takes: vec![GuidedTakeContract {
                line_id: 1,
                exact_text: "Tomorrow we will review the project timeline.".to_string(),
                wav_file: canonical_take_file_name(1),
            }],
            held_out_lines: vec![GuidedEvaluationLineContract {
                line_id: 101,
                exact_text: "The meeting begins after everyone is ready.".to_string(),
            }],
        }
    }

    fn actor_manifest(duration_ms: u64) -> VoiceActorPackageManifest {
        VoiceActorPackageManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            engine: VOICE_ACTOR_ENGINE.to_string(),
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.to_string(),
            gpt_weight_file: GPT_WEIGHT_FILE.to_string(),
            sovits_weight_file: SOVITS_WEIGHT_FILE.to_string(),
            reference_wav_file: REFERENCE_WAV_FILE.to_string(),
            reference_text: "Tomorrow we will review the project timeline.".to_string(),
            reference_duration_ms: duration_ms,
            held_out_evaluation_complete: true,
        }
    }

    fn write_actor(dir: &Path, marker: &[u8]) {
        fs::create_dir_all(dir).expect("create actor");
        fs::write(dir.join(GPT_WEIGHT_FILE), marker).expect("gpt weight");
        fs::write(dir.join(SOVITS_WEIGHT_FILE), marker).expect("sovits weight");
        write_wav(&dir.join(REFERENCE_WAV_FILE), 4_000);
        write_json(&dir.join(ACTOR_MANIFEST_FILE), &actor_manifest(4_000)).expect("manifest");
    }

    #[test]
    fn lifecycle_is_generation_bound_and_cancel_does_not_fake_completion() {
        let mut lifecycle = BuildLifecycle::default();
        let start = lifecycle.begin().expect("start");
        let generation = start.generation.expect("generation");
        assert!(lifecycle.blocks_meeting());
        assert_eq!(lifecycle.finish(generation), Err("voice_lab:invalid_build_transition".to_string()));
        lifecycle
            .transition(generation, BuildPhase::Preparing, BuildPhase::Training)
            .expect("training");
        let cancelling = lifecycle.cancel(generation).expect("cancel");
        assert!(cancelling.cancel_requested);
        assert_eq!(cancelling.phase, "cancelling");
        lifecycle.finish(generation).expect("cancel complete");
        assert!(!lifecycle.blocks_meeting());
    }

    #[test]
    fn dataset_requires_authorization_and_true_held_out_lines() {
        let mut manifest = dataset();
        manifest.authorized_voice_confirmed = false;
        assert_eq!(
            validate_guided_dataset_manifest(&manifest),
            Err("voice_lab:voice_authorization_required".to_string())
        );
        let mut manifest = dataset();
        manifest.held_out_lines[0].exact_text = manifest.takes[0].exact_text.clone();
        assert_eq!(
            validate_guided_dataset_manifest(&manifest),
            Err("voice_lab:held_out_line_used_for_training".to_string())
        );
    }

    #[test]
    fn accepted_dataset_is_frozen_from_canonical_guided_wavs_only() {
        let root = test_root("dataset");
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        write_wav(&storage.takes_dir.join(canonical_take_file_name(1)), 1_200);
        let path = prepare_guided_dataset_at(&storage, &dataset()).expect("prepare dataset");
        assert!(path.is_file());
        assert!(storage.build_dataset_dir.join(canonical_take_file_name(1)).is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn actor_contract_requires_native_weights_reference_and_evaluation() {
        let root = test_root("actor");
        write_actor(&root, b"weight");
        assert!(validate_actor_package(&root).is_ok());
        let mut manifest = actor_manifest(4_000);
        manifest.held_out_evaluation_complete = false;
        write_json(&root.join(ACTOR_MANIFEST_FILE), &manifest).expect("rewrite manifest");
        assert_eq!(
            validate_actor_package(&root),
            Err("voice_lab:actor_evaluation_incomplete".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_rebuild_candidate_never_replaces_current_actor() {
        let root = test_root("promotion");
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        write_actor(&storage.approved_actor_dir, b"old");
        write_actor(&storage.candidate_actor_dir, b"new");
        promote_voice_actor_candidate_at(&storage).expect("first promotion");
        assert_eq!(
            fs::read(storage.approved_actor_dir.join(GPT_WEIGHT_FILE)).expect("approved"),
            b"new"
        );
        fs::remove_file(storage.candidate_actor_dir.join(SOVITS_WEIGHT_FILE)).expect("invalidate");
        assert!(promote_voice_actor_candidate_at(&storage).is_err());
        assert_eq!(
            fs::read(storage.approved_actor_dir.join(GPT_WEIGHT_FILE)).expect("preserved"),
            b"new"
        );
        let _ = fs::remove_dir_all(root);
    }
}

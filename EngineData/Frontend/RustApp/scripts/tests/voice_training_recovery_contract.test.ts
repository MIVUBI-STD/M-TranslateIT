import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (path: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + path), "utf8");
const worker = (path: string) => readFileSync(resolve(root,
  "../../Backend/LocalWorker/WorkerRuntime/" + path), "utf8");

test("recoverable checkpoint capture is pinned to exact unchanged GPT-SoVITS source and dataset", () => {
  const recovery = rust("voice_lab/storage/training_recovery.rs");
  const engine = worker("voice_lab_gpt_sovits.py");
  const pipeline = worker("voice_lab_gpt_sovits_build.py");
  assert.match(engine, /ENGINE_REVISION = "d523079fc05d9a8028d6085bffe4a2757c32abb6"/);
  assert.match(recovery, /VOICE_ACTOR_ENGINE_REVISION/);
  assert.match(recovery, /frozen_dataset_snapshot_id\(storage\)/);
  assert.match(recovery, /checkpoint_preserved_not_verified_for_resume/);
  assert.match(recovery, /sovits-G/);
  assert.match(recovery, /sovits-D/);
  assert.match(recovery, /gpt-config\.yaml/);
  assert.match(recovery, /sovits-config\.json/);
  assert.match(pipeline, /"if_save_latest": True/);
  assert.match(pipeline, /run_stage\(source_root, assets\["sovits_train"\]/);
  assert.match(pipeline, /run_stage\(source_root, assets\["gpt_train"\]/);
});

test("interrupted checkpoint capture blocks destructive old-work cleanup on failure", () => {
  const build = rust("voice_lab_build.rs");
  const recovery = rust("voice_lab/storage/training_recovery.rs");
  const start = build.indexOf("fn start_voice_lab_build_selected(");
  const prepare = build.indexOf("prepare_guided_dataset(", start);
  const capture = build.indexOf("preserve_training_checkpoints(&paths)", start);
  assert.ok(start > 0 && capture > start && capture < prepare);
  assert.match(build, /prior_training_recovery_required/);
  assert.match(build, /let recovery = if ready/);
  assert.match(build, /if recovery\.is_err\(\)/);
  assert.match(recovery, /create_dir\(&pending\)/);
  assert.match(recovery, /verify_package\(&pending/);
  assert.match(recovery, /fs::rename\(&pending, &complete\)/);
  assert.match(recovery, /recovery_checkpoint_changed_during_copy/);
  assert.match(recovery, /incomplete_training_checkpoint_set_is_preserved_for_investigation_not_resumed/);
  const production = recovery.split("#[cfg(test)]")[0];
  assert.doesNotMatch(production, /remove_file\(|remove_dir_all\(/);
});

test("backup includes checkpoint recovery objects but never converts a candidate into a resume action", () => {
  const backup = rust("voice_lab/storage/backup.rs");
  const registry = rust("registry.rs");
  const recording = rust("voice_lab_recording.rs");
  const ui = readFileSync(resolve(root, "src/components/my-voice/VoiceBackupRestore.svelte"), "utf8");
  const api = readFileSync(resolve(root, "src/app/bridge/myVoiceApi.ts"), "utf8");
  assert.match(backup, /"TrainingRecovery"/);
  assert.match(backup, /valid_recovery_file/);
  assert.match(backup, /recovery_receipt_identity_invalid/);
  assert.match(backup, /recovery_dataset_missing/);
  assert.match(backup, /recovery_file_changed/);
  assert.match(registry, /get_voice_lab_training_recovery_status/);
  assert.match(recording, /pub fn get_voice_lab_training_recovery_status/);
  assert.match(api, /resume_available: boolean/);
  assert.match(ui, /automatic Resume is not yet available/);
  assert.match(ui, /preserved_checkpoint_candidates/);
  assert.doesNotMatch(ui, /onclick=.*resumeVoiceTraining/i);
});

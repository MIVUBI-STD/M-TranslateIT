import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const worker = (file: string) => readFileSync(
  resolve(root, "../../Backend/LocalWorker/WorkerRuntime/" + file), "utf8"
);

test("GPT opt-in probe remains separate from ordinary My Voice training", () => {
  const runner = worker("voice_lab_upstream_stage.py");
  const normal = worker("voice_lab_gpt_sovits_build.py");
  const gpt = worker("voice_lab_resume_gpt_probe.py");
  assert.match(runner, /--translateit-dev-strict-gpt-probe/);
  assert.match(runner, /relative_script\.as_posix\(\) != "GPT_SoVITS\/s1_train\.py"/);
  assert.match(runner, /install_opt_in_gpt_lightning_probe\(source_root, Path\(script_args\[1\]\)\)/);
  assert.match(runner, /script_args = \["--config_file", plan\.config_path\]/);
  assert.doesNotMatch(normal, /--translateit-dev-strict-gpt-probe/);
  assert.match(gpt, /_engine_source\(source_root\)/);
  assert.match(gpt, /pytorch_lightning/);
  assert.match(gpt, /pl\.Trainer = StrictTrainer/);
  assert.match(gpt, /torch\.cuda\.device_count\(\) != 1/);
});

test("GPT load acknowledgement requires the actual Lightning optimizer and scheduler state", () => {
  const gpt = worker("voice_lab_resume_gpt_probe.py");
  assert.match(gpt, /checkpoint_or_config_identity_mismatch/);
  assert.match(gpt, /gpt_explicit_checkpoint_required/);
  assert.match(gpt, /weights_only=True/);
  assert.doesNotMatch(gpt, /torch\.load\([^)]*weights_only=False/);
  assert.match(gpt, /def restored\(self, trainer: Any, model: Any, saved:/);
  assert.match(gpt, /gpt_model_state_not_restored/);
  assert.match(gpt, /gpt_optimizer_state_not_restored/);
  assert.match(gpt, /gpt_scheduler_state_not_restored/);
  assert.match(gpt, /checkpoint_loaded/);
  assert.match(gpt, /gpt_first_optimizer_step_did_not_advance_global_step/);
  assert.match(gpt, /after_optimizer_step/);
  assert.match(gpt, /on_train_batch_end/);
  assert.match(gpt, /trainer\.global_step|global_step/);
  assert.match(gpt, /gpt_checkpoint_training_never_advanced/);
});

test("GPT worker event evidence cannot enable the end-user Resume feature", () => {
  const parser = worker("voice_lab_strict_resume_ack.py");
  const gpt = worker("voice_lab_resume_gpt_probe.py");
  const normal = worker("voice_lab_gpt_sovits_build.py");
  assert.match(parser, /"worker_provenance_verified": False/);
  assert.match(parser, /"trainer_load_verified": False/);
  assert.match(parser, /"resume_available": False/);
  assert.match(gpt, /NOT permission to enable the user-facing Resume action/);
  assert.doesNotMatch(normal, /voice_lab_resume_gpt_probe|voice_lab_resume_child_probe/);
});

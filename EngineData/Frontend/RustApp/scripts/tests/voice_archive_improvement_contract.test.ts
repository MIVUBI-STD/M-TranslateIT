import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (p: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + p), "utf8");
const ui = (p: string) => readFileSync(resolve(root, "src/" + p), "utf8");

test("archive improvement is a separate user-authorized intent on the original GPT-SoVITS build owner", () => {
  const build = rust("voice_lab_build.rs");
  const source = rust("voice_lab/storage/archive_dataset.rs");
  const worker = rust("voice_lab/storage/dataset_history.rs");
  const recording = rust("voice_lab_recording.rs");
  const registry = rust("registry.rs");
  const wrapper = rust("application_runtime/mutations.rs");
  assert.match(registry, /start_product_voice_improvement/);
  assert.match(wrapper, /emit_after\(&app, "start_voice_improvement"/);
  assert.match(build, /pub fn start_voice_lab_improvement/);
  assert.match(build, /start_voice_lab_build_selected\(authorized_voice_confirmed, Some\(archive_ids\)\)/);
  assert.match(build, /missing_training_coverage_group\(&selection.takes\)/);
  assert.match(build, /selection.duration_ms < MIN_TRAINING_SPEECH_MS/);
  assert.match(build, /selection.reference_ready/);
  assert.match(build, /prepare_guided_dataset_from_archives\(/);
  assert.match(build, /reviewable_evaluation\(&paths\)/);
  assert.match(source, /const MAX_SELECTED_TAKES: usize = 128/);
  assert.match(source, /improvement_one_recording_per_line/);
  assert.match(source, /improvement_transcript_mismatch/);
  assert.match(recording, /fn guided_training_prompts/);
  assert.match(worker, /preserve_frozen_dataset_with_archives/);
});

test("existing held-out approval follows exact frozen immutable audio for archive rebuilds", () => {
  const evaluation = rust("voice_lab_build/evaluation.rs");
  const snapshot = rust("voice_lab/storage/dataset_history.rs");
  const archive = rust("voice_lab/storage/recording_archive.rs");
  assert.match(evaluation, /archived_frozen_review_matches\(paths\)/);
  assert.match(evaluation, /frozen_dataset_matches_current/);
  assert.match(snapshot, /from_saved_archive: bool/);
  assert.match(snapshot, /snapshot_selected_take_changed/);
  assert.match(snapshot, /staged != verified/);
  assert.match(snapshot, /verify_saved_take_binding/);
  assert.match(archive, /archived_take_for_training/);
  assert.match(snapshot, /archived_frozen_review_matches/);
  assert.match(evaluation, /candidate_selection_matches_actor_json/);
});

test("UI selects one saved version of each sentence, and uncertain builds never claim success", () => {
  const page = ui("pages/MyVoice.svelte");
  const api = ui("app/bridge/myVoiceBuildApi.ts");
  assert.match(page, /improvementTakes\[take.line_id\] === take.archive_id/);
  assert.match(page, /Create improved version/);
  assert.match(page, /I own these recordings or have permission/);
  assert.match(page, /improvementIds.length > 128/);
  assert.match(page, /buildSnapshot\?\.evaluation_ready/);
  assert.match(page, /buildStatusUnavailable = true/);
  assert.match(api, /start_product_voice_improvement/);
  assert.match(api, /Couldn't verify whether My Voice improvement started/);
});

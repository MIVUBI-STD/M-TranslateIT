import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (p: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + p), "utf8");

test("worker dataset remains unchanged while all training takes have durable lineage", () => {
  const storage = rust("voice_lab/storage.rs");
  const dataset = rust("voice_lab/storage/dataset_history.rs");
  const archive = rust("voice_lab/storage/recording_archive.rs");
  assert.match(storage, /dataset_history::preserve_frozen_dataset\(storage, manifest\)\?/);
  assert.match(dataset, /archive_guided_take\(/);
  assert.match(dataset, /verify_saved_take_binding\(/);
  assert.match(dataset, /persist_manifest\(/);
  assert.match(dataset, /frozen_dataset_snapshot_id/);
  assert.match(archive, /receipt\.line_id != line_id \|\| receipt\.exact_text != exact_text/);
  assert.match(storage, /write_json\(&manifest_path, manifest\)\?/);
});

test("actor replacement preserves active, stored and approved candidate package", () => {
  const storage = rust("voice_lab/storage.rs");
  const history = rust("voice_lab/storage/actor_history.rs");
  const promote = storage.indexOf("pub(super) fn promote_voice_actor_candidate_at");
  const stage = storage.indexOf("let staging =", promote);
  assert.ok(promote !== -1 && stage > promote);
  const prepromotion = storage.slice(promote, stage);
  assert.match(prepromotion, /preserve_actor_version\(storage, &storage\.approved_actor_dir\)/);
  assert.match(prepromotion, /preserve_actor_version\(storage, &storage\.candidate_actor_dir\)/);
  assert.match(prepromotion, /preserve_training_lineage/);
  assert.match(storage, /preserve_actor_version\(paths, current\)/);
  assert.match(storage, /preserve_actor_version\(paths, &paths\.approved_actor_dir\)/);
  assert.match(history, /hash_regular_file/);
  assert.match(history, /create_dir\(&pending\)/);
  assert.match(history, /verify_version\(&saved/);
  assert.match(history, /different_approved_actors_remain_individually_recoverable/);
});

test("new durable history is independent of active engine and no restore is advertised yet", () => {
  const rootCargo = readFileSync(resolve(root, "src-tauri/Cargo.toml"), "utf8");
  const lock = readFileSync(resolve(root, "src-tauri/Cargo.lock"), "utf8");
  const versions = rust("voice_lab/storage/actor_history.rs");
  const data = rust("voice_lab/storage/dataset_history.rs");
  assert.match(rootCargo, /sha2 = "=0\.10\.9"/);
  assert.match(lock, /name = "sha2"\nversion = "0\.10\.9"/);
  assert.match(versions, /NOT an optimizer training checkpoint/);
  assert.match(versions, /No deletion, no activation/);
  assert.match(data, /Existing|worker continues reading the exact existing dataset/);
});

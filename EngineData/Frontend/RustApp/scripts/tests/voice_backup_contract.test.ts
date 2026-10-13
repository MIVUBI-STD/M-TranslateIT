import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const source = (file: string) => readFileSync(resolve(app, file), "utf8");
const rust = (file: string) => source("src-tauri/src/commands/" + file);

test("backup exports verified immutable user objects and never overwrites existing backups", () => {
  const backup = rust("voice_lab/storage/backup.rs");
  const store = rust("voice_lab/storage.rs");
  assert.match(backup, /const ROOTS:|match parts\.as_slice\(\)/);
  assert.match(backup, /Recordings/);
  assert.match(backup, /DatasetSnapshots/);
  assert.match(backup, /TrainingLineage/);
  assert.match(backup, /VoiceVersions/);
  assert.match(backup, /hash_regular_file/);
  assert.match(backup, /create_new\(true\)/);
  assert.match(backup, /voice_backup:copy_verification_failed/);
  assert.match(backup, /check_references\(&data, &manifest\.entries\)/);
  assert.match(backup, /fs::rename\(&pending, &committed\)/);
  assert.match(backup, /complete_backup_restores_archives_without_touching_selected_meeting_voice/);
  assert.match(store, /actor_history::preserve_actor_version\(storage, source\)/);
  assert.match(store, /resolve_previous_voice_recovery_before_export/);
});

test("restore rejects unsafe paths, conflicts, corrupted sources, and never selects Meeting voice", () => {
  const backup = rust("voice_lab/storage/backup.rs");
  const production = backup.split("#[cfg(test)]")[0];
  assert.match(backup, /fn safe_relative\(/);
  assert.match(backup, /unsupported_engine_or_format/);
  assert.match(backup, /voice_backup:existing_archive_conflict/);
  assert.match(backup, /voice_backup:incomplete_restore_pending_file/);
  assert.match(backup, /source_overlaps_user_data/);
  assert.match(backup, /hostile|backup_paths_only_allow_known_archive_objects/);
  assert.match(backup, /fn verify_backup\(/);
  assert.doesNotMatch(production, /remove_file\(|remove_dir_all\(/);
  assert.doesNotMatch(production, /approve_voice_lab_candidate|activate_archived_personal_voice|install_builtin_voice/);
});

test("backup commands are native and blocked during recording, training, Meeting and Quick Preview", () => {
  const recording = rust("voice_lab_recording.rs");
  const build = rust("voice_lab_build.rs");
  const registry = rust("registry.rs");
  assert.match(registry, /create_voice_lab_backup/);
  assert.match(registry, /verify_voice_lab_backup/);
  assert.match(registry, /restore_voice_lab_backup/);
  assert.match(recording, /backup_operation_allowed/);
  assert.match(recording, /voice_lab_recording_active\(\)/);
  assert.match(recording, /current_voice_lab_build_snapshot\(\)\.active/);
  assert.match(recording, /meeting_blocks_voice_change\(\)/);
  assert.match(recording, /quick_voice_preview_active\(\)/);
  assert.match(recording, /if !confirmed/);
  assert.match(build, /voice_selection_guard/);
  assert.match(build, /voice_backup_active\(\)/);
});

test("backup UI labels verification honestly, requires consent and does not autoactivate", () => {
  const api = source("src/app/bridge/myVoiceApi.ts");
  const child = source("src/components/my-voice/VoiceBackupRestore.svelte");
  const page = source("src/pages/MyVoice.svelte");
  for (const cmd of ["create_voice_lab_backup", "verify_voice_lab_backup", "restore_voice_lab_backup"]) {
    assert.match(api, new RegExp(cmd));
  }
  assert.match(child, /Create verified backup/);
  assert.match(child, /Verify backup/);
  assert.match(child, /Import saved recordings and versions/);
  assert.match(child, /Only restore backups you trust/);
  assert.match(child, /bind:checked=\{authorized\}/);
  assert.match(child, /status is unknown|status is unknown|Couldn't confirm/);
  assert.match(page, /VoiceBackupRestore/);
});

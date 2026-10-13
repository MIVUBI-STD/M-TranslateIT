import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (p: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + p), "utf8");
const frontend = (p: string) => readFileSync(resolve(root, "src/" + p), "utf8");

test("legacy migration uses an explicit non-destructive copy-only native operation", () => {
  const registry = rust("registry.rs");
  const source = rust("voice_lab/storage/recording_migration.rs");
  const production = source.split("#[cfg(all(test, target_os")[0];
  const recording = rust("voice_lab_recording.rs");
  assert.match(registry, /get_voice_lab_legacy_archive_status/);
  assert.match(registry, /protect_voice_lab_legacy_recordings/);
  assert.match(recording, /current_voice_lab_build_snapshot\(\)\.active/);
  assert.match(recording, /draft_store\(\)\.lock/);
  assert.match(production, /archive_guided_take\(/);
  assert.doesNotMatch(production, /remove_file\(|remove_dir_all\(|fs::rename\(/);
  assert.match(production, /wav\.previous/);
  assert.match(recording, /previous take backup needs protection/);
});

test("legacy protection UI shows uncertainty instead of claiming all files are safe", () => {
  const page = frontend("pages/MyVoice.svelte");
  const api = frontend("app/bridge/myVoiceApi.ts");
  assert.match(api, /LegacyProtectionStatus \| null/);
  assert.match(page, /legacyProtectionUnknown = true/);
  assert.match(page, /Protect earlier recordings/);
  assert.match(page, /Original files stay on this computer/);
  assert.match(page, /Save protected copies/);
});


test("saved take library lists verified receipts and requests audio by archive ID only", () => {
  const archive = rust("voice_lab/storage/recording_archive.rs");
  const registry = rust("registry.rs");
  const api = frontend("app/bridge/myVoiceApi.ts");
  const page = frontend("pages/MyVoice.svelte");
  assert.match(registry, /list_voice_lab_saved_recordings/);
  assert.match(registry, /get_voice_lab_saved_recording_audio/);
  assert.match(archive, /receipt_for_archive/);
  assert.match(archive, /valid_archive_id/);
  assert.match(archive, /verified_archive\(&dir, &receipt\)/);
  assert.match(archive, /LIBRARY_PAGE_SIZE: usize = 25/);
  assert.match(api, /archiveId/);
  assert.match(page, /Saved recordings/);
  assert.match(page, /Show more recordings/);
  assert.match(page, /savedLibraryError = true/);
});

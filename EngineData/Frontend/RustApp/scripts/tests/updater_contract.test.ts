import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(testDir, "../..");

const read = (path: string) => readFileSync(join(appRoot, path), "utf8");

test("updater remains one-shot and has no polling scheduler", () => {
  const api = read("src/app/update/appUpdateApi.ts");
  const action = read("src/components/runtime/UpdateAction.svelte");

  assert.match(api, /let startupCheckStarted = false;/);
  assert.match(api, /if \(startupCheckStarted\) return null;/);
  assert.match(api, /startupCheckStarted = true;/);
  assert.match(action, /checkAtStartupOnce\(\)/);

  for (const source of [api, action]) {
    assert.doesNotMatch(source, /setInterval\s*\(/);
    assert.doesNotMatch(source, /setTimeout\s*\(/);
    assert.doesNotMatch(source, /requestAnimationFrame\s*\(/);
  }
});

test("native updater is signed, GitHub-release based, and activity-safe", () => {
  const source = read("src-tauri/src/commands/app_update.rs");
  const header = read("src/components/layout/AppHeader.svelte");
  const productivity = read("src/components/runtime/ProductivityActions.svelte");
  const action = read("src/components/runtime/UpdateAction.svelte");

  assert.match(source, /option_env!\("TRANSLATEIT_UPDATER_PUBLIC_KEY"\)/);
  assert.match(source, /releases\/latest\/download\/latest\.json/);
  assert.match(source, /voice_lab_recording_blocks_app_exit/);
  assert.match(source, /current_voice_lab_build_snapshot\(\)\.active/);
  assert.match(source, /runtime\.has_active_session/);
  assert.match(header, /audioLocked=\{snapshot\.resources\.audio_locked\}/);
  assert.match(header, /audioOwnerKind=\{snapshot\.resources\.owner_kind\}/);
  assert.match(header, /voiceBuildActive=\{snapshot\.voiceBuildActive\}/);
  assert.match(productivity, /audioOwnerKind/);
  assert.match(productivity, /<UpdateAction \{audioLocked\} \{audioOwnerKind\} \{voiceBuildActive\} \{myVoiceRecording\}/);
  assert.match(action, /audioLocked/);
  assert.match(action, /audioOwnerKind/);
  assert.match(action, /voiceBuildActive/);
  assert.match(action, /myVoiceRecording/);
  assert.match(action, /Stop Translation before updating TranslateIT/);
  assert.match(action, /Stop Mic Test before updating TranslateIT/);
  assert.match(action, /Stop the current My Voice recording before updating TranslateIT/);
  assert.match(action, /Wait for My Voice creation to finish before updating TranslateIT/);
  assert.ok((source.match(/update_blocker\(\)/g) ?? []).length >= 2);
  assert.ok(source.lastIndexOf("update_blocker()") < source.indexOf("download_and_install"));
  assert.match(source, /download_and_install/);

  assert.doesNotMatch(source, /loop\s*\{/);
  assert.doesNotMatch(source, /std::thread::spawn/);
});

test("release build fails closed without updater signing material", () => {
  const release = read("scripts/build_release.ps1");
  const releaseConfig = JSON.parse(read("src-tauri/tauri.release.conf.json"));

  assert.match(release, /TRANSLATEIT_UPDATER_PUBLIC_KEY/);
  assert.match(release, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(release, /TAURI_SIGNING_PRIVATE_KEY_PATH/);
  assert.equal(releaseConfig.bundle?.createUpdaterArtifacts, true);
});


test("external runtime payload is preserved only when installed runtime is compatible", () => {
  const hook = read("src-tauri/windows/r3_payload_hooks.template.nsh");
  const helper = read("src-tauri/windows/r3_payload_installer.ps1");

  assert.match(hook, /GetOptions.*"\/UPDATE"/);
  assert.match(hook, /-Mode VerifyInstalled/);
  assert.match(hook, /preserving the verified installed runtime payload/);
  assert.match(helper, /ValidateSet\('Verify','Install','VerifyInstalled'\)/);
  assert.match(helper, /function Verify-InstalledRuntime/);
  assert.match(helper, /Assert-InstalledRuntimeManifest/);
  assert.match(helper, /translation_revision/);
  assert.match(helper, /asr_revision/);
  assert.match(helper, /voice_revision/);
});

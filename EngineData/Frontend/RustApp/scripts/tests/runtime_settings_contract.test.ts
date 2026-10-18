import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = readFileSync(
  new URL("../../src-tauri/src/engine/settings.rs", import.meta.url),
  "utf8",
);
const types = readFileSync(
  new URL("../../src/app/shared/types.ts", import.meta.url),
  "utf8",
);
const state = readFileSync(
  new URL("../../src/app/shared/state.ts", import.meta.url),
  "utf8",
);

function rustRuntimeSettingsFields(): string[] {
  const match = rust.match(/pub struct RuntimeSettings \{([\s\S]*?)\n\}/);
  assert.ok(match, "Rust RuntimeSettings struct must exist");
  return [...match[1].matchAll(/pub\s+([a-z_][a-z_0-9]*)\s*:/g)]
    .map((item) => item[1])
    .sort();
}

function tsRuntimeSettingsFields(): string[] {
  const match = types.match(/export type RuntimeSettings = \{([\s\S]*?)\n\};/);
  assert.ok(match, "TypeScript RuntimeSettings type must exist");
  return [...match[1].matchAll(/^\s{2}([a-z_][a-z_0-9]*)\s*:/gm)]
    .map((item) => item[1])
    .sort();
}

test("RuntimeSettings top-level fields stay aligned across Rust and TypeScript", () => {
  assert.deepEqual(tsRuntimeSettingsFields(), rustRuntimeSettingsFields());
});

test("RuntimeSettings schema version stays aligned with frontend defaults", () => {
  const rustVersion = rust.match(/const CURRENT_SCHEMA_VERSION: u32 = (\d+);/);
  const frontendVersion = state.match(/schema_version:\s*(\d+)/);
  assert.ok(rustVersion);
  assert.ok(frontendVersion);
  assert.equal(frontendVersion[1], rustVersion[1]);
});

test("core RuntimeSettings defaults remain aligned", () => {
  for (const [field, value] of [
    ["source_language", "id"],
    ["target_language", "en"],
    ["translation_style", "natural"],
    ["meeting_listen_source_language", "en"],
    ["meeting_listen_target_language", "id"],
    ["meeting_setup_state", "new"],
  ]) {
    assert.match(state, new RegExp(`${field}: "${value}"`));
  }
  assert.match(state, /meeting_setup_checkpoint:\s*1/);
  assert.match(state, /noise_suppression:\s*"auto"/);
  assert.match(state, /input_device_id:\s*null/);
  assert.match(state, /output_device_id:\s*null/);
});

test("device preference mutations distinguish native save from an uncertain display refresh", () => {
  const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  const audio = readFileSync(new URL("../../src/app/bridge/productAudioFacade.ts", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/settings.rs", import.meta.url), "utf8");
  const change = settings.slice(settings.indexOf("async function changeDevice("),
    settings.indexOf("async function runSetupRepair("));
  assert.match(change, /let savedByNative = false/);
  assert.match(change, /if \(!result\.ok\) return;\s*savedByNative = true;/);
  assert.match(change, /deviceMessage = savedByNative/);
  assert.match(change, /Device preference was saved, but the display could not be refreshed/);
  assert.match(change, /Could not verify whether the device preference changed/);
  assert.doesNotMatch(change, /The device wasn't changed\. Try again/);
  assert.match(audio, /if \(!result\) \{/);
  assert.match(audio, /Audio settings could not be verified/);
  assert.doesNotMatch(audio, /The previous device preference was kept/);
  assert.match(native, /if !save\.ok \{/);
  assert.match(native, /let saved = engine::load_settings\(\)/);
});

test("Settings and ApplicationRuntime block device rebind during no-session Meeting Retry Stop", () => {
  const settings = readFileSync(new URL("../../src-tauri/src/commands/settings.rs", import.meta.url), "utf8");
  const meeting = readFileSync(new URL("../../src-tauri/src/commands/meeting_session.rs", import.meta.url), "utf8");
  const resources = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/resources.rs", import.meta.url), "utf8");
  const ui = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  assert.match(meeting, /fn meeting_stop_cleanup_pending\(\) -> bool/);
  assert.match(meeting, /lifecycle::transient_stop_cleanup_pending\(\)/);
  assert.match(settings, /fn runtime_session_owns_audio_resources\(\) -> bool \{[\s\S]*super::meeting_session::meeting_stop_cleanup_pending\(\)/);
  assert.match(settings, /if runtime_session_owns_audio_resources\(\) \{/);
  assert.match(resources, /let orphan_cleanup = !meeting\.has_session && meeting\.lifecycle == "cleanup_incomplete"/);
  assert.match(resources, /let audio_locked = meeting\.has_session \|\| orphan_cleanup/);
  assert.match(resources, /resource:meeting_cleanup_incomplete/);
  assert.match(ui, /const meetingResourcesLocked = \$derived\(snapshot\.resources\.audio_locked\)/);
});

test("settings sanitizers keep product enum-like values bounded", () => {
  assert.match(rust, /"formal" => "formal"/);
  assert.match(rust, /"off" => "off"/);
  assert.match(rust, /"deferred" => "deferred"/);
  assert.match(rust, /"completed" => "completed"/);
});

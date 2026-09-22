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

test("settings sanitizers keep product enum-like values bounded", () => {
  assert.match(rust, /"formal" => "formal"/);
  assert.match(rust, /"off" => "off"/);
  assert.match(rust, /"deferred" => "deferred"/);
  assert.match(rust, /"completed" => "completed"/);
});

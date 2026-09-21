import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const guard = readFileSync(new URL("../../src-tauri/src/commands/device_loss_guard.rs", import.meta.url), "utf8");

test("device-loss guard never changes device selection or starts recovery automatically", () => {
  assert.doesNotMatch(guard, /save_runtime_settings|select_audio_device|start_live_capture_runtime|start_meeting_translation/);
  assert.match(guard, /will not silently switch/i);
});

test("required and optional audio losses remain semantically separate", () => {
  assert.match(guard, /required_device_lost/);
  assert.match(guard, /optional_device_lost/);
  assert.match(guard, /required outbound translation remains authoritative/);
});

test("explicit device selection requires exact continued presence while Windows Default remains flexible", () => {
  assert.match(guard, /settings\.audio\.input_device_id\.is_some\(\)/);
  assert.match(guard, /settings\.audio\.output_device_id\.is_some\(\)/);
  assert.match(guard, /selected_device_present/);
});

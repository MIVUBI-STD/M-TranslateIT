import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const diagnostics = readFileSync(new URL("../../src/components/settings/SettingsDiagnostics.svelte", import.meta.url), "utf8");
const runtimeApi = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
const audioCommand = readFileSync(new URL("../../src-tauri/src/commands/audio.rs", import.meta.url), "utf8");

test("audio quality guard reuses the live rolling buffer instead of opening capture", () => {
  assert.match(audioCommand, /live_audio_buffer_status\(\)/);
  assert.doesNotMatch(audioCommand, /build_input_stream/);
  assert.doesNotMatch(audioCommand, /stream\.play/);
});

test("audio quality guard exposes bounded user-facing states", () => {
  for (const state of ["good", "too_quiet", "clipping", "noisy", "no_signal", "unavailable"]) {
    assert.match(audioCommand, new RegExp(state));
  }
  assert.match(runtimeApi, /getAudioQuality/);
});

test("Settings Diagnostics reads audio quality on explicit refresh without starting capture", () => {
  assert.match(diagnostics, /runtimeApi\.getAudioQuality\(\)/);
  assert.match(diagnostics, /const revision = refreshRevision/);
  assert.match(diagnostics, /active && revision === refreshRevision/);
  assert.match(diagnostics, /return \(\) => \{ active = false; \}/);
  assert.match(diagnostics, /audioQuality = null/);
  assert.match(diagnostics, /audioQualityLoading \? "Checking\.\.\."/);
  assert.match(diagnostics, /Microphone quality/);
  assert.doesNotMatch(diagnostics, /startCapture\(|setInterval\(/);
  assert.doesNotMatch(meeting, /getAudioQuality\(/);
  assert.doesNotMatch(meeting, /startCapture\(/);
});

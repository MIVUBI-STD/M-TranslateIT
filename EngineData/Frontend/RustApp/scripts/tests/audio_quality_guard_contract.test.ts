import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const runtimeApi = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
const audioCommand = readFileSync(new URL("../../src-tauri/src/commands/audio.rs", import.meta.url), "utf8");

test("audio quality guard reuses the live rolling buffer instead of opening capture", () => {
  assert.match(audioCommand, /live_audio_buffer_status\(\)/);
  assert.doesNotMatch(audioCommand, /build_input_stream/);
  assert.doesNotMatch(audioCommand, /stream\.play/);
});

test("audio quality guard exposes bounded user-facing states", () => {
  for (const state of ["good", "too_quiet", "clipping", "noisy", "unavailable"]) {
    assert.match(audioCommand, new RegExp(state));
  }
  assert.match(runtimeApi, /getAudioQuality/);
});

test("meeting page polls only diagnostic status and never starts capture", () => {
  assert.match(meeting, /getAudioQuality\(\)/);
  assert.match(meeting, /2500/);
  assert.doesNotMatch(meeting, /startCapture\(/);
});

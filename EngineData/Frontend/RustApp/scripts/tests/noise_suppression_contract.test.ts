import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
const consumer = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/consumer_runtime.rs", import.meta.url), "utf8");
const writer = readFileSync(new URL("../../src-tauri/src/engine/audio/live_segment_writer.rs", import.meta.url), "utf8");

test("noise suppression exposes only auto and off", () => {
  assert.match(settings, /Auto · recommended/);
  assert.match(settings, />Off</);
  assert.doesNotMatch(settings, /Low|Medium|High|Aggressive/);
});

test("meeting consumers snapshot suppression once instead of rereading per utterance", () => {
  assert.match(consumer, /noise_suppression_enabled/);
  assert.match(consumer, /engine::load_settings\(\)\.audio\.noise_suppression != "off"/);
  assert.match(consumer, /write_finalized_outbound_utterance_wav_with_noise_suppression/);
  assert.match(consumer, /write_finalized_incoming_utterance_wav_with_noise_suppression/);
});

test("noise suppression happens after VAD and before ASR wav without a neural model", () => {
  assert.match(writer, /suppress_low_level_noise/);
  assert.match(writer, /write_pcm16_wav/);
  assert.doesNotMatch(writer, /onnx|torch|model|worker/);
});

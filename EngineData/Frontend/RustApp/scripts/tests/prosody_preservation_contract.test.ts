import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const utterance = readFileSync(new URL("../../src-tauri/src/engine/audio/finalized_utterance.rs", import.meta.url), "utf8");
const consumer = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/consumer_runtime.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const voice = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits.py", import.meta.url), "utf8");

test("prosody preservation promotes measured speech duration through the outbound path", () => {
  assert.match(utterance, /speech_duration_ms: u64/);
  assert.match(consumer, /utterance\.speech_duration_ms/);
  assert.match(outbound, /"source_speech_duration_ms": speech_duration_ms/);
  assert.match(outbound, /"source_text": transcript\.clone\(\)/);
});

test("prosody preservation uses native bounded speed factor without emotion guessing", () => {
  assert.match(voice, /speed_factor/);
  assert.match(voice, /max\(0\.90, min\(1\.10/);
  assert.doesNotMatch(voice, /emotion|sentiment|angry|happy|sad/i);
});

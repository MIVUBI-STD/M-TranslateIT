import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Source contracts only. Execution, CUDA and receiver latency require the
// exact-revision Windows meeting baseline, not these assertions.
const source = (relative: string) =>
  readFileSync(new URL(`../../${relative}`, import.meta.url), "utf8");

const scheduler = source("src-tauri/src/commands/helper_bridge_runtime/scheduler.rs");
const transport = source("src-tauri/src/commands/helper_bridge/transport.rs");
const outbound = source("src-tauri/src/commands/meeting_session/outbound_pipeline.rs");
const timing = source("src-tauri/src/commands/meeting_session/session_state.rs");
const output = source("src-tauri/src/engine/audio/meeting_output_runtime.rs");
const playback = source("src-tauri/src/commands/meeting_session/playback_runtime.rs");
const diagnostics = source("src-tauri/src/commands/diagnostic_support.rs");
const vad = source("src-tauri/src/engine/audio/vad.rs");
const actor = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits.py", import.meta.url), "utf8");

test("only one inference job is admitted; wait timing is measured by Rust scheduler", () => {
  assert.match(scheduler, /active: bool/);
  assert.match(scheduler, /if state\.active \{/);
  assert.match(scheduler, /pub fn scheduler_wait_ms\(&self\) -> u64/);
  assert.match(scheduler, /scheduler_wait_ms: wait_started\.elapsed\(\)/);
  assert.match(transport, /json!\(scheduler_wait_ms\)/);
  for (const key of ["asr_scheduler_wait_ms", "translation_scheduler_wait_ms", "tts_scheduler_wait_ms"]) {
    assert.match(outbound, new RegExp(key));
  }
});

test("audio-boundary latency estimates are labeled, numeric, and do not change VAD thresholds", () => {
  assert.match(timing, /estimated_end_to_first_audio_ms/);
  assert.match(timing, /estimated_onset_to_first_audio_ms/);
  assert.match(timing, /saturating_add\(timing\.metrics\.captured_speech_span_ms\)/);
  assert.match(vad, /minimum_speech_duration_ms: 140/);
  assert.match(vad, /partial_asr_enabled: true/);
});

test("CPAL build timing is isolated from worker processing; no persistent stream behavior was introduced", () => {
  assert.match(output, /let stream_build_started_at = Instant::now\(\)/);
  assert.match(output, /stream_build_ms: Some\(stream_build_ms\)/);
  assert.match(output, /fn stream_start_to_first_audio_ms\(/);
  assert.match(playback, /timing\.metrics\.output_stream_build_ms = route\.stream_build_ms/);
  assert.match(output, /drop\(stream\)/);
  assert.match(actor, /"streaming_mode": False/);
});

test("support bundle contains only numeric derived timing, no audio or transcript", () => {
  for (const field of ["estimated_speech_onset_to_first_audio", "asr_scheduler_wait",
    "translation_scheduler_wait", "tts_scheduler_wait", "output_stream_build"]) {
    assert.match(diagnostics, new RegExp(`"${field}": timing\\.`), field);
  }
  assert.doesNotMatch(diagnostics, /"source_text"\s*:|"translated_text"\s*:|"session_id"\s*:/);
  assert.doesNotMatch(diagnostics, /"finalized_unix_ms"\s*:|"first_playback_unix_ms"\s*:/);
});

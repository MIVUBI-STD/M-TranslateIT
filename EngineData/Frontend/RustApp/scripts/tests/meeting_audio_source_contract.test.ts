import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const read = (path: string) => readFileSync(resolve(projectRoot, path), "utf8");

test("Meeting source uses one native selector and existing audio owners", () => {
  const meeting = read("src/pages/Meeting.svelte");
  const bridge = read("src/app/bridge/runtimeApi.ts");
  const registry = read("src-tauri/src/commands/registry.rs");
  assert.match(meeting, /Original Microphone/);
  assert.match(meeting, /TranslateIT Audio/);
  assert.match(meeting, /selectMeetingAudioSource/);
  assert.match(bridge, /set_meeting_audio_source/);
  assert.match(registry, /meeting_session::set_meeting_audio_source/);
});

test("stale translation and playback are gated by source epoch", () => {
  const producer = read("src-tauri/src/commands/meeting_session/outbound_pipeline.rs");
  const player = read("src-tauri/src/commands/meeting_session/playback_runtime.rs");
  const native = read("src-tauri/src/engine/audio/meeting_output_runtime.rs");
  const capture = read("src-tauri/src/engine/audio/live_capture.rs");
  assert.match(producer, /translated_epoch_matches\(generation, source_epoch\)/);
  assert.match(player, /job\.source_epoch/);
  assert.match(native, /source_epoch\.is_some_and/);
  assert.match(native, /original_voice_pcm::translated_epoch_matches/);
  assert.match(capture, /original_voice_pcm::try_push_mono/);
});

test("unknown bridge state preserves a safe literal translated-mode fallback", () => {
  const fallback = read("src/app/runtime/meetingBridgeFallback.ts");
  assert.match(fallback, /audio_source: "translateit" as const/);
});

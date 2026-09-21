import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const panel = readFileSync(new URL("../../src/components/meeting/MeetingPresetPanel.svelte", import.meta.url), "utf8");
const settings = readFileSync(new URL("../../src-tauri/src/commands/settings.rs", import.meta.url), "utf8");
const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const cache = readFileSync(new URL("../../src/app/runtime/textTranslationCache.ts", import.meta.url), "utf8");
const sessionReview = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");
const feedbackReview = readFileSync(new URL("../../src/components/settings/TranslationFeedbackReview.svelte", import.meta.url), "utf8");

test("Meeting presets apply through one Rust-owned validation transaction", () => {
  assert.match(registry, /apply_meeting_preset/);
  assert.match(panel, /runtimeApi\.applyMeetingPreset/);
  assert.doesNotMatch(panel, /probeInputDeviceCandidate|probeOutputDeviceCandidate|saveSettings\(/);
  assert.match(settings, /pub fn apply_meeting_preset/);
  assert.match(settings, /runtime_session_owns_audio_resources/);
  assert.match(settings, /probe_input_device_candidate/);
  assert.match(settings, /probe_output_device_candidate/);
  assert.match(settings, /persist_runtime_settings\(candidate\)/);
});

test("Text cache stays process-local rather than becoming persistent history", () => {
  assert.match(cache, /MAX_ENTRIES = 32/);
  assert.doesNotMatch(cache, /localStorage|sessionStorage|indexedDB|fs\./i);
});

test("session review and feedback remain explicit bounded workflows", () => {
  assert.match(sessionReview, /getRecentMeetingTranscript/);
  assert.match(sessionReview, /temporary until a new Meeting starts or the app closes/);
  assert.match(feedbackReview, /Copy review data/);
  assert.match(feedbackReview, /> Clear</);
  assert.match(feedbackReview, /bounded to 40 entries/);
});

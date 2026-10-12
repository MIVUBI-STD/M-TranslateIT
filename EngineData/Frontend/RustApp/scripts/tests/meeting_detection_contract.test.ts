import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const meeting = source("src/pages/Meeting.svelte");
const api = source("src/app/bridge/runtimeApi.ts");
const registry = source("src-tauri/src/commands/registry.rs");
const modules = source("src-tauri/src/commands/mod.rs");

test("Meeting app detection and preset suggestions are fully retired", () => {
  assert.doesNotMatch(meeting, /detectMeetingApp|MeetingPresetPanel|meetingDetection|Suggested preset/);
  assert.match(meeting, /onclick=\{handlePrimary\}/);
  assert.doesNotMatch(api, /detectMeetingApp|meetingAppDetectionFallback|MeetingAppDetection/);
  assert.doesNotMatch(registry, /detect_meeting_app/);
  assert.doesNotMatch(modules, /meeting_detection/);
});

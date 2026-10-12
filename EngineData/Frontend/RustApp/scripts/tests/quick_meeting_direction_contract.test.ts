import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const activity = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");

test("Meeting direction is automatic with no visible swap control", () => {
  assert.match(meeting, /Indonesian speech → English voice/);
  assert.doesNotMatch(meeting, /swapListenDirection|directionSaving|Translate what you hear:/);
  assert.match(outbound, /"source_language": "id"/);
  assert.match(outbound, /"target_language": "en"/);
});
test("incoming direction stays in persisted runtime setup, not the Meeting screen", () => {
  assert.match(incoming, /meeting_listen_source_language/);
  assert.match(incoming, /meeting_listen_target_language/);
  assert.match(activity, /languageName\(turn\.source_language\)/);
  assert.match(activity, /languageName\(turn\.target_language\)/);
});

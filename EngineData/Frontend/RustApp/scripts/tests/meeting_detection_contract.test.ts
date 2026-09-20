import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meetingPage = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const runtimeApi = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");

test("meeting detection remains advisory and never auto-starts translation", () => {
  assert.match(meetingPage, /detectMeetingApp/);
  assert.match(meetingPage, /5000/);
  assert.match(meetingPage, /only start when you choose Start Translation/);
  assert.doesNotMatch(meetingPage, /detectMeetingApp\(\)[\s\S]{0,300}startMeetingTranslation/);
});

test("meeting detection bridge has an explicit safe fallback", () => {
  assert.match(runtimeApi, /meetingAppDetectionFallback/);
  assert.match(runtimeApi, /frontend_bridge_unavailable/);
  assert.match(runtimeApi, /"detect_meeting_app"/);
});

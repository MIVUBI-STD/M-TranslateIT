import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const monitor = readFileSync(new URL("../../src/app/runtime/reliabilityMonitor.ts", import.meta.url), "utf8");
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("live reliability monitor is session-scoped and advisory only", () => {
  assert.match(app, /snapshot\.meeting\.applicationOwned && snapshot\.meeting\.hasSession/);
  assert.match(app, /startMeetingRuntimeMonitors/);
  assert.match(monitor, /getRuntimeWatchdogStatus/);
  assert.match(monitor, /getDeviceLossGuardStatus/);
  assert.doesNotMatch(monitor, /stopMeetingTranslation|startMeetingTranslation|selectAudioDevice|startHelperBridge/);
});

test("reliability monitor deduplicates notices and stays low frequency", () => {
  assert.match(monitor, /LIVE_RELIABILITY_POLL_MS = 8_000/);
  assert.match(monitor, /notice\.key !== lastKey/);
  assert.match(monitor, /inFlight/);
});

test("meeting poll and reliability poll share one session-scoped owner", () => {
  assert.match(monitor, /startMeetingRuntimeMonitors/);
  assert.match(monitor, /1_200/);
  assert.match(monitor, /stopReliability/);
});

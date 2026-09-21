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
  assert.match(monitor, /getLongSessionHealthStatus/);
  assert.doesNotMatch(monitor, /stopMeetingTranslation|startMeetingTranslation|selectAudioDevice|startHelperBridge/);
});

test("reliability monitor deduplicates notices and stays low frequency", () => {
  assert.match(monitor, /LIVE_RELIABILITY_POLL_MS = 8_000/);
  assert.match(monitor, /LONG_SESSION_POLL_MS = 30_000/);
  assert.match(monitor, /notice\.key !== lastKey/);
  assert.match(monitor, /inFlight/);
  assert.match(monitor, /longSessionInFlight/);
  assert.match(monitor, /disposed \|\| longSessionInFlight/);
});

test("meeting poll and reliability poll share one session-scoped owner", () => {
  assert.match(monitor, /startMeetingRuntimeMonitors/);
  assert.match(monitor, /1_200/);
  assert.match(monitor, /stopReliability/);
});


test("long-session health is advisory and change-deduplicated", () => {
  assert.match(monitor, /health\.warning_count > 0/);
  assert.match(monitor, /lastLongSessionState/);
  assert.doesNotMatch(monitor, /clearDeferred|cleanup.*temp|replay|restart/i);
});

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const monitor = readFileSync(new URL("../../src/app/runtime/reliabilityMonitor.ts", import.meta.url), "utf8");
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const runtimeEvents = readFileSync(
  new URL("../../src-tauri/src/engine/runtime_events.rs", import.meta.url),
  "utf8",
);
const committedTurns = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url),
  "utf8",
);

test("live reliability monitor is Meeting-owner scoped and advisory only", () => {
  assert.match(app, /snapshot\?\.resources\.owner_kind === "meeting"/);
  assert.match(app, /startMeetingRuntimeMonitors/);
  assert.match(monitor, /getMeetingReliabilitySnapshot/);
  assert.doesNotMatch(monitor, /getRuntimeWatchdogStatus\(/);
  assert.doesNotMatch(monitor, /getDeviceLossGuardStatus\(/);
  assert.match(monitor, /getLongSessionHealthStatus/);
  assert.doesNotMatch(monitor, /stopMeetingTranslation|startMeetingTranslation|selectAudioDevice|startHelperBridge/);
});

test("live reliability uses one aggregated IPC and stays low frequency", () => {
  assert.match(monitor, /getMeetingReliabilitySnapshot/);
  assert.match(monitor, /const \{ watchdog, devices \}/);
  assert.match(monitor, /getLongSessionHealthStatus/);

  assert.match(monitor, /LIVE_RELIABILITY_POLL_MS = 8_000/);
  assert.match(monitor, /LONG_SESSION_POLL_MS = 30_000/);
  assert.match(monitor, /notice\.key !== lastKey/);
  assert.match(monitor, /inFlight/);
  assert.match(monitor, /longSessionInFlight/);
  assert.match(monitor, /disposed \|\| longSessionInFlight/);
});

test("Meeting freshness is event-first with slow reconciliation fallback", () => {
  assert.match(monitor, /MEETING_RUNTIME_EVENT = "translateit:\/\/meeting-runtime"/);
  assert.match(monitor, /listen<MeetingRuntimeEvent>/);
  assert.match(monitor, /MEETING_EVENT_DEBOUNCE_MS = 80/);
  assert.match(monitor, /MEETING_RECONCILIATION_POLL_MS = 10_000/);
  assert.match(monitor, /pollMeeting\(true\)/);
  assert.match(monitor, /pollMeeting\(false\)/);
  assert.doesNotMatch(monitor, /setInterval\([^)]*1_200/);
});

test("backend publishes lightweight Meeting change tokens from authoritative turn state", () => {
  assert.match(runtimeEvents, /MEETING_RUNTIME_EVENT.*translateit:\/\/meeting-runtime/);
  assert.match(runtimeEvents, /revision/);
  assert.match(runtimeEvents, /session_id/);
  assert.match(runtimeEvents, /sequence/);
  assert.match(committedTurns, /emit_meeting_runtime_event\("turn_committed"/);
  assert.match(committedTurns, /emit_meeting_runtime_event\("delivery_state_changed"/);
  assert.doesNotMatch(runtimeEvents, /MeetingCommittedTurnsSnapshot|translated_text|source_text/);
});

test("long-session health is advisory and change-deduplicated", () => {
  assert.match(monitor, /health\.warning_count > 0/);
  assert.match(monitor, /lastLongSessionState/);
  assert.doesNotMatch(monitor, /clearDeferred|cleanup.*temp|replay|restart/i);
});

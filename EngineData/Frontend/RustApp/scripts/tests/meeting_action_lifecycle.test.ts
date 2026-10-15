import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { mapProductMeetingState, meetingControlState } from "../../src/app/bridge/productMeetingState.ts";
import { meetingSessionStatusFallback } from "../../src/app/runtime/meetingBridgeFallback.ts";
import type { MeetingSessionStatus } from "../../src/app/bridge/runtimeApi.ts";
import { APPLICATION_MEETING_OWNER_ID } from "../../src/app/shared/types.ts";

const initial = meetingSessionStatusFallback("status unavailable");
const ready: MeetingSessionStatus = {
  ...initial,
  lifecycle: "idle",
  has_session: false,
  runtime_claim: "application_meeting_session_source_contract_not_windows_runtime_proof",
  blocker: "",
  preflight: {
    ...initial.preflight,
    ready_for_start: true,
    start_eligible: true,
    functional_outbound_ready: true,
    blockers: [],
  },
};

function state(overrides: Partial<MeetingSessionStatus> = {}) {
  return mapProductMeetingState({ ...ready, ...overrides });
}

const control = (m: ReturnType<typeof state>, voice: boolean | null = true, setup = "completed") =>
  meetingControlState(m, voice, setup);

test("Ready offers Start only when voice and Meeting setup are known", () => {
  const m = state();
  assert.equal(m.canStart, true);
  assert.deepEqual(control(m), { action: "start", label: "Start Translation", enabled: true });
  assert.equal(control(m, null).enabled, false);
  assert.equal(control(m, false).label, "Choose Voice");
  assert.equal(control(m, true, "deferred").action, null);
});

test("Live, Starting, Stopping and cleanup all use the same authoritative action", () => {
  const owned = {
    has_session: true, owner_id: APPLICATION_MEETING_OWNER_ID,
    session_id: "meeting-1", authority_active: true,
  };
  const live = state({ ...owned, lifecycle: "live" });
  assert.equal(live.live, true);
  assert.deepEqual(control(live, null), { action: "stop", label: "Stop Translation", enabled: true });

  const starting = state({ ...owned, lifecycle: "starting" });
  assert.deepEqual(control(starting), { action: null, label: "Starting...", enabled: false });
  const stopping = state({ ...owned, lifecycle: "stopping" });
  assert.deepEqual(control(stopping), { action: null, label: "Stopping...", enabled: false });

  const cleanup = state({ ...owned, lifecycle: "cleanup_incomplete", authority_active: false });
  assert.equal(cleanup.canStop, true);
  assert.deepEqual(control(cleanup, null, "deferred"), { action: "stop", label: "Retry Stop", enabled: true });

  const orphanCleanup = state({ lifecycle: "cleanup_incomplete", has_session: false });
  assert.equal(orphanCleanup.hasSession, false);
  assert.equal(orphanCleanup.canStart, false);
  assert.equal(orphanCleanup.canStop, true);
  assert.deepEqual(control(orphanCleanup), { action: "stop", label: "Retry Stop", enabled: true });
});

test("Unknown and foreign-owned sessions cannot issue Start or Stop", () => {
  const unavailable = mapProductMeetingState(initial);
  assert.equal(unavailable.canStart, false);
  assert.equal(unavailable.canStop, false);
  assert.equal(control(unavailable).action, null);
  const unknownOwner = state({ has_session: true, lifecycle: "live", owner_id: null });
  assert.equal(unknownOwner.label, "Check Status");
  assert.equal(control(unknownOwner).enabled, false);
  const foreign = state({ has_session: true, owner_id: "translateit_mic_test", lifecycle: "live" });
  assert.equal(foreign.canStop, false);
  assert.equal(control(foreign).action, null);
});

const rustLifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
const rustPreflight = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/preflight.rs", import.meta.url), "utf8");
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const page = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");
const quickAccess = readFileSync(new URL("../../src/pages/QuickAccess.svelte", import.meta.url), "utf8");

test("Rust preserves no-session cleanup until Stop succeeds and blocks Start meanwhile", () => {
  assert.match(rustLifecycle, /STOP_CLEANUP_PENDING\.store\(true, Ordering::Release\)/);
  assert.match(rustLifecycle, /STOP_CLEANUP_PENDING\.store\(false, Ordering::Release\)/);
  assert.match(rustLifecycle, /if transient_stop_cleanup_pending\(\)/);
  assert.match(rustPreflight, /transient_stop_cleanup_pending\(\)/);
  assert.match(rustPreflight, /"cleanup_incomplete"\.to_string\(\)/);
});

test("Meeting page and Quick Access use the same native Meeting actions", () => {
  assert.match(page, /const primaryControl = \$derived\(meetingControlState\(/);
  assert.match(page, /disabled=\{!actionEnabled\}/);
  assert.match(page, /needsSetup \? "Finish Setup"/);
  assert.match(page, /meeting_setup_state: "new"/);
  assert.match(page, /onclick=\{handlePrimary\}/);
  assert.match(app, /const control = meetingControlState\(/);
  assert.match(app, /const action = startOriginal \? "start" : control\.action/);
  assert.match(header, /toggleQuickAccess/);
  assert.match(quickAccess, /runtimeApi\.selectMeetingAudioSource/);
  assert.match(quickAccess, /applicationRuntimeApi\.dispatchIntent/);
  assert.match(quickAccess, /stop_meeting/);
});

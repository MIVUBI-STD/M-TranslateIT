import assert from "node:assert/strict";
import test from "node:test";

import { resolveClosePolicy, type ClosePolicySnapshot } from "../../src/app/runtime/closePolicy.ts";

const base: ClosePolicySnapshot = {
  recordingLineId: null,
  pendingReview: false,
  runtimeUnavailable: false,
  voiceBuildActive: false,
  audioLocked: true,
  ownerKind: "meeting",
  lifecycle: "meeting_live",
};

function verdict(overrides: Partial<ClosePolicySnapshot> = {}) {
  return resolveClosePolicy({ ...base, ...overrides });
}

test("blocks close while My Voice is recording", () => {
  assert.deepEqual(verdict({ recordingLineId: 2 }), {
    kind: "dialog",
    title: "Voice recording is still running",
    message: "Stop the current My Voice recording before closing TranslateIT so the take can be reviewed safely.",
    action: null,
  });
});

test("My Voice recording takes precedence over runtime availability", () => {
  const result = verdict({ recordingLineId: 2, runtimeUnavailable: true });
  assert.equal(result.kind, "dialog");
  assert.equal(result.kind === "dialog" ? result.title : "", "Voice recording is still running");
});

test("blocks close while a take is pending review", () => {
  assert.equal(verdict({ pendingReview: true }).kind, "dialog");
});

test("fails closed when canonical runtime state is unavailable", () => {
  assert.deepEqual(verdict({ runtimeUnavailable: true }), {
    kind: "dialog",
    title: "Can't confirm runtime state",
    message: "TranslateIT can't confirm whether shared runtime resources are still active. Keep the app open and try again.",
    action: "retry",
  });
});

test("blocks close while My Voice build is active", () => {
  assert.equal(verdict({ voiceBuildActive: true }).kind, "dialog");
});

test("destroys immediately when no shared audio resource is owned", () => {
  assert.deepEqual(verdict({ audioLocked: false, ownerKind: "none", lifecycle: "ready" }), { kind: "destroy" });
});

test("blocks close when audio belongs to another action", () => {
  assert.equal(verdict({ ownerKind: "mic_test", lifecycle: "mic_test_live" }).kind, "dialog");
  assert.equal(verdict({ ownerKind: "voice_recording", lifecycle: "voice_recording_live" }).kind, "dialog");
});

test("waits for an existing Meeting stop to finish", () => {
  assert.deepEqual(verdict({ lifecycle: "meeting_stopping" }), {
    kind: "wait-for-stop",
    title: "Translation is stopping",
    message: "TranslateIT will close after Meeting translation finishes stopping.",
  });
});

test("requires stop-and-close for an application-owned active Meeting session", () => {
  assert.deepEqual(verdict(), { kind: "stop-and-close" });
});

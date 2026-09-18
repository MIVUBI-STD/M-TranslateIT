import assert from "node:assert/strict";
import test from "node:test";

import {
  persistedSetupCheckpoint,
  safeSetupResumeStep,
  setupCheckpoint,
  setupStateNeedsResume,
} from "../../src/app/runtime/setupFlow.ts";

test("maps legacy five-step checkpoints into the compact four-step flow", () => {
  assert.equal(setupCheckpoint(1), 1);
  assert.equal(setupCheckpoint(2), 2);
  assert.equal(setupCheckpoint(3), 3);
  assert.equal(setupCheckpoint(4), 3);
  assert.equal(setupCheckpoint(5), 4);
  assert.equal(setupCheckpoint(99), 4);
});

test("writes compact step four using the legacy-safe persisted checkpoint", () => {
  assert.equal(persistedSetupCheckpoint(1), 1);
  assert.equal(persistedSetupCheckpoint(2), 2);
  assert.equal(persistedSetupCheckpoint(3), 3);
  assert.equal(persistedSetupCheckpoint(4), 5);
  assert.equal(setupCheckpoint(persistedSetupCheckpoint(4)), 4);
});

test("resume returns to the first unmet required setup owner", () => {
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: false, meetingRouteReady: false }),
    2,
  );
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: true, meetingRouteReady: false }),
    3,
  );
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: true, meetingRouteReady: true }),
    4,
  );
});

test("only deferred setup is treated as resumable", () => {
  assert.equal(setupStateNeedsResume("deferred"), true);
  assert.equal(setupStateNeedsResume("new"), false);
  assert.equal(setupStateNeedsResume("completed"), false);
});

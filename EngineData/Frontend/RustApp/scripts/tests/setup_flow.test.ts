import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

const firstSetup = readFileSync(new URL("../../src/pages/FirstSetup.svelte", import.meta.url), "utf8");
const setupNavigation = readFileSync(new URL("../../src/components/setup/SetupNavigation.svelte", import.meta.url), "utf8");
const appShell = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("first launch gives Text its own task entry without completing Meeting setup", () => {
  const start = firstSetup.indexOf("async function openTextInstead()");
  const end = firstSetup.indexOf("\n  async function saveSelectedMicrophone()", start);
  assert.ok(start >= 0 && end > start, "Text entry must use the existing FirstSetup owner");
  const entry = firstSetup.slice(start, end);
  assert.match(entry, /persistSetupFact\("deferred", step\)/);
  assert.match(entry, /if \(saved\) await onComplete\(settings, "text"\)/);
  assert.doesNotMatch(entry, /persistSetupFact\("completed"/);

  assert.match(setupNavigation, /onOpenText: \(\) => void \| Promise<void>/);
  assert.match(setupNavigation, /Translate text/);
  assert.match(setupNavigation, /Set up meeting translation/);
  assert.match(setupNavigation, /Set Up Later/);
});

test("Text destination is explicit; returning users retain Meeting as the default", () => {
  const start = appShell.indexOf("async function finishFirstSetup(");
  const end = appShell.indexOf("\n  async function openMyVoiceFromSetup(", start);
  assert.ok(start >= 0 && end > start);
  const entry = appShell.slice(start, end);
  assert.match(entry, /destination: "meeting" \| "text" = "meeting"/);
  assert.match(entry, /route = destination/);
  assert.match(entry, /await refreshSnapshot/);
  assert.match(appShell, /let route=\$state<AppRoute>\("meeting"\)/);
});

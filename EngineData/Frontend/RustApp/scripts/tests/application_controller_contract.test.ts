import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const controller = readFileSync(
  new URL("../../src/app/runtime/applicationController.ts", import.meta.url),
  "utf8",
);
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("application controller owns the product runtime snapshot", () => {
  assert.match(controller, /snapshot: ProductRuntimeSnapshot \| null/);
  assert.match(controller, /async refresh\(knownSettings\?: RuntimeSettings\)/);
  assert.match(controller, /applySettings\(settings: RuntimeSettings\)/);
  assert.match(controller, /applyMeetingSession/);
});

test("App shell consumes one controller snapshot instead of subsystem copies", () => {
  assert.match(app, /createApplicationController\(defaultSettings\(\)\)/);
  assert.match(app, /const snapshot = \$derived\(applicationState\.snapshot\)/);
  assert.equal(app.includes("let runtimeSettings = $state"), false);
  assert.equal(app.includes("let helperStatus = $state"), false);
  assert.equal(app.includes("let workerStatus = $state"), false);
  assert.equal(app.includes("let inputStatus = $state"), false);
  assert.equal(app.includes("let approvedVoiceReady = $state"), false);
});

test("controller owns setup state transitions", () => {
  assert.match(controller, /meeting_setup_state === "new"/);
  assert.match(controller, /setupRequired/);
  assert.match(controller, /runtimeLoaded/);
  assert.equal(/setupRequired\s*=/.test(app), false);
});

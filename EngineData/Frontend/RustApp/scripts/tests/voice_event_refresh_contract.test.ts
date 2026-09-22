import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const mutations = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/mutations.rs", import.meta.url),
  "utf8",
);
const runtimeEvents = readFileSync(
  new URL("../../src-tauri/src/engine/runtime_events.rs", import.meta.url),
  "utf8",
);

test("Voice selection and approval refresh through ApplicationRuntime events", () => {
  assert.match(mutations, /emit_after\(&app, "approve_voice_candidate"/);
  assert.match(mutations, /emit_after\(&app, "select_builtin_voice"/);

  const start = app.indexOf("async function syncMeetingVoice");
  const body = app.slice(start, start + 260);
  assert.match(body, /setNotice\(/);
  assert.doesNotMatch(body, /refreshSnapshot\(/);
});

test("background Voice build completion uses a global backend event", () => {
  assert.match(runtimeEvents, /translateit:\/\/voice-build-runtime/);
  assert.doesNotMatch(app, /onRuntimeStateChanged/);
});

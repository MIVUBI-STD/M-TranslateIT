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
const voiceApi = readFileSync(
  new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url),
  "utf8",
);
const voiceSync = readFileSync(
  new URL("../../src/app/runtime/voiceBuildRuntimeSync.ts", import.meta.url),
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
  assert.match(voiceApi, /subscribeRuntime/);
  assert.match(voiceApi, /translateit:\/\/voice-build-runtime/);
  assert.match(app, /startVoiceBuildRuntimeSync/);
  assert.match(voiceSync, /myVoiceBuildApi\.subscribeRuntime/);
  assert.match(voiceSync, /if \(disposed\)/);
  assert.match(voiceSync, /unlisten\?\.\(\)/);
  assert.doesNotMatch(app, /onRuntimeStateChanged/);
});

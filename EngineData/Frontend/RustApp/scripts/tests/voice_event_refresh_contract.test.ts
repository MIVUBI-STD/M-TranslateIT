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

test("Voice selection and approval refresh the canonical runtime snapshot", () => {
  assert.match(mutations, /emit_after\(&app, "approve_voice_candidate"/);
  assert.match(mutations, /emit_after\(&app, "select_builtin_voice"/);

  const start = app.indexOf("async function syncMeetingVoice");
  const body = app.slice(start, start + 260);
  assert.match(body, /await refreshSnapshot\(message \?\? "Meeting voice updated\."\)/);
  assert.doesNotMatch(body, /setNotice\(/);
});

test("unknown My Voice selection responses never invite repeated writes", () => {
  const source = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  for (const name of ["selectBuiltin", "selectStoredPersonalVoice"]) {
    const start = source.indexOf(`async function ${name}(`);
    const end = source.indexOf("\n  }", start);
    assert.ok(start >= 0 && end > start);
    const action = source.slice(start, end);
    assert.match(action, /!selectedVoiceVerified/);
    assert.match(action, /let nativeSelectionConfirmed = false/);
    assert.match(action, /result\.state === "frontend_bridge_error" \|\| result\.build\.phase === "unavailable"/);
    assert.match(action, /selectedVoiceVerified = false/);
    assert.match(action, /nativeSelectionConfirmed = true/);
    assert.match(action, /if \(!await refreshVoiceSelection\(\)\)/);
    assert.match(action, /nativeSelectionConfirmed\s*\?/);
    assert.doesNotMatch(action, /Could not switch to your saved My Voice\. Try again\./);
  }
  assert.match(source, /onRetry=\{refreshVoiceSelection\}/);
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

const myVoicePage = readFileSync(
  new URL("../../src/pages/MyVoice.svelte", import.meta.url),
  "utf8",
);

function myVoiceAction(name: string): string {
  const start = myVoicePage.indexOf(`async function ${name}(`);
  assert.ok(start >= 0, `Missing My Voice action ${name}`);
  const end = myVoicePage.indexOf("\n  }", start);
  assert.ok(end > start, `Missing end of My Voice action ${name}`);
  return myVoicePage.slice(start, end);
}

test("unknown recording mutation responses re-read backend truth without repeating writes", () => {
  const reconcile = myVoiceAction("reconcileUncertainRecordingAction");
  assert.match(reconcile, /recordingStatusUnknown = true/);
  assert.match(reconcile, /onRecordingChange\(true\)/);
  assert.match(reconcile, /await refresh\(\)/);
  assert.match(reconcile, /!recordingStatusUnknown/);
  const result = myVoiceAction("applyResult");
  assert.match(result, /frontend_bridge_error/);
  assert.match(result, /!result\.recording/);
  assert.match(result, /await reconcileUncertainRecordingAction\(/);

  for (const name of ["startRecording", "stopRecording", "retryTake", "acceptTake"]) {
    const body = myVoiceAction(name);
    assert.match(body, /await applyResult\(/, `${name} must await reconciliation`);
    assert.match(body, /catch \{\s*await reconcileUncertainRecordingAction\(/,
      `${name} must reconcile an unknown mutation outcome`);
  }
  const accept = myVoiceAction("acceptTake");
  assert.match(accept, /await applyResult\(result, true\)/);
  assert.match(accept, /acceptedLineId/);
  assert.match(accept, /reconcileUncertainRecordingAction\([\s\S]*?true/);
  assert.doesNotMatch(accept.slice(accept.indexOf("} catch {")), /myVoiceApi\.acceptTake\(/);
});

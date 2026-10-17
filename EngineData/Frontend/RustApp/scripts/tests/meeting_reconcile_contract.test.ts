import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const reader = readFileSync(
  new URL("../../src/app/runtime/meetingReconcileReader.ts", import.meta.url),
  "utf8",
);
const controller = readFileSync(
  new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url),
  "utf8",
);

test("reconciliation ignores non-Meeting runtime owners before transcript reads", () => {
  assert.match(reader, /APPLICATION_MEETING_OWNER_ID/);
  assert.match(reader, /status\.owner_id !== APPLICATION_MEETING_OWNER_ID/);
  const ownerGuard = reader.indexOf("status.owner_id !== APPLICATION_MEETING_OWNER_ID");
  const transcriptRead = reader.indexOf("getMeetingCommittedTurns");
  assert.ok(ownerGuard >= 0 && transcriptRead > ownerGuard);
});

test("newer action/application snapshot invalidates stale Meeting-only reads", () => {
  const app = readFileSync(
    new URL("../../src/App.svelte", import.meta.url),
    "utf8",
  );
  assert.match(app, /if \(!result\.applied\) return;[\s\S]*meetingViewState = meetingLiveController\.invalidate\(\)/);
  assert.match(app, /const actionControllerRevision = applicationController\.read\(\)\.revision/);
  assert.match(app, /if \(applicationController\.read\(\)\.revision !== actionControllerRevision\)/);
  assert.match(app, /const pollControllerRevision = applicationController\.read\(\)\.revision/);
  assert.match(app, /if \(applicationController\.read\(\)\.revision !== pollControllerRevision\)/);
  assert.match(controller, /callbacks\.onStatus\(result\.status\);\s*\/\/[^\n]*[\s\S]*?if \(revision !== state\.revision\) return publish\(\);\s*const overlayRevision/);
});

test("session-scoped transcript fallback never reuses foreign or failed turns", () => {
  assert.match(reader, /currentTurns\?\.ok && currentTurns\.has_session/);
  assert.match(reader, /currentTurns\.session_id === status\.session_id \? currentTurns : null/);
  assert.match(reader, /statusKey === previousStatusKey && !forceTurns && retainedTurns/);
  assert.match(reader, /nextTurns\.ok && nextTurns\.has_session && nextTurns\.session_id === status\.session_id/);
  assert.match(reader, /turns: retainedTurns,/);
  assert.match(reader, /transcriptStatusKey: retainedTurns \? previousStatusKey : ""/);
  assert.doesNotMatch(reader, /currentTurns \?\? nextTurns/);
  assert.match(controller, /publishLatestMeetingOverlay\(\s*result\.turns,/);
});

test("event reconciliation can bypass unchanged status-key shortcut", () => {
  assert.match(reader, /forceTurns = false/);
  assert.match(reader, /statusKey === previousStatusKey && !forceTurns/);
  assert.match(controller, /forceTurns = false/);
  assert.match(controller, /forceTurns,/);
});

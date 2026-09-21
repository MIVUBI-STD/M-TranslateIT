import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const meeting = readFileSync(
  new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url),
  "utf8",
);
const close = readFileSync(
  new URL("../../src/app/runtime/closeController.ts", import.meta.url),
  "utf8",
);
const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);

test("Meeting live coordination is owned outside App.svelte", () => {
  assert.match(app, /createMeetingLiveController\(\)/);
  assert.match(meeting, /readMeetingPoll/);
  assert.match(meeting, /publishLatestMeetingOverlay/);
  assert.equal(app.includes("let meetingPollInFlight"), false);
  assert.equal(app.includes("let lastTranscriptStatusKey"), false);
  assert.equal(app.includes("let lastOverlayMeetingRevision"), false);
});

test("close flow coordination is owned outside App.svelte", () => {
  assert.match(app, /createCloseController\(\)/);
  assert.match(close, /resolveNativeCloseVerdict/);
  assert.match(close, /stopAndResolveNativeClose/);
  assert.equal(app.includes("let closeDialogOpen = $state"), false);
  assert.equal(app.includes("let closeDialogTitle = $state"), false);
  assert.equal(app.includes("let closeDialogMessage = $state"), false);
});

test("runtime product facade remains a thin compatibility surface", () => {
  assert.match(facade, /from "\.\/productAudioFacade"/);
  assert.match(facade, /from "\.\/productTranslationFacade"/);
  assert.match(facade, /from "\.\/productSetupFacade"/);
  assert.doesNotMatch(facade, /async function probeProductAudioDevice/);
  assert.doesNotMatch(facade, /async function runProductTranslationAlternative/);
});

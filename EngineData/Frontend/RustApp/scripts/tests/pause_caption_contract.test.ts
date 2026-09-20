import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const overlay = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");

test("pause captions freezes only meeting presentation and keeps latest caption pending", () => {
  assert.match(overlay, /captionsPaused/);
  assert.match(overlay, /pendingCaption/);
  assert.match(overlay, /if \(captionsPaused && next\.source === "meeting"\)/);
  assert.match(overlay, /pendingCaption = next/);
});

test("resume captions applies the newest pending or durable meeting caption", () => {
  assert.match(overlay, /pendingCaption \?\? readLatestOverlayCaption\(\)/);
  assert.match(overlay, /Resume floating captions/);
  assert.match(overlay, /Pause floating captions/);
  assert.match(overlay, /PAUSED/);
});

test("pause captions does not call meeting stop or mutate translation runtime", () => {
  assert.doesNotMatch(overlay, /stopMeetingTranslation/);
  assert.doesNotMatch(overlay, /startMeetingTranslation/);
  assert.doesNotMatch(overlay, /stop_capture|start_capture/);
});

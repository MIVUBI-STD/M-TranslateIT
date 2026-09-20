import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const overlay = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");

test("recent caption history reads canonical meeting turns only on user request", () => {
  assert.match(overlay, /Show recent translations/);
  assert.match(overlay, /runtimeApi\.getMeetingCommittedTurns\(\)/);
  assert.match(overlay, /recentMeetingCaptions\(snapshot, 20\)/);
  assert.doesNotMatch(overlay, /setInterval/);
});

test("recent history stays ephemeral and separate from floating caption persistence", () => {
  assert.doesNotMatch(overlay, /localStorage/);
  assert.match(overlay, /Back to live caption/);
  assert.match(overlay, /Recent committed translations/);
});

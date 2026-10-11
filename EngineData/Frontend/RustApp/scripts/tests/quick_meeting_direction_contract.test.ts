import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const activity = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const api = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
const overlayPolicy = readFileSync(
  new URL("../../src/app/runtime/translationOverlayPolicy.ts", import.meta.url), "utf8",
);

test("quick meeting listen direction persists through canonical settings", () => {
  assert.match(meeting, /meeting_listen_source_language/);
  assert.match(meeting, /meeting_listen_target_language/);
  assert.match(meeting, /saveSettings\(candidate\)/);
  assert.match(meeting, /Translate what you hear:/);
  assert.match(meeting, /directionSaving \? "Saving\.\.\." : "Swap"/);
});

test("conversation rendering uses committed language metadata", () => {
  assert.match(api, /source_language: string/);
  assert.match(api, /target_language: string/);
  assert.match(activity, /languageName\(turn\.source_language\)/);
  assert.match(activity, /languageName\(turn\.target_language\)/);
  assert.match(activity, /lang=\{turn\.source_language\}/);
  assert.match(activity, /lang=\{turn\.target_language\}/);
});

test("meeting quick direction stays locked while any Meeting session or cleanup remains", () => {
  assert.match(meeting, /directionSaving \|\| meeting\.hasSession \|\| meeting\.canStop/);
  assert.match(meeting, /Finish stopping Meeting translation before changing direction/);
});

test("Meeting outbound direction is not changed by the Text language swap", () => {
  assert.match(meeting, /const outboundSourceName = "Indonesian"/);
  assert.match(meeting, /const outboundTargetName = "English"/);
  assert.doesNotMatch(meeting, /outboundSourceName = \$derived\(languageName\(snapshot\.settings\.source_language\)\)/);
});

test("committed turn language metadata reaches overlay policy", () => {
  assert.match(overlayPolicy, /latest\.target_language/);
  assert.match(overlayPolicy, /turn\.target_language/);
});

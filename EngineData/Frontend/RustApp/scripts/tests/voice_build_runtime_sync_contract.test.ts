import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const build = readFileSync(
  new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url),
  "utf8",
);
const page = readFileSync(
  new URL("../../src/pages/MyVoice.svelte", import.meta.url),
  "utf8",
);
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("Voice build completion refreshes canonical runtime state", () => {
  assert.match(build, /const buildFinished = build\.active && !next\.active/);
  assert.match(build, /if \(buildFinished\) void onRuntimeStateChanged\(\)/);
  assert.match(page, /\{onRuntimeStateChanged\}/);
  assert.match(app, /onRuntimeStateChanged=\{\(\) => refreshSnapshot\(\)\}/);
});

test("Voice build completion callback is distinct from Meeting voice selection callback", () => {
  assert.match(build, /onMeetingVoiceChanged/);
  assert.match(build, /onRuntimeStateChanged/);
  assert.notEqual(
    build.indexOf("onMeetingVoiceChanged"),
    build.indexOf("onRuntimeStateChanged"),
  );
});

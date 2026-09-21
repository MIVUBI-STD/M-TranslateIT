import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const overlayPolicy = readFileSync(new URL("../../src/app/runtime/translationOverlayPolicy.ts", import.meta.url), "utf8");
const runtimeApi = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");

test("meeting quick direction is locked while meeting owns live or busy runtime", () => {
  assert.match(meeting, /directionSaving \|\| meeting\.live \|\| meeting\.busy/);
  assert.match(meeting, /Stop Meeting translation before changing direction/);
});

test("meeting header is derived from configured outbound language direction", () => {
  assert.match(meeting, /outboundSourceName/);
  assert.match(meeting, /outboundTargetName/);
  assert.doesNotMatch(meeting, />Indonesian<\/strong>/);
  assert.doesNotMatch(meeting, />English voice<\/strong>/);
});

test("committed turn language metadata reaches overlay policy", () => {
  assert.match(runtimeApi, /source_language: string/);
  assert.match(runtimeApi, /target_language: string/);
  assert.match(overlayPolicy, /latest\.target_language/);
  assert.match(overlayPolicy, /turn\.target_language/);
});

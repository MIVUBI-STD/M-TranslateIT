import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const source = (path: string) => readFileSync(new URL(path, root), "utf8");
const page = source("src/pages/MyVoice.svelte");
const build = source("src/components/my-voice/MyVoiceBuild.svelte");

test("normal My Voice opens with a ready-to-use voice picker before optional creation", () => {
  assert.match(page, /<BuiltinVoicePicker/);
  assert.match(page, /let creatorOpen = \$state\(false\)/);
  assert.match(page, /hidden=\{!creatorOpen\}/);
  assert.ok(page.indexOf("<BuiltinVoicePicker") < page.indexOf('id="my-voice-create-workspace"'));
  assert.match(page, /Built-in voices work without recording or training/);
  assert.match(page, /aria-controls="my-voice-create-workspace" aria-expanded=\{creatorOpen\}/);
});

test("hidden voice creator stays mounted and must show active or uncertain work", () => {
  assert.match(page, /const creatorNeedsAttention = \$derived\(Boolean\(/);
  for (const state of [
    "recordingStatusUnknown", "isRecording", "pendingLineId !== null",
    "buildStatusUnavailable", "buildSnapshot?.active", "buildSnapshot?.preview_active",
    "buildSnapshot?.evaluation_ready",
  ]) assert.ok(page.includes(state), state);
  assert.match(page, /if \(creatorNeedsAttention\) creatorOpen = true/);
  assert.match(page, /disabled=\{creatorNeedsAttention\}/);
  assert.match(page, /<MyVoiceBuild/);
  assert.match(build, /Stop Creating/);
  assert.match(build, /Stop Preview/);
  assert.doesNotMatch(page, /\{#if creatorOpen\}[\s\S]*?<MyVoiceBuild/);
});

test("ongoing Improve My Voice request cannot be hidden while submission is pending", () => {
  assert.match(page, /\|\| improvementStarting \|\| buildStatusUnavailable/);
  assert.match(page, /if \(creatorNeedsAttention\) creatorOpen = true/);
  assert.match(page, /disabled=\{creatorNeedsAttention\}/);
});

test("saved recordings, historic versions and backup remain accessible but secondary", () => {
  assert.match(page, /<details[^>]*aria-label="Saved voice management"/);
  assert.match(page, /Saved voices &amp; backups/);
  assert.match(page, /Recording storage needs attention/);
  assert.match(page, /<VoiceVersionBrowser/);
  assert.match(page, /<VoiceBackupRestore/);
  assert.match(page, /<section class="mt-4" aria-label="Recording library">/);
  assert.match(page, /protectLegacyRecordings\(\)/);
});

test("training hardware internals and nonselectable quick preview are disclosed only on demand", () => {
  assert.match(build, /<summary[^>]*>Technical device details<\/summary>/);
  assert.match(build, /<summary[^>]*>Advanced: temporary voice sample<\/summary>/);
  assert.match(build, /This sample cannot be selected as a Meeting voice/);
  assert.match(build, /<QuickVoicePreview/);
  assert.match(build, /onGenerate=\{generateQuickPreview\}/);
  assert.match(build, /onPlay=\{playQuickPreview\}/);
  assert.match(build, /computeStatus\?\.admission_state === "admitted"/);
  assert.match(build, /Recheck hardware/);
});

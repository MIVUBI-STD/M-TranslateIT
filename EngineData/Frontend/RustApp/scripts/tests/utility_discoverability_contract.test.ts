import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (relative: string) => readFileSync(new URL(`../../src/${relative}`, import.meta.url), "utf8");
const commands = source("app/runtime/productCommandRegistry.ts");
const palette = source("components/runtime/CommandPalette.svelte");
const app = source("App.svelte");
const settings = source("pages/Settings.svelte");
const preferences = source("components/settings/TranslationPreferences.svelte");
const feedback = source("components/settings/TranslationFeedbackReview.svelte");
const presets = source("components/meeting/MeetingPresetPanel.svelte");

test("Quick Actions locates canonical tools without adding background watchers or new runtime owners", () => {
  for (const id of ["meeting-presets", "meeting-transcript", "floating-captions", "spoken-terms",
    "preferred-words", "translation-feedback", "meeting-microphone", "diagnostics"]) {
    assert.match(commands, new RegExp(`"${id}"`));
  }
  assert.match(commands, /openUtility/);
  assert.doesNotMatch(commands, /localStorage|setInterval|addEventListener/);
  assert.match(app, /utilityRequest \+= 1/);
  assert.match(settings, /utilityTarget/);
});

test("Quick Actions keyboard activation follows native focused buttons and restores focus", () => {
  assert.match(palette, /onInputKeydown/);
  assert.match(palette, /onResultKeydown/);
  assert.match(palette, /onDialogKeydown/);
  assert.match(palette, /event\.key === "Tab"/);
  assert.match(palette, /event\.key === "Escape"/);
  assert.match(palette, /previousFocus/);
  assert.match(palette, /restore\.focus\(\)/);
  assert.match(palette, /onclick=\{\(\) => void run\(command\)\}/);
});

test("destructive utility actions have reversible decision before persistent mutation", () => {
  assert.match(presets, /pendingRemovalId !== id/);
  assert.match(presets, /Remove preset</);
  assert.match(preferences, /pendingDelete/);
  assert.match(preferences, /Remove this spoken term/);
  assert.match(preferences, /Remove this pair/);
  assert.match(feedback, /confirmClear/);
  assert.match(feedback, /Clear reports/);
});

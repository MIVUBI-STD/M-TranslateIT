import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { canUseCurrentTextResult, type TextResultContext } from "../../src/app/runtime/textResultPolicy.ts";
import { textTranslationCacheKey } from "../../src/app/runtime/textTranslationCache.ts";
import { defaultSettings } from "../../src/app/shared/state.ts";

const base = defaultSettings();
const settingsKey = textTranslationCacheKey("", base);
const valid: TextResultContext = {
  source: " halo ",
  target: "Hello",
  translatedSource: "halo",
  translatedSettingsKey: settingsKey,
  currentSettingsKey: settingsKey,
  state: "success",
};

test("Text result is reusable only for its original source and settings", () => {
  assert.equal(canUseCurrentTextResult(valid), true);
  assert.equal(canUseCurrentTextResult({ ...valid, state: "review" }), true);
  assert.equal(canUseCurrentTextResult({ ...valid, state: "edited", target: "Good morning" }), true);
  assert.equal(canUseCurrentTextResult({ ...valid, source: "selamat pagi" }), false);
  assert.equal(canUseCurrentTextResult({ ...valid, target: "  " }), false);
  assert.equal(canUseCurrentTextResult({ ...valid, translatedSource: null }), false);
  assert.equal(canUseCurrentTextResult({ ...valid, translatedSettingsKey: null }), false);
});

test("only active translation direction invalidates a result; retired preferences do not", () => {
  const reverse = { ...base, source_language: "en", target_language: "id" };
  const formal = { ...base, translation_style: "formal" };
  const terms = { ...base, terminology: [{ indonesian: "arsip", english: "archive" }] };
  assert.equal(canUseCurrentTextResult({
    ...valid, currentSettingsKey: textTranslationCacheKey("", reverse),
  }), false);
  for (const inactive of [formal, terms]) {
    assert.equal(canUseCurrentTextResult({
      ...valid, currentSettingsKey: textTranslationCacheKey("", inactive),
    }), true);
  }
});

test("idle, pending, outdated and failed results cannot enter downstream actions", () => {
  for (const state of ["idle", "translating", "stale", "error"] as const) {
    assert.equal(canUseCurrentTextResult({ ...valid, state }), false);
  }
});

const ui = readFileSync(new URL("../../src/pages/Text.svelte", import.meta.url), "utf8");

test("Text maps fresh and cached review metadata into one visible review state", () => {
  assert.match(ui, /function applyTranslationResult\(/);
  assert.match(ui, /resultState = needsReview \? "review" : "success"/);
  assert.match(ui, /reviewHints = needsReview \? \[\.\.\.hints\] : \[\]/);
  assert.match(ui, /applyTranslationResult\(cached\.translated, cached\.needsReview, cached\.reviewHints, requestSource, currentKey\)/);
  assert.match(ui, /applyTranslationResult\(result\.translated, result\.needsReview, result\.reviewHints, requestSource, currentKey\)/);
  assert.match(ui, /resultState === "review" && currentResultUsable/);
  assert.match(ui, /aria-label="Translation review notes"/);
  assert.match(ui, /\{#each reviewHints as hint/);
  assert.match(ui, /function sourceChanged\(\): void \{[\s\S]*?reviewHints = \[\]/);
  assert.match(ui, /function targetChanged\(\): void \{[\s\S]*?reviewHints = \[\]/);
  assert.match(ui, /sourceText = nextInput; targetText = nextTarget;[\s\S]*?reviewHints = \[\]/);
});

test("minimal Text guards Copy and preserves source during Swap", () => {
  assert.match(ui, /const currentResultUsable = \$derived\(canUseCurrentTextResult\(/);
  assert.match(ui, /async function copyTranslation\(\)/);
  assert.match(ui, /if \(!currentResultUsable\) return/);
  assert.match(ui, /disabled=\{!currentResultUsable\}/);
  assert.match(ui, /const originalInput = sourceText/);
  assert.match(ui, /originalOutput\.trim\(\) \? originalOutput : originalInput/);
  assert.match(ui, /originalOutput\.trim\(\) \? originalInput : ""/);
  assert.doesNotMatch(ui, /publishTranslationOverlay|showFloatingCaption|requestAlternative/);
});

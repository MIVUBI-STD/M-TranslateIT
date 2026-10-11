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

test("changed direction, style or terminology invalidates prior translation actions", () => {
  const enId = { ...base, source_language: "en", target_language: "id" };
  const formal = { ...base, translation_style: "formal" };
  const vocabulary = { ...base, terminology: [{ indonesian: "arsip", english: "archive" }] };
  for (const next of [enId, formal, vocabulary]) {
    assert.equal(canUseCurrentTextResult({
      ...valid,
      currentSettingsKey: textTranslationCacheKey("", next),
    }), false);
  }
});

test("idle, pending, outdated and failed results cannot enter downstream actions", () => {
  for (const state of ["idle", "translating", "stale", "error"] as const) {
    assert.equal(canUseCurrentTextResult({ ...valid, state }), false);
  }
});

const ui = readFileSync(new URL("../../src/pages/Text.svelte", import.meta.url), "utf8");

test("Text UI checks result provenance for alternative, caption, copy and feedback", () => {
  assert.match(ui, /const currentResultUsable = \$derived\(canUseCurrentTextResult\(/);
  const handlers = ["requestAlternative", "showFloatingCaption", "copySelection"];
  for (const name of handlers) {
    const start = ui.indexOf(`async function ${name}(`);
    assert.ok(start >= 0, `${name} missing`);
    assert.match(ui.slice(start, start + 600), /currentResultUsable/, `${name} lacks action guard`);
  }
  assert.match(ui, /disabled=\{!currentResultUsable \|\|/);
  assert.match(ui, /source=\{currentResultUsable \? sourceText\.trim\(\) : ""\}/);
});

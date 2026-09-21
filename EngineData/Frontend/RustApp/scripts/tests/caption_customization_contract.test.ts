import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const owner = readFileSync(new URL("../../src/components/settings/FloatingCaptionPreferences.svelte", import.meta.url), "utf8");
const overlay = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");
const state = readFileSync(new URL("../../src/app/runtime/translationOverlayState.ts", import.meta.url), "utf8");

test("caption customization stays limited to readability controls", () => {
  assert.match(owner, /Text size/);
  assert.match(owner, /Reading width/);
  assert.match(owner, /Contrast/);
  assert.doesNotMatch(owner, /font family|color picker|shadow|opacity slider/i);
});

test("caption width and contrast persist as bounded presets", () => {
  assert.match(state, /"compact", "standard", "wide"/);
  assert.match(state, /"standard", "high"/);
  assert.match(overlay, /overlayWidth\(preferences\.width\)/);
  assert.match(overlay, /preferences\.contrast === "high"/);
});

test("floating caption settings live in a dedicated owner component", () => {
  const translationPreferences = readFileSync(new URL("../../src/components/settings/TranslationPreferences.svelte", import.meta.url), "utf8");
  assert.match(translationPreferences, /FloatingCaptionPreferences/);
  assert.doesNotMatch(translationPreferences, /Caption text size/);
});

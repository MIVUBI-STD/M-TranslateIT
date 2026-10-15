import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const owner = readFileSync(new URL("../../src/components/settings/FloatingCaptionPreferences.svelte", import.meta.url), "utf8");
const overlay = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");
const overlayCss = readFileSync(new URL("../../src/styles/translation-overlay.css", import.meta.url), "utf8");
const state = readFileSync(new URL("../../src/app/runtime/translationOverlayState.ts", import.meta.url), "utf8");

test("caption customization stays limited to readability controls", () => {
  assert.match(owner, /Text size/);
  assert.match(owner, /<summary[^>]*>More caption options<\/summary>/);
  assert.match(owner, /<details/);
  assert.match(owner, /Reading width/);
  assert.match(owner, /Contrast/);
  assert.doesNotMatch(owner, /font family|color picker|shadow|opacity slider/i);
});

test("caption width and contrast persist as bounded presets", () => {
  assert.match(state, /"compact", "standard", "wide"/);
  assert.match(state, /"standard", "high"/);
  assert.match(overlay, /overlayWidth\(preferences\.width\)/);
  assert.match(overlay, /preferences\.contrast === "high"/);
  assert.match(overlay, /translation-overlay\.css/);
  assert.match(overlayCss, /\.overlay-shell\.high-contrast \.caption-card/);
  assert.match(owner, /Allow clicks through caption/);
  assert.match(owner, /Show caption window/);
});

test("floating caption preferences belong directly to Meeting Settings", () => {
  const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  const textPage = readFileSync(new URL("../../src/pages/Text.svelte", import.meta.url), "utf8");
  assert.match(settings, /<FloatingCaptionPreferences/);
  assert.match(settings, /Meeting Captions/);
  assert.doesNotMatch(textPage, /publishTranslationOverlay|Floating caption/);
});

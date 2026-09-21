import assert from "node:assert/strict";
import test from "node:test";

import {
  getCachedTextTranslation,
  putCachedTextTranslation,
  textTranslationCacheKey,
} from "../../src/app/runtime/textTranslationCache.ts";
import { defaultSettings } from "../../src/app/shared/state.ts";

test("Text cache key invalidates on direction, style, and terminology", () => {
  const base = defaultSettings();
  const natural = textTranslationCacheKey(" halo ", base);

  const formalSettings = { ...base, translation_style: "formal", audio: { ...base.audio } };
  assert.notEqual(textTranslationCacheKey("halo", formalSettings), natural);

  const reverse = { ...base, source_language: "en", target_language: "id", audio: { ...base.audio } };
  assert.notEqual(textTranslationCacheKey("halo", reverse), natural);

  const terminology = {
    ...base,
    terminology: [{ indonesian: "arsip", english: "archive" }],
    audio: { ...base.audio },
  };
  assert.notEqual(textTranslationCacheKey("halo", terminology), natural);
});

test("Text cache is bounded to 32 LRU entries and copies review hints", () => {
  const settings = defaultSettings();
  const keys = Array.from({ length: 33 }, (_, index) => textTranslationCacheKey(`cache-${index}`, settings));
  for (let index = 0; index < keys.length; index += 1) {
    putCachedTextTranslation({
      key: keys[index]!,
      translated: `translated-${index}`,
      reviewHints: [`hint-${index}`],
      needsReview: false,
    });
  }

  assert.equal(getCachedTextTranslation(keys[0]!), null);
  const hit = getCachedTextTranslation(keys[32]!);
  assert.equal(hit?.translated, "translated-32");
  assert.deepEqual(hit?.reviewHints, ["hint-32"]);
  if (hit) hit.reviewHints.push("mutated");
  assert.deepEqual(getCachedTextTranslation(keys[32]!)?.reviewHints, ["hint-32"]);
});

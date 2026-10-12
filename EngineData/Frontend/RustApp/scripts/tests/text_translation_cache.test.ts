import assert from "node:assert/strict";
import test from "node:test";

import { getCachedTextTranslation, putCachedTextTranslation, textTranslationCacheKey } from "../../src/app/runtime/textTranslationCache.ts";
import { defaultSettings } from "../../src/app/shared/state.ts";

test("Text cache depends on source and direction, not retired style or terminology", () => {
  const base = defaultSettings();
  const natural = textTranslationCacheKey(" halo ", base);
  assert.equal(natural, textTranslationCacheKey("halo", { ...base, translation_style: "formal" }));
  assert.equal(natural, textTranslationCacheKey("halo", {
    ...base, terminology: [{ indonesian: "arsip", english: "archive" }],
  }));
  assert.notEqual(natural, textTranslationCacheKey("halo", { ...base, source_language: "en", target_language: "id" }));
});

test("Text cache remains bounded to 32 in-memory LRU entries", () => {
  const settings = defaultSettings();
  const keys = Array.from({ length: 33 }, (_, index) => textTranslationCacheKey(`cache-${index}`, settings));
  for (let index = 0; index < keys.length; index += 1) putCachedTextTranslation({
    key: keys[index]!, translated: `translated-${index}`, reviewHints: [`hint-${index}`], needsReview: false,
  });
  assert.equal(getCachedTextTranslation(keys[0]!), null);
  const hit = getCachedTextTranslation(keys[32]!);
  assert.equal(hit?.translated, "translated-32");
  if (hit) hit.reviewHints.push("mutated");
  assert.deepEqual(getCachedTextTranslation(keys[32]!)?.reviewHints, ["hint-32"]);
});

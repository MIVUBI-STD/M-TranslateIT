import assert from "node:assert/strict";
import test from "node:test";

import { cloneSettings, defaultSettings } from "../../src/app/shared/state.ts";

test("runtime settings default spoken terms to an empty list", () => {
  const settings = defaultSettings();
  assert.equal(settings.schema_version, 11);
  assert.deepEqual(settings.spoken_terms, []);
});

test("cloning runtime settings deep-clones spoken term aliases", () => {
  const settings = defaultSettings();
  settings.spoken_terms = [{ term: "MIVUBI", aliases: ["mi vu bi"] }];
  const cloned = cloneSettings(settings);
  cloned.spoken_terms[0].aliases[0] = "changed";
  assert.equal(settings.spoken_terms[0].aliases[0], "mi vu bi");
});


test("runtime settings default translation style to natural", () => {
  const settings = defaultSettings();
  assert.equal(settings.translation_style, "natural");
});


test("runtime settings default noise suppression to auto", () => {
  const settings = defaultSettings();
  assert.equal(settings.audio.noise_suppression, "auto");
});

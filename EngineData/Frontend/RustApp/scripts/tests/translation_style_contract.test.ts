import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const settingsUi = readFileSync(new URL("../../src/components/settings/TranslationPreferences.svelte", import.meta.url), "utf8");
const textCommand = readFileSync(new URL("../../src-tauri/src/commands/text_translation.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");
const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");

test("translation style exposes only natural and formal product choices", () => {
  assert.match(settingsUi, />Natural</);
  assert.match(settingsUi, />Formal</);
  assert.doesNotMatch(settingsUi, /Creative|Funny|Academic|Casual/);
});

test("translation style reaches text and both meeting translation lanes", () => {
  assert.match(textCommand, /"translation_style": settings\.translation_style/);
  assert.match(outbound, /"translation_style": settings\.translation_style/);
  assert.match(incoming, /"translation_style": translation_style/);
});

test("deferred incoming work snapshots translation style with its language direction", () => {
  assert.match(deferred, /translation_style: String/);
  assert.match(incoming, /translation_style: translation_style\.to_string\(\)/);
  assert.match(incoming, /Some\(\(source_language, target_language, translation_style\)\)/);
});

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const meeting = source("src/pages/Meeting.svelte");
const textPage = source("src/pages/Text.svelte");
const settings = source("src/pages/Settings.svelte");
const native = source("src-tauri/src/commands/settings.rs");
const registry = source("src-tauri/src/commands/registry.rs");
const cache = source("src/app/runtime/textTranslationCache.ts");
const app = source("src/App.svelte");

test("Meeting presets cannot be applied through any UI or registered native command", () => {
  assert.doesNotMatch(native, /pub fn apply_meeting_preset|MeetingPresetApplyResult/);
  assert.doesNotMatch(registry, /apply_product_meeting_preset/);
  assert.doesNotMatch(meeting, /MeetingPresetPanel|meeting-presets/);
  assert.doesNotMatch(settings, /MeetingPresetPanel|Meeting presets/);
});

test("retired preference screens are not mounted", () => {
  assert.doesNotMatch(settings, /TranslationPreferences|Spoken terms|Preferred words|Translation style/);
  assert.match(settings, /Voice &amp; Audio/);
  assert.match(settings, /Meeting Captions/);
  assert.match(settings, /Help/);
});

test("Text is an ephemeral translate-copy workspace", () => {
  assert.match(textPage, /function clearWorkspace/);
  assert.match(textPage, /aria-label="Clear text"/);
  assert.match(textPage, /copyTranslation/);
  assert.doesNotMatch(textPage, /Undo Clear|More options|Try another wording|Floating caption|Report issue/);
  assert.doesNotMatch(textPage, /localStorage|sessionStorage|indexedDB/);
  assert.match(app, /class=\{route === "text" \? "" : "hidden"\}/);
  assert.match(cache, /MAX_ENTRIES = 32/);
});

test("Text results respect request identity and retain user edits", () => {
  assert.match(textPage, /targetRevision !== revision/);
  assert.match(textPage, /sourceText\.trim\(\) !== requestSource/);
  assert.match(textPage, /currentTextSettingsKey !== currentKey/);
  assert.match(textPage, /originalOutput\.trim\(\) \? originalOutput : originalInput/);
});

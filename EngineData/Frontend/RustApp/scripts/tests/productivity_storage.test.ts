import assert from "node:assert/strict";
import test from "node:test";

import {
  deleteMeetingPreset,
  meetingPresetFromSettings,
  readMeetingPresets,
  saveMeetingPreset,
} from "../../src/app/runtime/meetingPresetState.ts";
import {
  clearTranslationFeedback,
  readTranslationFeedback,
  saveTranslationFeedback,
} from "../../src/app/runtime/translationFeedbackState.ts";
import { defaultSettings } from "../../src/app/shared/state.ts";

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, String(value)); }
  removeItem(key: string) { this.values.delete(key); }
  clear() { this.values.clear(); }
}
const memory = new MemoryStorage();
Object.defineProperty(globalThis, "localStorage", { value: memory, configurable: true });

test("Meeting preset storage is bounded and drops malformed persisted entries", () => {
  memory.clear();
  localStorage.setItem("translateit.meeting-presets.v1", JSON.stringify([
    { id: "bad", name: "Bad only" },
    {
      id: "ok",
      name: "  Work  ",
      sourceLanguage: "id",
      targetLanguage: "en",
      listenSourceLanguage: "en",
      listenTargetLanguage: "id",
      translationStyle: "natural",
      microphoneId: null,
      meetingSoundId: null,
      noiseSuppression: "auto",
    },
  ]));
  assert.deepEqual(readMeetingPresets().map((entry) => entry.name), ["Work"]);

  const settings = defaultSettings();
  for (let index = 0; index < 7; index += 1) {
    saveMeetingPreset(meetingPresetFromSettings(`p-${index}`, `Preset ${index}`, settings));
  }
  assert.equal(readMeetingPresets().length, 6);
  deleteMeetingPreset("p-6");
  assert.equal(readMeetingPresets().some((entry) => entry.id === "p-6"), false);
});

test("Translation feedback is opt-in bounded, sanitizes malformed data, and clears", () => {
  memory.clear();
  localStorage.setItem("translateit.translation-feedback.v1", JSON.stringify([{ category: "bad" }]));
  assert.deepEqual(readTranslationFeedback(), []);

  for (let index = 0; index < 45; index += 1) {
    saveTranslationFeedback({
      category: "unnatural",
      source: `source-${index}`,
      translation: `translation-${index}`,
      sourceLanguage: "id",
      targetLanguage: "en",
    });
  }
  assert.equal(readTranslationFeedback().length, 40);
  clearTranslationFeedback();
  assert.equal(readTranslationFeedback().length, 0);
});

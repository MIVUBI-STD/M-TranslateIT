import assert from "node:assert/strict";
import test from "node:test";
import { clearTranslationFeedback, readTranslationFeedback, saveTranslationFeedback } from "../../src/app/runtime/translationFeedbackState.ts";

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, String(value)); }
  removeItem(key: string) { this.values.delete(key); }
  clear() { this.values.clear(); }
}
const memory = new MemoryStorage();
Object.defineProperty(globalThis, "localStorage", { value: memory, configurable: true });

test("legacy translation issue reports remain private and bounded until user clears them", () => {
  memory.clear();
  localStorage.setItem("translateit.translation-feedback.v1", JSON.stringify([{ category: "bad" }]));
  assert.deepEqual(readTranslationFeedback(), []);
  for (let index = 0; index < 45; index += 1) {
    assert.equal(saveTranslationFeedback({
      category: "unnatural", source: `source-${index}`, translation: `translation-${index}`,
      sourceLanguage: "id", targetLanguage: "en",
    }), true);
  }
  assert.equal(readTranslationFeedback().length, 40);
  assert.equal(clearTranslationFeedback(), true);
  assert.equal(readTranslationFeedback().length, 0);
});

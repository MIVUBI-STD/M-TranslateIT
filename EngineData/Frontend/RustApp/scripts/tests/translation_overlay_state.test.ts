import assert from "node:assert/strict";
import test from "node:test";

import {
  clearOverlayError,
  readLatestOverlayCaption,
  readOverlayDiagnostic,
  readOverlayPosition,
  readOverlayPreferences,
  recordOverlayError,
  updateOverlayPreferences,
  writeLatestOverlayCaption,
  writeOverlayPosition,
} from "../../src/app/runtime/translationOverlayState.ts";

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, String(value)); }
  removeItem(key: string) { this.values.delete(key); }
  clear() { this.values.clear(); }
}

const memory = new MemoryStorage();
Object.defineProperty(globalThis, "localStorage", { value: memory, configurable: true });

test("overlay preferences default safely and persist bounded choices", () => {
  memory.clear();
  assert.deepEqual(readOverlayPreferences(), { meetingEnabled: true, visibility: "expanded", textSize: "medium", width: "standard", contrast: "standard" });
  assert.deepEqual(updateOverlayPreferences({ meetingEnabled: false, visibility: "hidden", textSize: "large" }), {
    meetingEnabled: false, visibility: "hidden", textSize: "large", width: "standard", contrast: "standard",
  });
  assert.deepEqual(readOverlayPreferences(), { meetingEnabled: false, visibility: "hidden", textSize: "large", width: "standard", contrast: "standard" });
});

test("corrupt persisted preferences fail back to readable defaults", () => {
  memory.clear();
  memory.setItem("translateit.translationOverlay.preferences.v2", "{bad json");
  assert.deepEqual(readOverlayPreferences(), { meetingEnabled: true, visibility: "expanded", textSize: "medium", width: "standard", contrast: "standard" });
});

test("latest caption and physical position round trip through durable state", () => {
  memory.clear();
  const caption = { text: "Hello", language: "en", source: "text" as const, revision: "text:1" };
  writeLatestOverlayCaption(caption);
  writeOverlayPosition({ x: 120, y: -40 });
  assert.deepEqual(readLatestOverlayCaption(), caption);
  assert.deepEqual(readOverlayPosition(), { x: 120, y: -40 });
});

test("invalid physical position is ignored", () => {
  memory.clear();
  memory.setItem("translateit.translationOverlay.position.v2", JSON.stringify({ x: "bad", y: 10 }));
  assert.equal(readOverlayPosition(), null);
});

test("overlay diagnostics are bounded and clearable", () => {
  memory.clear();
  recordOverlayError("x".repeat(500));
  const diagnostic = readOverlayDiagnostic();
  assert.ok(diagnostic);
  assert.equal(diagnostic.message.length, 220);
  assert.match(diagnostic.occurredAt, /^\d{4}-\d{2}-\d{2}T/);
  clearOverlayError();
  assert.equal(readOverlayDiagnostic(), null);
});


test("legacy overlay preferences migrate to standard width and contrast", () => {
  localStorage.setItem(
    "translateit.translationOverlay.preferences.v2",
    JSON.stringify({ meetingEnabled: false, visibility: "collapsed", textSize: "large" }),
  );
  assert.deepEqual(readOverlayPreferences(), {
    meetingEnabled: false,
    visibility: "collapsed",
    textSize: "large",
    width: "standard",
    contrast: "standard",
  });
});

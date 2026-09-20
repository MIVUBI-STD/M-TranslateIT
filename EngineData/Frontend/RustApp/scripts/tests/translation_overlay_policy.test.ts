import assert from "node:assert/strict";
import test from "node:test";
import { bestWorkAreaForWindow, clampPositionToWorkArea, defaultBottomCenterPosition, intersectionArea, latestMeetingCaption, normalizeOverlayText, overlayFontSize, overlayHeightForTextSize } from "../../src/app/runtime/translationOverlayPolicy.ts";

test("floating caption normalizes text without inventing content", () => {
  assert.equal(normalizeOverlayText("  Hello world.  "), "Hello world.");
  assert.equal(normalizeOverlayText("\r\nLine one\r\nLine two\r\n"), "Line one\nLine two");
});
test("meeting caption selects newest committed translation with stable semantic revision", () => {
  assert.deepEqual(latestMeetingCaption({ ok: true, has_session: true, session_id: "session-a", turns: [{ sequence: 1, lane: "you", translated_text: "First" }, { sequence: 2, lane: "you", translated_text: "Second" }] }), { text: "Second", language: "en", source: "meeting", revision: "session-a:2" });
});
test("incoming translation is Indonesian and invalid snapshots stay silent", () => {
  assert.equal(latestMeetingCaption({ ok: true, has_session: true, session_id: "b", turns: [{ sequence: 7, lane: "incoming", translated_text: "Selamat pagi" }] })?.language, "id");
  assert.equal(latestMeetingCaption(null), null);
  assert.equal(latestMeetingCaption({ ok: false, has_session: true, session_id: "x", turns: [{ sequence: 1, lane: "you", translated_text: "No" }] }), null);
});
test("placement clamps the full caption rectangle inside work area", () => {
  const area = { x: 0, y: 0, width: 1920, height: 1040 };
  assert.deepEqual(clampPositionToWorkArea({ x: 1800, y: 1000 }, 620, 180, area), { x: 1300, y: 860 });
  assert.deepEqual(defaultBottomCenterPosition(620, 180, area, 72), { x: 650, y: 788 });
});
test("placement chooses only a work area intersected by caption", () => {
  const left = { x: 0, y: 0, width: 1920, height: 1040 }, right = { x: 1920, y: 0, width: 2560, height: 1400 };
  assert.equal(bestWorkAreaForWindow({ x: 2100, y: 100 }, 620, 180, [left, right]), right);
  assert.equal(bestWorkAreaForWindow({ x: 5000, y: 100 }, 620, 180, [left, right]), null);
});
test("readability presets stay bounded", () => {
  assert.equal(overlayFontSize("medium"), 22); assert.equal(overlayFontSize("extra-large"), 30);
  assert.equal(overlayHeightForTextSize("extra-large"), 240); assert.equal(overlayHeightForTextSize("extra-large", true), 64);
});

test("monitor overlap scoring prefers the display containing most of the caption", () => {
  const caption = { x: 1700, y: 100, width: 620, height: 180 };
  const left = { x: 0, y: 0, width: 1920, height: 1040 };
  const right = { x: 1920, y: 0, width: 2560, height: 1400 };
  assert.ok(intersectionArea(caption, right) > intersectionArea(caption, left));
});

import assert from "node:assert/strict";
import test from "node:test";
import { bestWorkAreaForWindow, clampPositionToWorkArea, defaultBottomCenterPosition, intersectionArea, latestMeetingCaption, normalizeOverlayText, overlayFontSize, overlayHeightForTextSize, overlayWidth, shouldAcceptOverlayCaption, captionHeightForContent, fitOverlayWidth, fitOverlayHeight, isProgrammaticOverlayMove } from "../../src/app/runtime/translationOverlayPolicy.ts";

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
  assert.equal(overlayWidth("compact"), 520); assert.equal(overlayWidth("standard"), 620); assert.equal(overlayWidth("wide"), 760);
});

test("monitor overlap scoring prefers the display containing most of the caption", () => {
  const caption = { x: 1700, y: 100, width: 620, height: 180 };
  const left = { x: 0, y: 0, width: 1920, height: 1040 };
  const right = { x: 1920, y: 0, width: 2560, height: 1400 };
  assert.ok(intersectionArea(caption, right) > intersectionArea(caption, left));
});

test("same-session late captions cannot overtake newer committed turns", () => {
  const latest = { text: "new", language: "en", source: "meeting" as const, revision: "s:8" };
  assert.equal(shouldAcceptOverlayCaption(latest, { ...latest, revision: "s:7" }), false);
  assert.equal(shouldAcceptOverlayCaption(latest, { ...latest }), false);
  assert.equal(shouldAcceptOverlayCaption(latest, { ...latest, revision: "s:9" }), true);
  assert.equal(shouldAcceptOverlayCaption(latest, { ...latest, revision: "other:1" }), true);
});

test("caption height uses measured content and preserves font size", () => {
  assert.equal(captionHeightForContent(20, "medium"), 180);
  assert.equal(captionHeightForContent(230, "medium"), 306);
  assert.equal(captionHeightForContent(10000, "medium"), 420);
  assert.equal(captionHeightForContent(999, "large", true), 64);
  assert.equal(captionHeightForContent(Number.NaN, "medium"), 180);
});

test("overlay width accounts for physical pixels at mixed DPI", () => {
  assert.equal(fitOverlayWidth(760, 1920, 1.5), 760);
  assert.equal(fitOverlayWidth(760, 900, 1.5), 568);
  assert.equal(fitOverlayWidth(760, 500, 1), 468);
  assert.equal(fitOverlayWidth(620, 1200, Number.NaN), 620);
});

test("work area height bounds long captions without lowering font size", () => {
  assert.equal(fitOverlayHeight(420, 450, 1), 418);
  assert.equal(fitOverlayHeight(420, 900, 2), 418);
  assert.equal(fitOverlayHeight(180, 800, 1.25), 180);
  assert.equal(fitOverlayHeight(420, 200, 2), 68);
  assert.equal(fitOverlayHeight(420, 0, 1), 420);
});

test("automatic overlay placement events do not overwrite intentional position", () => {
  assert.equal(isProgrammaticOverlayMove({ x: 650, y: 700 }, { x: 650, y: 700 }), true);
  assert.equal(isProgrammaticOverlayMove({ x: 649, y: 702 }, { x: 650, y: 700 }), true);
  assert.equal(isProgrammaticOverlayMove({ x: 550, y: 700 }, { x: 650, y: 700 }), false);
  assert.equal(isProgrammaticOverlayMove({ x: 650, y: 700 }, null), false);
});

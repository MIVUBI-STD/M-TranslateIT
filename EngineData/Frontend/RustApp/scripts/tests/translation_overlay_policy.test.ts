import assert from "node:assert/strict";
import test from "node:test";

import { latestMeetingCaption, normalizeOverlayText } from "../../src/app/runtime/translationOverlayPolicy.ts";

test("floating caption keeps readable text without inventing content", () => {
  assert.equal(normalizeOverlayText("  Hello world.  "), "Hello world.");
  assert.equal(normalizeOverlayText("\r\nLine one\r\nLine two\r\n"), "Line one\nLine two");
});

test("meeting caption selects the newest committed translation", () => {
  const caption = latestMeetingCaption({
    ok: true,
    has_session: true,
    session_id: "session-a",
    turns: [
      { sequence: 1, lane: "you", translated_text: "First", updated_unix_ms: 10 },
      { sequence: 2, lane: "you", translated_text: "Second", updated_unix_ms: 20 },
    ],
  });

  assert.deepEqual(caption, {
    text: "Second",
    language: "en",
    source: "meeting",
    revision: "session-a:2:20",
  });
});

test("incoming meeting translation is presented as Indonesian", () => {
  const caption = latestMeetingCaption({
    ok: true,
    has_session: true,
    session_id: "session-b",
    turns: [
      { sequence: 7, lane: "incoming", translated_text: "Selamat pagi", updated_unix_ms: 30 },
    ],
  });

  assert.equal(caption?.language, "id");
  assert.equal(caption?.text, "Selamat pagi");
});

test("meeting overlay stays silent without an authoritative committed caption", () => {
  assert.equal(latestMeetingCaption(null), null);
  assert.equal(latestMeetingCaption({
    ok: false,
    has_session: true,
    session_id: "session-c",
    turns: [{ sequence: 1, lane: "you", translated_text: "No", updated_unix_ms: 1 }],
  }), null);
  assert.equal(latestMeetingCaption({
    ok: true,
    has_session: false,
    session_id: null,
    turns: [],
  }), null);
});

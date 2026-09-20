import assert from "node:assert/strict";
import test from "node:test";

import { recentMeetingCaptions } from "../../src/app/runtime/translationOverlayPolicy.ts";

test("recent meeting captions are bounded, ordered, and translation-only", () => {
  const turns = Array.from({ length: 25 }, (_, index) => ({
    sequence: index + 1,
    lane: index % 2 === 0 ? "you" : "incoming",
    translated_text: ` translated ${index + 1} `,
    created_unix_ms: 1000 + index,
  }));
  const recent = recentMeetingCaptions({
    ok: true,
    has_session: true,
    session_id: "meeting-a",
    turns,
  }, 20);

  assert.equal(recent.length, 20);
  assert.equal(recent[0]?.sequence, 25);
  assert.equal(recent[19]?.sequence, 6);
  assert.equal(recent[0]?.text, "translated 25");
  assert.equal(recent[0]?.language, "en");
  assert.equal(recent[19]?.language, "id");
});

test("recent meeting captions stay empty without an authoritative session", () => {
  assert.deepEqual(recentMeetingCaptions(null), []);
  assert.deepEqual(recentMeetingCaptions({ ok: false, has_session: true, session_id: "x", turns: [] }), []);
  assert.deepEqual(recentMeetingCaptions({ ok: true, has_session: false, session_id: null, turns: [] }), []);
});

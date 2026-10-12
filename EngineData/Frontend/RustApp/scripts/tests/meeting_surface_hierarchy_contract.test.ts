import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const live = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const review = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");

test("Meeting has a single primary button that can Start, Stop, or guide recovery", () => {
  assert.match(meeting, /Indonesian speech → English voice/);
  assert.match(meeting, /meetingControlState\(/);
  assert.match(meeting, /disabled=\{!actionEnabled\}/);
  assert.match(meeting, /onclick=\{handlePrimary\}/);
  assert.match(meeting, /needsSetup \? "Finish Setup"/);
  assert.match(meeting, /needsVoice \? "Choose Voice"/);
  assert.equal((meeting.match(/onclick=\{/g) ?? []).length, 1);
  for (const retired of ["MeetingPresetPanel", "meeting-presets", "swapListenDirection",
    "detectMeetingApp", "Meeting options", "Mic Test"]) {
    assert.equal(meeting.includes(retired), false, retired);
  }
});
test("deferred setup has a real recovery path without creating another Meeting start", () => {
  assert.match(meeting, /async function continueSetup\(\)/);
  assert.match(meeting, /meeting_setup_state: "new"/);
  assert.match(meeting, /onNavigate\("settings"\)/);
  assert.match(meeting, /onNavigate\("my-voice"\)/);
  assert.match(meeting, /meeting\.canStop/);
});
test("live Meeting retains committed conversation and optional session review", () => {
  assert.match(meeting, /<MeetingActivity status=\{meetingStatus\} turns=\{meetingTurns\}/);
  assert.match(meeting, /<MeetingSessionReview/);
  assert.match(meeting, /meeting\.canStop \|\| meeting\.hasSession/);
});
test("Meeting Live reports local output truth, not fabricated participant delivery", () => {
  assert.match(live, /if \(status\.outbound\.output_active\)/);
  assert.match(live, /label: "Speaking"/);
  assert.match(live, /label: "Translating"/);
  assert.match(live, /label: "Listening"/);
  assert.match(live, /case "output_complete": return "Played"/);
  assert.match(live, /case "output_failed": return "Not played"/);
  assert.doesNotMatch(live, /"Sent"|"Delivered to participants"|"Sending"/);
  assert.match(live, /statusLabel=\{turn\.lane === "you" \? deliveryLabel\(turn\) : ""\}/);
});

test("Transcript tools follow user intent and preserve focus, reading position, and copy errors", () => {
  assert.match(live, /orderedTurns\.length > 1 \|\| searchOpen/);
  assert.match(live, /aria-expanded=\{searchOpen\}/);
  assert.match(live, /searchInput\?\.focus\(\)/);
  assert.match(live, /followingLatest/);
  assert.match(live, /onscroll=\{updateScrollPosition\}/);
  assert.match(live, />Jump to latest<\/button>/);
  assert.match(live, /copyFailedSequence = turn\.sequence/);
  assert.match(live, /Couldn't copy this phrase\. Try again\./);
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  assert.match(live, /<MeetingTurnBubble \{turn\}/);
  assert.match(bubble, /languageName\(turn\.source_language\)/);
  assert.match(bubble, /lang=\{turn\.translation_state === "translated" \? turn\.target_language : undefined\}/);
});

test("Last-session transcript is private-by-default and export remains explicit", () => {
  assert.match(review, /<details bind:open=\{reviewOpen\}/);
  assert.match(review, /reviewOpen = false/);
  assert.match(review, /getRecentMeetingTranscript\(\)/);
  assert.match(review, /Only this meeting is available until the next meeting or app exit/);
  assert.match(review, /<MeetingTranscriptExport hasSession=\{false\}/);
  assert.doesNotMatch(review, /\$effect\([\s\S]{0,250}exportMeetingTranscript\(/);
});

test("live and last-session transcript reuse one WhatsApp-style lane-aware bubble", () => {
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  assert.match(live, /<MeetingTurnBubble \{turn\}/);
  assert.match(review, /<MeetingTurnBubble \{turn\}/);
  assert.match(bubble, /own \? "justify-end" : "justify-start"/);
  assert.match(bubble, /meetingSpeakerLabel\(turn\.lane\)/);
  assert.match(bubble, /turn\.created_unix_ms/);
  assert.match(bubble, /Original/);
  assert.match(live, /Person 1 groups incoming audio/);
  assert.match(review, /Person 1 groups incoming audio/);
  assert.match(bubble, /Translation/);
  assert.doesNotMatch(bubble, /Person 2|speaker_id|diarizationResult/);
});

test("failed Meeting translation retains source and cannot look completed", () => {
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  const display = readFileSync(new URL("../../src/app/runtime/meetingTranscriptPresentation.ts", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url), "utf8");
  assert.match(bubble, /meetingTranslationDisplay\(turn\)/);
  assert.match(display, /Translation unavailable/);
  assert.match(native, /fn finish_translation_at\(/);
  assert.match(native, /translation_state = "failed"/);
  assert.match(native, /session\.phase != "live"/);
});

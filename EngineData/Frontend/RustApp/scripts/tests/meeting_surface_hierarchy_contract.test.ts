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
  assert.match(live, /Status of local playback, not confirmation from call participants/);
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
  assert.match(live, /languageName\(turn\.source_language\)/);
  assert.match(live, /lang=\{turn\.target_language\}/);
});

test("Last-session transcript is private-by-default and export remains explicit", () => {
  assert.match(review, /<details bind:open=\{reviewOpen\}/);
  assert.match(review, /reviewOpen = false/);
  assert.match(review, /getRecentMeetingTranscript\(\)/);
  assert.match(review, /temporary until a new Meeting starts or the app closes/);
  assert.match(review, /<MeetingTranscriptExport hasSession=\{false\}/);
  assert.doesNotMatch(review, /\$effect\([\s\S]{0,250}exportMeetingTranscript\(/);
});

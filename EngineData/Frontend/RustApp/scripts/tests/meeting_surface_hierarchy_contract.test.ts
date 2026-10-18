import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const live = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const review = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");
const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");

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
test("Meeting keeps a single accessible Stop or Retry Stop above live transcript", () => {
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? primaryControl\.label : "Stop Microphone"/);
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? "warning"/);
  const action = meeting.indexOf('aria-label="Meeting main action"');
  const sourceSelector = meeting.indexOf('for="meeting-audio-source"');
  const transcript = meeting.indexOf("<MeetingActivity status={meetingStatus}");
  assert.ok(action > 0 && action < sourceSelector && sourceSelector < transcript,
    "Stop/Retry Stop must remain before the scrollable conversation");
  assert.equal((meeting.match(/onclick=\{handlePrimary\}/g) ?? []).length, 1);
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
  assert.match(live, /case "output_complete": return "Local playback complete"/);
  assert.match(live, /case "output_failed": return "Not played"/);
  assert.doesNotMatch(live, /"Sent"|"Delivered to participants"|"Sending"/);
  assert.match(live, /statusLabel=\{turn\.lane === "you" \? deliveryLabel\(turn\) : ""\}/);
});

test("Meeting exposes bounded speech loss without falsely attributing a speaker", () => {
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/session_state.rs", import.meta.url), "utf8");
  assert.match(native, /status\.overflow_dropped_utterance_count = overflow_dropped_utterance_count\(\)/);
  assert.match(native, /status\.evicted_pending_utterance_count = evicted_pending_utterance_count\(\)/);
  assert.match(live, /status\.outbound\.overflow_dropped_utterance_count/);
  assert.match(live, /status\.outbound\.evicted_pending_utterance_count/);
  assert.match(live, /\{#if missingSpeech\}/);
  assert.match(live, /counts combine your microphone and incoming meeting audio/);
  assert.match(live, /Important details may need repeating/);
});

test("Live transcript stays minimal while preserving reading position and copy errors", () => {
  assert.doesNotMatch(live, /searchOpen|toggleSearch|togglePin|pinnedSequences|PinOff/);
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

test("global notices expose full text without noninteractive keyboard tabindex", () => {
  assert.doesNotMatch(header, /flex-1 truncate text-right/);
  assert.match(header, /max-h-16 flex-1 overflow-y-auto break-words/);
  assert.match(header, /role="status" aria-live="polite" title=\{notice\}/);
});

test("Last-session transcript is private-by-default and export remains explicit", () => {
  assert.match(review, /<details bind:open=\{reviewOpen\}/);
  assert.match(review, /reviewOpen = false/);
  assert.match(review, /getRecentMeetingTranscript\(\)/);
  assert.match(review, /Saved Transcripts below keeps recorded sessions until you delete them/);
  assert.doesNotMatch(review, /<MeetingTranscriptExport/);
  assert.match(meeting, /<MeetingTranscriptExport hasSession=\{meeting\.hasSession\}/);
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
  assert.match(live, /Meeting audio is combined; individual speakers are not identified/);
  assert.match(review, /Meeting audio combines incoming sound; individual speakers are not identified/);
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

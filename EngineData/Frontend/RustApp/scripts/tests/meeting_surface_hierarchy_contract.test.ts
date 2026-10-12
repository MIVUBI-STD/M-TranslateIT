import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const live = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const review = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");

test("Meeting Ready remains one task-first surface, with outbound direction independent of Text", () => {
  assert.match(meeting, /const outboundSourceName = "Indonesian"/);
  assert.match(meeting, /const outboundTargetName = "English"/);
  assert.match(meeting, /Microphone<\/span>/);
  assert.match(meeting, /Meeting voice/);
  assert.match(meeting, /disabled=\{!primaryControl\.enabled\}/);
  assert.match(meeting, />\{primaryControl\.label\}<\/button>/);
  assert.doesNotMatch(meeting, /grid-cols-3 divide-x|<StatusBadge label=\{audioQuality/);
  assert.match(meeting, /lg:grid-cols-2/);
});

test("Meeting options preserve real controls but are disclosed on demand, including Quick Actions focus", () => {
  assert.match(meeting, /<details bind:open=\{optionsOpen\}/);
  assert.match(meeting, /optionsOpen = true;\s*void tick\(\)\.then/);
  assert.match(meeting, /id="utility-meeting-presets"/);
  assert.match(meeting, /id="utility-meeting-transcript"/);
  assert.match(meeting, /<MeetingPresetPanel/);
  assert.match(meeting, /<MeetingTranscriptExport/);
  assert.match(meeting, /Translate what you hear:/);
  assert.match(meeting, /classifyVirtualMicRoute\(routeStatus\)/);
  assert.match(meeting, /routeCheck === "unavailable"/);
  assert.match(meeting, /cannot verify your calling app's input selection/);
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

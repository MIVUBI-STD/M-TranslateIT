import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { buildProductCommands } from "../../src/app/runtime/productCommandRegistry.ts";

const read = (relative: string): string => readFileSync(new URL(`../../src/${relative}`, import.meta.url), "utf8");
const commandsSource = read("app/runtime/productCommandRegistry.ts");
const settings = read("pages/Settings.svelte");
const meeting = read("pages/Meeting.svelte");
const palette = read("components/runtime/CommandPalette.svelte");

function commands(start: boolean, stop: boolean, label: string) {
  return buildProductCommands({
    meetingCanStart: start, meetingCanStop: stop, meetingActionLabel: label,
    navigationLocked: false, navigate: () => {}, openUtility: () => {}, toggleMeeting: () => {},
  });
}

test("keyboard command palette contains only navigation, Meeting, and Settings utilities", () => {
  assert.deepEqual(commands(true, false, "Start Translation").map(x => x.id), [
    "navigate-meeting", "navigate-text", "navigate-my-voice", "navigate-settings",
    "meeting-toggle", "meeting-microphone", "meeting-captions", "diagnostics",
  ]);
  assert.doesNotMatch(commandsSource, /meeting-presets|spoken-terms|preferred-words|quick-translate-clipboard/);
  assert.match(settings, /aria-label="Settings categories"/);
  assert.match(settings, /Voice &amp; Audio/);
  assert.match(settings, /Meeting Captions/);
  assert.doesNotMatch(settings, /TranslationPreferences/);
  assert.doesNotMatch(meeting, /<MeetingPresetPanel/);
});

test("meeting start and stop intent cannot invert through keyboard commands", () => {
  for (const [start, stop, label, expected, forbidden] of [
    [true, false, "Start Translation", "start", "stop"],
    [false, true, "Stop Translation", "stop", "start"],
    [false, false, "Stopping...", "status", "start"],
  ] as const) {
    const action = commands(start, stop, label).find(x => x.id === "meeting-toggle");
    assert.ok(action);
    assert.equal(action.keywords.includes(expected), true);
    assert.equal(action.keywords.includes(forbidden), false);
    assert.equal(Boolean(action.disabled), !start && !stop);
  }
});

test("keyboard search remains dismissible and restores focus", () => {
  for (const token of ["onInputKeydown", "onResultKeydown", "onDialogKeydown", "previousFocus",
    "command.handsOffFocus", "restore.focus()", "event.key === \"Escape\""]) {
    assert.ok(palette.includes(token), token);
  }
});

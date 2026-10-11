import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (relative: string) => readFileSync(new URL(`../../src/${relative}`, import.meta.url), "utf8");
const commands = source("app/runtime/productCommandRegistry.ts");
const palette = source("components/runtime/CommandPalette.svelte");
const app = source("App.svelte");
const settings = source("pages/Settings.svelte");
const preferences = source("components/settings/TranslationPreferences.svelte");
const feedback = source("components/settings/TranslationFeedbackReview.svelte");
const presets = source("components/meeting/MeetingPresetPanel.svelte");

test("Quick Actions locates canonical tools without adding background watchers or new runtime owners", () => {
  for (const id of ["meeting-presets", "meeting-transcript", "floating-captions", "spoken-terms",
    "preferred-words", "translation-feedback", "meeting-microphone", "diagnostics"]) {
    assert.match(commands, new RegExp(`"${id}"`));
  }
  assert.match(commands, /openUtility/);
  assert.doesNotMatch(commands, /localStorage|setInterval|addEventListener/);
  assert.match(app, /utilityRequest \+= 1/);
  assert.match(settings, /utilityTarget/);
  assert.doesNotMatch(settings, /if \(tab === "translation"\) return;/);
  assert.match(commands, /context\.meetingHasSession/);
  assert.match(commands, /settings-spoken-terms/);
  assert.match(commands, /settings-preferred-words/);
  assert.match(preferences, /utility-spoken-terms/);
  assert.match(preferences, /utility-preferred-words/);
});

test("Quick Actions keyboard activation follows native focused buttons and restores focus", () => {
  assert.match(palette, /onInputKeydown/);
  assert.match(palette, /onResultKeydown/);
  assert.match(palette, /onDialogKeydown/);
  assert.match(palette, /event\.key !== "Tab"/);
  assert.match(palette, /event\.key === "Escape"/);
  assert.match(palette, /previousFocus/);
  assert.match(palette, /command\.handsOffFocus/);
  assert.match(palette, /restore\.focus\(\)/);
  assert.match(palette, /onclick=\{\(\) => void run\(command\)\}/);
});

test("destructive utility actions have reversible decision before persistent mutation", () => {
  assert.match(presets, /pendingRemovalId !== id/);
  assert.match(presets, /Remove preset</);
  assert.match(preferences, /pendingDelete/);
  assert.match(preferences, /Promise<boolean>/);
  assert.match(preferences, /Preferred words couldn't be saved\. Try again\."\);\s*return false;/);
  assert.match(preferences, /Spoken terms couldn't be saved\. Try again\."\);\s*return false;/);
  assert.match(preferences, /if \(await persistSpokenTerms/);
  assert.match(preferences, /if \(await persist\(/);
  assert.match(preferences, /Remove this spoken term/);
  assert.match(preferences, /Remove this pair/);
  assert.match(feedback, /confirmClear/);
  assert.match(feedback, /Clear reports/);
});

test("Meeting command searches never invert Start and Stop actions", async () => {
  const { buildProductCommands } = await import("../../src/app/runtime/productCommandRegistry.ts");
  const scenarios = [
    { meetingHasSession: false, meetingCanStart: true, meetingCanStop: false,
      expected: "Start Translation", included: "start", excluded: "stop", disabled: false },
    { meetingHasSession: true, meetingCanStart: false, meetingCanStop: true,
      expected: "Stop Translation", included: "stop", excluded: "start", disabled: false },
    { meetingHasSession: true, meetingCanStart: false, meetingCanStop: false,
      expected: "Stop Translation", included: "stop", excluded: "start", disabled: true },
  ] as const;

  for (const scenario of scenarios) {
    const commands = buildProductCommands({
      meetingHasSession: scenario.meetingHasSession,
      meetingCanStart: scenario.meetingCanStart,
      meetingCanStop: scenario.meetingCanStop,
      navigationLocked: false,
      quickTranslateReady: true,
      navigate: () => {},
      openUtility: () => {},
      toggleMeeting: () => {},
      quickTranslateClipboard: () => {},
    });
    const action = commands.find(command => command.id === "meeting-toggle");
    assert.ok(action);
    assert.equal(action.label, scenario.expected);
    assert.equal(action.keywords.includes(scenario.included), true);
    assert.equal(action.keywords.includes(scenario.excluded), false);
    assert.equal(Boolean(action.disabled), scenario.disabled);
  }
});

test("manual Settings tab changes consume stale Quick Actions focus intent", () => {
  assert.match(settings, /let activeUtility = \$state<UtilityDestination \| null>\(null\)/);
  assert.match(settings, /utilityTarget=\{activeUtility\}/);
  for (const tab of ["meeting", "translation", "advanced"]) {
    assert.ok(settings.includes(`activeUtility = null; tab = "${tab}"`));
  }
});

test("term removal retains confirmation when storage rejects a delete", () => {
  assert.match(preferences, /if \(await removeSpokenTerm\(index\)\) pendingDelete = null/);
  assert.match(preferences, /if \(await removeTerm\(index\)\) pendingDelete = null/);
  assert.match(preferences, /col-span-2 flex flex-wrap/);
  assert.match(preferences, /col-span-4 flex flex-wrap/);
});

test("Meeting preset capacity and translation preference mutations stay protected", () => {
  const storageSource = source("app/runtime/meetingPresetState.ts");
  assert.match(storageSource, /existing\.length >= MAX_MEETING_PRESETS/);
  assert.doesNotMatch(storageSource, /\[preset, \.\.\.existing\]\.slice/);
  assert.match(presets, /presets\.length >= MAX_MEETING_PRESETS/);
  assert.match(preferences, /const preferencesBusy = \$derived\(saving \|\| styleSaving\)/);
  assert.match(preferences, /if \(preferencesBusy\) return false/);
  assert.match(preferences, /if \(preferencesBusy \|\| settings\.translation_style === style\) return/);
});

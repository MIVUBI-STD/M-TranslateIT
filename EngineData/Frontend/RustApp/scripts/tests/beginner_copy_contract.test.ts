import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");

const primarySurfaces = [
  "src/App.svelte",
  "src/pages/FirstSetup.svelte",
  "src/pages/Meeting.svelte",
  "src/pages/Text.svelte",
  "src/pages/MyVoice.svelte",
  "src/components/my-voice/BuiltinVoicePicker.svelte",
  "src/components/my-voice/QuickVoicePreview.svelte",
  "src/components/my-voice/MyVoiceBuildSummary.svelte",
  "src/components/setup/SetupVoiceChoice.svelte",
  "src/components/settings/TranslationStylePicker.svelte",
  "src/components/meeting/MeetingActivity.svelte",
  "src/components/my-voice/MyVoiceBuild.svelte",
  "src/components/setup/SetupNavigation.svelte",
  "src/components/settings/TranslationPreferences.svelte",
  "src/components/layout/Sidebar.svelte",
];

const forbiddenUserFacingPhrases = [
  "check Diagnostics",
  "open Diagnostics",
  "Meeting microphone",
  "canonical model",
  "release asset",
  "provider ready",
  "frontend/Tauri",
];

test("primary UI copy stays beginner friendly", () => {
  for (const relativePath of primarySurfaces) {
    const source = readFileSync(resolve(root, relativePath), "utf8");
    for (const phrase of forbiddenUserFacingPhrases) {
      assert.equal(
        source.toLowerCase().includes(phrase.toLowerCase()),
        false,
        `${relativePath} must not expose technical copy: ${phrase}`,
      );
    }
  }
});

test("voice and Meeting status use familiar labels without claiming delivery into the calling app", () => {
  const myVoice = readFileSync(resolve(root, "src/pages/MyVoice.svelte"), "utf8");
  const build = readFileSync(resolve(root, "src/components/my-voice/MyVoiceBuild.svelte"), "utf8");
  const quickPreview = readFileSync(resolve(root, "src/components/my-voice/QuickVoicePreview.svelte"), "utf8");
  const buildSummary = readFileSync(resolve(root, "src/components/my-voice/MyVoiceBuildSummary.svelte"), "utf8");
  const activity = readFileSync(resolve(root, "src/components/meeting/MeetingActivity.svelte"), "utf8");
  const sidebar = readFileSync(resolve(root, "src/components/layout/Sidebar.svelte"), "utf8");
  assert.ok(myVoice.includes('<h2 class="ti-page-title">My Voice</h2>'));
  assert.ok(myVoice.includes("Review this recording"));
  assert.ok(build.includes("<QuickVoicePreview"));
  assert.ok(quickPreview.includes("Quick Preview · Not for meetings"));
  assert.ok(buildSummary.includes("Retry status"));
  assert.ok(activity.includes("TranslateIT is sending translated audio to the selected call microphone."));
  assert.ok(!activity.includes("Other people in the call are hearing"));
  assert.ok(sidebar.includes("aria-label={item.label}"));
});

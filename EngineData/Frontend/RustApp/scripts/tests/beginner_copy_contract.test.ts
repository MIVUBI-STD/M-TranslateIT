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

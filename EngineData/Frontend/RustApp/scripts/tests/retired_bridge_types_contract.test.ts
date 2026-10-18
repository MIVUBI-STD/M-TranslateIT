import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) =>
  readFileSync(new URL("../../" + path, import.meta.url), "utf8");

test("retired Meeting Presets and Quick Translation are not required bridge response types", () => {
  const validator = source("scripts/validate_bridge_contract.mjs");
  const settingsRust = source("src-tauri/src/commands/settings.rs");
  const translationRust = source("src-tauri/src/commands/text_translation.rs");
  const frontend = source("src/app/bridge/runtimeApi.ts");

  // A retired feature must not force its deleted DTO back into native code.
  for (const name of ["MeetingPresetApplyResult", "QuickTranslationResult", "QuickTranslationCommandResult"]) {
    assert.equal(validator.includes(`"${name}"`), false, `retired contract: ${name}`);
  }
  assert.equal(settingsRust.includes("pub struct MeetingPresetApplyResult"), false);
  assert.equal(translationRust.includes("pub struct QuickTranslationResult"), false);
  assert.equal(frontend.includes("export type MeetingPresetApplyResult"), false);
  assert.equal(frontend.includes("export type QuickTranslationCommandResult"), false);

  // Preserve parity checks for the active device-selection and Text APIs.
  assert.ok(validator.includes('"AudioDeviceSelectionResult"'));
  assert.ok(validator.includes('"TextTranslationResult"'));
  assert.ok(settingsRust.includes("pub struct AudioDeviceSelectionResult"));
  assert.ok(translationRust.includes("pub struct TextTranslationResult"));
  assert.ok(frontend.includes("export type AudioDeviceSelectionCommandResult"));
  assert.ok(frontend.includes("export type TextTranslationCommandResult"));
});

test("bridge validator isolates inline DTO fields instead of reading the next type", () => {
  const validator = source("scripts/validate_bridge_contract.mjs");
  const frontend = source("src/app/bridge/runtimeApi.ts");
  const archive = source("src-tauri/src/commands/meeting_session/transcript_archive.rs");
  const match = frontend.match(/export type SavedMeetingActionResult\s*=\s*\{([^{}]*)\};/);
  assert.ok(match);
  const fields = [...match[1].matchAll(/(?:^|[\n;])\s*([A-Za-z_][A-Za-z0-9_]*)\??\s*:/g)]
    .map((value) => value[1]).sort();
  assert.deepEqual(fields, ["message", "ok"]);
  assert.ok(archive.includes("pub struct ArchiveActionResult"));
  assert.ok(validator.includes("([^{}]*)"), "isolates the correct DTO closing brace");
  assert.ok(validator.includes("[\\n;]"), "supports inline and multiline fields");
  assert.ok(validator.includes('"ArchiveActionResult"'));
  assert.ok(validator.includes('"SavedMeetingActionResult"'));
});

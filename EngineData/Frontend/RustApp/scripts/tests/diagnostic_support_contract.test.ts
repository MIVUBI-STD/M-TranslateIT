import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const support = readFileSync(new URL("../../src-tauri/src/commands/diagnostic_support.rs", import.meta.url), "utf8");

test("support bundle is explicit local export with privacy whitelist", () => {
  assert.match(support, /SavedProject|user_saved_dir/);
  assert.match(support, /contains_transcript.*false/);
  assert.match(support, /contains_audio.*false/);
  assert.match(support, /contains_voice_reference.*false/);
  assert.doesNotMatch(support, /"source_text"\s*:|"translated_text"\s*:|"turns"\s*:/);
});

test("support bundle omits raw local identifiers", () => {
  assert.doesNotMatch(support, /stderr_log_path/);
  assert.doesNotMatch(support, /selected_output_device/);
  assert.doesNotMatch(support, /selected_input_device/);
  assert.doesNotMatch(support, /"session_id"\s*:/);
  assert.match(support, /sanitize_diagnostic_text/);
});

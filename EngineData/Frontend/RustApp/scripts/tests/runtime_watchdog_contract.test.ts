import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const watchdog = readFileSync(new URL("../../src-tauri/src/commands/runtime_watchdog.rs", import.meta.url), "utf8");
const diagnostics = readFileSync(new URL("../../src/components/settings/RuntimeReliabilityDiagnostics.svelte", import.meta.url), "utf8");

test("watchdog has no background loop or automatic restart authority", () => {
  assert.doesNotMatch(watchdog, /thread::spawn|setInterval|start_helper_bridge|start_meeting_translation/);
  assert.match(watchdog, /get_runtime_watchdog_status/);
});

test("watchdog ignores old timestamps while Meeting is simply listening", () => {
  assert.match(watchdog, /"transcribing" \| "translating"/);
  assert.match(watchdog, /"synthesizing"/);
  assert.match(watchdog, /"delivering"/);
  assert.doesNotMatch(watchdog, /"listening" => Some/);
});

test("watchdog thresholds stay above normal worker deadlines", () => {
  assert.match(watchdog, /WORKER_INFERENCE_RESPONSE_DEADLINE_MS \+ WATCHDOG_GRACE_MS/);
  assert.match(watchdog, /WORKER_SYNTHESIS_RESPONSE_DEADLINE_MS \+ WATCHDOG_GRACE_MS/);
});

test("reliability diagnostics surfaces watchdog and crash recovery together", () => {
  assert.match(diagnostics, /getRuntimeWatchdogStatus/);
  assert.match(diagnostics, /getStartupRecoveryStatus/);
});

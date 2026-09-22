import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const intents = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/intents.rs", import.meta.url),
  "utf8",
);

test("ApplicationRuntime owns helper bootstrap and recovery eligibility", () => {
  assert.match(intents, /fn helper_state_allows_start\(state: &str\)/);
  assert.match(intents, /matches!\(state, "not_started" \| "stopped"\)/);
  assert.match(intents, /"ensure_runtime_ready" => ensure_runtime_ready\(\)/);
  assert.match(intents, /"fix_setup" => fix_setup\(\)/);
});

test("helper lifecycle policy is not duplicated in frontend state helpers", () => {
  assert.doesNotMatch(intents, /frontend/);
});

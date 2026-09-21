import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const incident = readFileSync(new URL("../../src-tauri/src/commands/incident_log.rs", import.meta.url), "utf8");
const sessionState = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/session_state.rs", import.meta.url), "utf8");

test("incident history is bounded, redacted and excludes conversation payloads", () => {
  assert.match(incident, /MAX_INCIDENTS: usize = 20/);
  assert.match(incident, /sanitize_diagnostic_text/);
  assert.doesNotMatch(incident, /source_text|translated_text|audio_path/);
});

test("repeated identical incidents are deduplicated at the newest slot", () => {
  assert.match(incident, /incidents\.first\(\)\.is_some_and/);
  assert.match(incident, /incidents\[0\] = next/);
});

test("required and optional meeting failures record incidents at their source", () => {
  assert.match(sessionState, /record_runtime_incident\("meeting_outbound"/);
  assert.match(sessionState, /record_runtime_incident\("meeting_incoming"/);
});

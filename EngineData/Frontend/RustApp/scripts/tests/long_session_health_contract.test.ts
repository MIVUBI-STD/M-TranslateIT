import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const health = readFileSync(new URL("../../src-tauri/src/commands/long_session_health.rs", import.meta.url), "utf8");
const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");

test("long-session health observes bounded pressure without automatic cleanup", () => {
  assert.match(health, /outbound_overflow_dropped/);
  assert.match(health, /deferred_incoming_depth/);
  assert.match(health, /transcript_dropped_turns/);
  assert.match(health, /meeting_temp_file_count/);
  assert.doesNotMatch(health, /remove_file|clear_deferred_incoming_queue|stop_meeting_translation/);
});

test("deferred incoming exposes counts without exposing queued transcript content", () => {
  assert.match(deferred, /deferred_incoming_health_counts/);
  assert.match(deferred, /DEFERRED_DROPPED_OVERFLOW/);
  assert.match(deferred, /DEFERRED_DROPPED_STALE/);
});


test("long-session health uses count-only transcript and incident metadata", () => {
  assert.match(health, /committed_turn_health_counts/);
  assert.match(health, /runtime_incident_count/);
  assert.doesNotMatch(health, /get_meeting_committed_turns|get_recent_runtime_incidents/);
});

test("meeting start resets pressure counters for the new session", () => {
  const lifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
  assert.match(lifecycle, /reset_finalized_drop_counters\(\)/);
  assert.match(lifecycle, /reset_deferred_incoming_drop_counters\(\)/);
});

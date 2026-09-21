import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const lifecycle = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url),
  "utf8",
);

test("pre-Live rollback has one complete resource owner", () => {
  assert.match(lifecycle, /fn rollback_starting_meeting_resources/);
  assert.match(lifecycle, /cancel_meeting_output_for_generation\(generation\)/);
  assert.match(lifecycle, /stop_live_capture_runtime\(\)/);
  assert.match(lifecycle, /stop_meeting_sound_capture_runtime\(\)/);
  assert.match(lifecycle, /cancel_helper_bridge_meeting_session\(session_id\)/);
  assert.match(lifecycle, /stop_meeting_outbound_consumer\(generation\)/);
  assert.match(lifecycle, /clear_prepared_virtual_mic_route_selection\(\)/);
  assert.match(lifecycle, /clear_runtime_session_if_generation\(generation\)/);
});

test("Live commit failure uses the canonical rollback instead of a partial cleanup copy", () => {
  const start = lifecycle.indexOf("let committed = commit_application_meeting_session_live");
  const end = lifecycle.indexOf("update_outbound_status(", start);
  const block = lifecycle.slice(start, end);
  assert.match(block, /rollback_starting_meeting_resources\(generation, &session_id\)/);
  assert.match(block, /Helper cleanup:/);
  assert.doesNotMatch(block, /stop_live_capture_runtime\(\)/);
  assert.doesNotMatch(block, /stop_meeting_outbound_consumer\(generation\)/);
});

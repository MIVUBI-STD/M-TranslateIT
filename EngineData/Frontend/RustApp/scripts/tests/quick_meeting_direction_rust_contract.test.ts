import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");
const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");

test("incoming direction is snapshotted per utterance and survives deferral", () => {
  assert.match(incoming, /meeting_listen_source_language/);
  assert.match(incoming, /meeting_listen_target_language/);
  assert.match(deferred, /source_language: String/);
  assert.match(deferred, /target_language: String/);
  assert.match(incoming, /deferred_direction/);
});

test("outbound voice remains explicitly Indonesian to English", () => {
  assert.match(outbound, /"source_language": "id"/);
  assert.match(outbound, /"target_language": "en"/);
  assert.match(outbound, /"id",\s*"en",\s*&transcript/);
});

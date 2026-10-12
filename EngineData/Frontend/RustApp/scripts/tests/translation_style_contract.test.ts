import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const text = readFileSync(new URL("../../src-tauri/src/commands/text_translation.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");

test("retired translation style and terminology never enter production model requests", () => {
  for (const source of [text, outbound, incoming]) {
    assert.doesNotMatch(source, /"translation_style":|\"terminology\":|\"context_pairs\":/);
  }
});

test("retired spoken terms never bias production Meeting ASR requests", () => {
  for (const source of [outbound, incoming]) {
    assert.doesNotMatch(source, /"hotwords":|asr_hotwords\(\)/);
  }
});

test("retired translation style never enters incoming deferred queue or replay", () => {
  const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");
  assert.doesNotMatch(incoming, /translation_style/);
  assert.doesNotMatch(deferred, /translation_style/);
  assert.match(incoming, /deferred_direction: Option<\(String, String\)>/);
  assert.match(deferred, /MAX_DEFERRED_INCOMING_AGE_MS: u64 = 20_000/);
});

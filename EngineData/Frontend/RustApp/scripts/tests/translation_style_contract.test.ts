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

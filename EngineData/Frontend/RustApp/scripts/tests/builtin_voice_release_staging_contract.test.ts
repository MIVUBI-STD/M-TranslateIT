import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const stage = readFileSync(
  new URL("../stage_release_inputs_impl.ps1", import.meta.url), "utf8",
);
const validator = readFileSync(
  new URL("../validate_release_payload.mjs", import.meta.url), "utf8",
);
const sources = JSON.parse(readFileSync(
  new URL("../../../../Backend/RuntimeAssets/Voice/BuiltInVoices/SOURCES.json", import.meta.url),
  "utf8",
));

test("built-in voice release stage verifies tracked files rather than copying into themselves", () => {
  assert.doesNotMatch(stage, /Copy-Tree \(Join-Path \$RepoRoot 'EngineData\\Backend\\RuntimeAssets\\Voice\\BuiltInVoices'\)/);
  assert.match(stage, /Verify repository-owned built-in voice references/);
  assert.match(stage, /\$builtInRecord\.schema -ne 'translateit\.builtin_voice_sources\.v1'/);
  assert.match(stage, /Compare-Object \(\$expectedVoiceIds \| Sort-Object\) \(\$actualVoiceIds \| Sort-Object\)/);
  assert.match(stage, /Assert-Hash \$reference \(\[string\]\$voice\.wav_sha256\)/);
  assert.match(stage, /BuiltInVoices attribution missing/);
});

test("release payload rejects extra or duplicate voice identities and invalid source digests", () => {
  assert.match(validator, /builtin_voice_sources_inventory_mismatch/);
  assert.match(validator, /new Set\(voices\.map\(\(voice\) => voice\.voice_id\)\)\.size !== 2/);
  assert.match(validator, /builtin_voice_invalid_sha256/);
  assert.match(validator, /builtin_voice_hash_mismatch/);
  assert.match(validator, /requireFile\(path, \`BuiltInVoices\/\$\{voice\.voice_id\}\/reference\.wav\`\)/);
});

test("canonical built-in voice source inventory has two approved IDs and digest shapes", () => {
  assert.equal(sources.schema, "translateit.builtin_voice_sources.v1");
  assert.deepEqual(sources.voices.map((voice: { voice_id: string }) => voice.voice_id).sort(), ["FemaleVoice", "MaleVoice"]);
  for (const voice of sources.voices) {
    assert.match(voice.wav_sha256, /^[a-f0-9]{64}$/);
    assert.equal(voice.license, "CC-BY-4.0");
  }
});

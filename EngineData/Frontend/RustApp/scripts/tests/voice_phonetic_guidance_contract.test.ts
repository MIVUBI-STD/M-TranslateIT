import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (path: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + path), "utf8");
const corpusDir = "voice_lab_recording/";
const files = ["guided_lines.rs", "guided_corpus_short.rs", "guided_corpus_natural.rs", "guided_corpus_detailed.rs"];
const lines = files.flatMap((filename, i) => {
  const raw = rust(corpusDir + filename);
  const regex = i === 0
    ? /^\s*\((\d+),\s*("(?:[^"\\]|\\.)*")\),?$/gm
    : /^\s*\((\d+),\s*("(?:[^"\\]|\\.)*"),\s*"[^"]+",\s*"[^"]+"\),?$/gm;
  return [...raw.matchAll(regex)].map(m => ({ id: Number(m[1]), text: JSON.parse(m[2]) as string }));
});
const inventory = rust(corpusDir + "guided_phonetic_inventory.rs");
const selector = rust(corpusDir + "guided_phonetic_guidance.rs");
const rows = [...inventory.matchAll(/^\s*\((0x[0-9a-f]{16}),\s*(0x[0-9a-f]{16}),\s*(0x[0-9a-f]{16}),\s*(\d{1,4})\),\s*\/\/ line (\d+)$/gm)];

function fnv64(text: string) {
  let v = 0xcbf29ce484222325n;
  for (const byte of Buffer.from(text, "utf8")) {
    v = BigInt.asUintN(64, (v ^ BigInt(byte)) * 0x100000001b3n);
  }
  return "0x" + v.toString(16).padStart(16, "0");
}

test("dictionary hints are bound to exact current prompt text, including Unicode", () => {
  assert.equal(lines.length, 600);
  assert.equal(rows.length, 600);
  for (let i = 0; i < 600; i++) {
    const [, fingerprint, phones, transitions, ratio, id] = rows[i];
    assert.equal(Number(id), lines[i].id);
    assert.equal(fingerprint, fnv64(lines[i].text),
      "stale phonetic hints for line " + id + "; regenerate rather than applying to different text");
    assert.ok(Number(ratio) >= 0 && Number(ratio) <= 1000);
    assert.ok(BigInt(phones) >= 0 && BigInt(transitions) >= 0);
  }
  assert.match(inventory, /PHONE_LABELS: \[&str; 39\]/);
  assert.match(inventory, /PAIR_LABELS: \[&str; 64\]/);
  assert.match(inventory, /CMUdict source GitHub blob 2c0411740cce3e2026a80b90b650d5f6a7258164/);
});

test("recommendations are text-derived, optional, bounded and avoid duplicate recorded lines", () => {
  const recorded = rust("voice_lab_recording.rs");
  const build = rust("voice_lab_build.rs");
  const api = readFileSync(resolve(root, "src/app/bridge/myVoiceBuildApi.ts"), "utf8");
  const page = readFileSync(resolve(root, "src/pages/MyVoice.svelte"), "utf8");
  assert.match(selector, /pub\(super\) fn recommend_lines/);
  assert.match(selector, /inventory_current\(\)/);
  assert.match(selector, /excluded\.contains\(&id\)/);
  assert.match(selector, /count_ones\(\)/);
  assert.match(selector, /prompt_metadata\(id\)/);
  assert.match(selector, /limit\.min\(5\)/);
  assert.match(selector, /not-a-category/);
  assert.match(recorded, /guided_recording_recommendations/);
  assert.match(build, /recording_suggestions: Vec<RecordingSuggestion>/);
  assert.match(api, /recording_suggestions: MyVoiceRecordingSuggestion\[\]/);
  assert.match(page, /Optional recording suggestions/);
  assert.match(page, /not ratings of your pronunciation or voice quality/);
  assert.doesNotMatch(page, /Phonetic coverage \d+%/i);
});

test("no on-device model change, extra inference dependency or aggressive quality gate", () => {
  const selectorBody = selector.slice(selector.indexOf("/// Greedy ranked suggestions"), selector.indexOf("\n#[cfg(test)]\nmod tests"));
  assert.doesNotMatch(selectorBody, /torch|GPU|python|runCommand|delete|remove_file|remove_dir_all/);
  assert.match(selectorBody, /No recommendation affects train eligibility/);
  const build = rust("voice_lab_build.rs");
  assert.match(build, /let recording_suggestions = guided_recording_recommendations/);
  assert.match(build, /can_build: !snapshot\.active/);
  assert.match(build, /minimum_duration_ms: MIN_TRAINING_SPEECH_MS/);
});

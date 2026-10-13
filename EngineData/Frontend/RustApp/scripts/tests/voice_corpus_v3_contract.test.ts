import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (path: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + path), "utf8");
const ui = (path: string) => readFileSync(resolve(root, "src/" + path), "utf8");

function rowsFrom(source: string, extension: boolean) {
  const pattern = extension
    ? /^\s*\((\d+),\s*("(?:[^"\\]|\\.)*"),\s*"([^"]+)",\s*"([^"]+)"\),?$/gm
    : /^\s*\((\d+),\s*("(?:[^"\\]|\\.)*")\),?$/gm;
  return [...source.matchAll(pattern)].map((match) => ({
    id: Number(match[1]),
    text: JSON.parse(match[2]) as string,
    focus: match[3] ?? "legacy",
    coverage: match[4] ?? "legacy",
  }));
}

// Compatibility fingerprint of *every* original ID/text pair. Metadata can be
// added, but old literal transcripts must never be edited or relabeled.
function legacyFingerprint(lines: Array<{id:number; text:string}>): string {
  const payload = lines.map(({ id, text }) => String(id) + "\t" + text + "\n").join("");
  let value = 14695981039346656037n;
  for (let i = 0; i < payload.length; i++) {
    const code = payload.charCodeAt(i);
    assert.ok(code <= 127, "the frozen legacy corpus is ASCII");
    value = BigInt.asUintN(64, (value ^ BigInt(code)) * 1099511628211n);
  }
  return value.toString(16).padStart(16, "0");
}

const legacy = rowsFrom(rust("voice_lab_recording/guided_lines.rs"), false);
const short = rowsFrom(rust("voice_lab_recording/guided_corpus_short.rs"), true);
const natural = rowsFrom(rust("voice_lab_recording/guided_corpus_natural.rs"), true);
const detailed = rowsFrom(rust("voice_lab_recording/guided_corpus_detailed.rs"), true);
const all = [...legacy, ...short, ...natural, ...detailed];

test("legacy training prompts remain byte-for-byte text-compatible", () => {
  assert.equal(legacy.length, 128);
  assert.equal(legacyFingerprint(legacy), "be34d419e0139caf");
  assert.equal(legacy[0].text, "Good morning, everyone.");
  assert.equal(legacy[127].id, 128);
});

test("six hundred genuinely distinct English prompts with stable, reserved IDs", () => {
  assert.deepEqual([legacy.length, short.length, natural.length, detailed.length],
    [128, 156, 198, 118]);
  assert.equal(all.length, 600);
  for (const [index, line] of all.entries()) {
    assert.equal(line.id, index + 1, "no gaps, aliases or reassigned prompt IDs");
    assert.ok(!/[\r\n]/.test(line.text));
    const words = line.text.match(/[A-Za-z]+(?:'[A-Za-z]+)?/g) ?? [];
    assert.ok(words.length >= 3 && words.length <= 32, "natural recording length");
    assert.ok(!/\d/.test(line.text), "spoken English for numbers, no ambiguous digit reading");
  }
  const normalized = all.map(({text}) => text.toLowerCase().replace(/[^a-z0-9 ]+/g, "")
    .replace(/\s+/g, " ").trim());
  assert.equal(new Set(normalized).size, 600, "duplicate spoken prompt");
  assert.ok(all.every(({ id }) => id < 1001), "evaluation IDs 1001–1008 stay reserved");
});

test("metadata separates user-facing reading levels from training coverage", () => {
  const allowed = new Set(["conversation", "intonation", "natural", "details", "explanations"]);
  const extension = [...short, ...natural, ...detailed];
  assert.ok(extension.every((line) => allowed.has(line.coverage) && /^[a-z-]+$/.test(line.focus)));
  assert.ok(new Set(extension.map((x) => x.focus)).size >= 30);
  const coverage = Object.fromEntries([...allowed].map((key) =>
    [key, extension.filter((line) => line.coverage === key).length]));
  assert.ok(Object.values(coverage).every((count) => count >= 20),
    "all five historic coverage categories have new prompt choices");
  const state = rust("voice_lab_recording.rs");
  const build = rust("voice_lab_build.rs");
  assert.match(state, /guided_training_coverage/);
  assert.match(state, /prompt_metadata/);
  assert.match(build, /guided_training_coverage\(take\.line_id\) == Some\(group\.coverage\)/);
  assert.match(build, /suggested_line_id/);
  assert.doesNotMatch(build, /\(group\.start_line_id\.\.=group\.end_line_id\)\.contains\(&take\.line_id\)/);
});

test("UI handles the 600-prompt library through category/search and bounded pages", () => {
  const page = ui("pages/MyVoice.svelte");
  const types = ui("app/bridge/myVoiceApi.ts");
  assert.match(page, /Short Conversations/);
  assert.match(page, /Natural Conversations/);
  assert.match(page, /Detailed Explanations/);
  assert.match(page, /browsePageSize = 35/);
  assert.match(page, /browseVisible/);
  assert.match(page, /Search line number or words/);
  assert.match(page, /You don't have to record all/);
  assert.match(page, /Save this recording/);
  assert.match(types, /level: "short" \| "natural" \| "detailed"/);
  assert.match(types, /coverage: "conversation"/);
  assert.match(rust("voice_lab_recording.rs"), /migrate_legacy_recordings\(&storage_paths\(\), guided_lines\(\)\)/);
});

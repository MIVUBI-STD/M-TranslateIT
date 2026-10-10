import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
  "utf8",
);
const python = readFileSync(
  new URL(
    "../../../../Backend/LocalWorker/WorkerRuntime/realtime_local_worker_base.py",
    import.meta.url,
  ),
  "utf8",
);

function pythonHandlers(): string[] {
  const match = python.match(/HANDLERS\s*=\s*\{([\s\S]*?)\n\}/);
  assert.ok(match, "Python HANDLERS table must exist");
  return [...match[1].matchAll(/"([a-z_][a-z_0-9]*)"\s*:/g)]
    .map((item) => item[1])
    .sort();
}

function rustDeadlineTasks(): string[] {
  const match = rust.match(
    /pub fn worker_response_deadline_ms\(task: &str\)[\s\S]*?match task \{([\s\S]*?)\n\s*_ =>/,
  );
  assert.ok(match, "Rust worker deadline router must exist");
  const values = new Set<string>();
  for (const arm of match[1].matchAll(/([^=]+)=>/g)) {
    for (const item of arm[1].matchAll(/"([a-z_][a-z_0-9]*)"/g)) values.add(item[1]);
  }
  return [...values].sort();
}

test("Rust helper deadlines cover exactly the Python worker command handlers", () => {
  assert.deepEqual(rustDeadlineTasks(), pythonHandlers());
});

test("canonical worker protocol contains the required realtime task set", () => {
  assert.deepEqual(pythonHandlers(), [
    "asr_preload",
    "ping",
    "status",
    "transcribe",
    "translate",
    "translation_preload",
    "voice_actor_preflight",
    "voice_actor_synthesize",
  ]);
});

test("unknown Rust worker tasks remain bounded by a fallback deadline", () => {
  assert.match(rust, /_ => WORKER_FALLBACK_RESPONSE_DEADLINE_MS/);
});

test("Rust stdout JSON framing is capped and requires a newline before accepting data", () => {
  const reader = rust.slice(
    rust.indexOf("pub fn read_worker_response<R: BufRead>"),
    rust.indexOf("pub fn read_worker_response_direct_with_deadline"),
  );
  assert.ok(reader.length > 0);
  assert.match(rust, /MAX_WORKER_RESPONSE_BYTES: u64 = 1_000_000/);
  assert.match(reader, /\.take\(MAX_WORKER_RESPONSE_BYTES \+ 1\)/);
  assert.match(reader, /\.read_until\(b'\\n', &mut line\)/);
  assert.match(reader, /size as u64 > MAX_WORKER_RESPONSE_BYTES/);
  assert.match(reader, /worker:response_too_large/);
  assert.match(reader, /worker:response_not_newline_terminated/);
  assert.doesNotMatch(reader, /\.read_line\(/);
  assert.match(rust, /mod response_framing_tests/);
});

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const python = readFileSync(
  new URL(
    "../../../../Backend/LocalWorker/WorkerRuntime/realtime_local_worker_base.py",
    import.meta.url,
  ),
  "utf8",
);
const rust = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
  "utf8",
);
const frontend = readFileSync(
  new URL("../../src/app/bridge/workerCapabilities.ts", import.meta.url),
  "utf8",
);

const readinessKeys = [
  "asr",
  "translation_id_en",
  "translation_en_id",
  "voice_actor_tts",
] as const;

test("worker readiness keys stay aligned across Python Rust and TypeScript", () => {
  for (const key of readinessKeys) {
    assert.match(python, new RegExp(`"${key}"\\s*:`), `Python missing ${key}`);
    assert.match(frontend, new RegExp(`readiness\\["${key}"\\]`), `TypeScript missing ${key}`);
  }

  for (const key of ["asr", "translation_id_en", "voice_actor_tts"]) {
    assert.match(
      rust,
      new RegExp(`worker_nested_bool\\(status, "readiness", "${key}"\\)`),
      `Rust missing ${key}`,
    );
  }
});

test("worker preflight stage remains the capability response discriminator", () => {
  assert.match(python, /"stage": "local_realtime_worker_preflight"/);
  assert.match(frontend, /payload\["stage"\] === "local_realtime_worker_preflight"/);
  assert.match(rust, /stage == "local_realtime_worker_preflight"/);
});

test("frontend diagnostic loaded-state fields exist in Python status payload", () => {
  for (const key of [
    "asr",
    "asr_device",
    "asr_compute_type",
    "asr_model_id",
    "translation_directions",
    "voice_actor",
    "voice_actor_device",
  ]) {
    assert.match(python, new RegExp(`"${key}"\\s*:`), `Python loaded status missing ${key}`);
  }
});

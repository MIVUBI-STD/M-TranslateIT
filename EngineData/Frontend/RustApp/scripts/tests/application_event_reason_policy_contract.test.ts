import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);
const intents = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/intents.rs", import.meta.url),
  "utf8",
);
const mutations = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/mutations.rs", import.meta.url),
  "utf8",
);

function stringSetFromConst(name: string): Set<string> {
  const match = facade.match(new RegExp(`const ${name} = new Set\\(\\[([\\s\\S]*?)\\]\\);`));
  assert.ok(match, `${name} must exist`);
  return new Set([...match[1].matchAll(/"([a-z_0-9]+)"/g)].map((item) => item[1]));
}

function emittedReasons(): Set<string> {
  const values = new Set<string>();
  for (const match of intents.matchAll(/"([a-z_0-9]+)"\s*=>/g)) values.add(match[1]);
  for (const match of mutations.matchAll(/emit_after\(&app,\s*"([a-z_0-9]+)"/g)) values.add(match[1]);
  return values;
}

test("worker deep-refresh policy covers every emitted ApplicationRuntime reason", () => {
  const emitted = emittedReasons();
  const safe = stringSetFromConst("WORKER_REUSE_SAFE_REASONS");
  const expectedDeep = new Set([
    "start_meeting",
    "fix_setup",
    "check_readiness",
    "ensure_runtime_ready",
    "approve_voice_candidate",
    "select_builtin_voice",
  ]);

  assert.deepEqual(
    [...emitted].filter((reason) => !safe.has(reason)).sort(),
    [...expectedDeep].sort(),
  );
});

test("voice deep-refresh policy covers every emitted ApplicationRuntime reason", () => {
  const emitted = emittedReasons();
  const safe = stringSetFromConst("VOICE_REUSE_SAFE_REASONS");
  const expectedDeep = new Set([
    "start_voice_build",
    "cancel_voice_build",
    "approve_voice_candidate",
    "select_builtin_voice",
  ]);

  assert.deepEqual(
    [...emitted].filter((reason) => !safe.has(reason)).sort(),
    [...expectedDeep].sort(),
  );
});

test("safe reuse allowlists never contain a non-emitted reason", () => {
  const emitted = emittedReasons();
  for (const name of ["WORKER_REUSE_SAFE_REASONS", "VOICE_REUSE_SAFE_REASONS"]) {
    for (const reason of stringSetFromConst(name)) {
      assert.equal(emitted.has(reason), true, `${name} contains unknown reason ${reason}`);
    }
  }
});

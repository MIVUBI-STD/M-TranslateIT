import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const controller = readFileSync(
  new URL("../../src/app/runtime/applicationController.ts", import.meta.url),
  "utf8",
);
const types = readFileSync(
  new URL("../../src/app/bridge/runtimeProductTypes.ts", import.meta.url),
  "utf8",
);

test("product snapshot carries canonical application sequence", () => {
  assert.match(types, /applicationRevision: number/);
  assert.match(controller, /applicationRevision: number/);
});

test("application events are serialized before reconciliation", () => {
  assert.match(controller, /eventRefreshTail: Promise<void> = Promise\.resolve\(\)/);
  assert.match(controller, /eventRefreshTail\.then\(run, run\)/);
  assert.match(controller, /eventRefreshTail = result\.then/);
});

test("stale application events cannot overwrite newer state", () => {
  assert.match(controller, /application\.revision <= state\.applicationRevision/);
  assert.match(controller, /next\.applicationRevision < state\.applicationRevision/);
});

test("newer canonical application state can beat older request ordering", () => {
  assert.match(
    controller,
    /requestRevision !== state\.revision[\s\S]*next\.applicationRevision <= state\.applicationRevision/,
  );
  assert.match(controller, /applicationRevision: next\.applicationRevision/);
});

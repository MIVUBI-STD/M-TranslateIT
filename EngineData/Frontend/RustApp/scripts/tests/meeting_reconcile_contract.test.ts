import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const reader = readFileSync(
  new URL("../../src/app/runtime/meetingReconcileReader.ts", import.meta.url),
  "utf8",
);
const controller = readFileSync(
  new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url),
  "utf8",
);

test("event reconciliation can bypass unchanged status-key shortcut", () => {
  assert.match(reader, /forceTurns = false/);
  assert.match(reader, /statusKey === previousStatusKey && !forceTurns/);
  assert.match(controller, /forceTurns = false/);
  assert.match(controller, /forceTurns,/);
});

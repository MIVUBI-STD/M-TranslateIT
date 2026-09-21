import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = readFileSync(
  new URL("../../src/app/bridge/runtimeProductState.ts", import.meta.url),
  "utf8",
);
const meeting = readFileSync(
  new URL("../../src/app/bridge/productMeetingState.ts", import.meta.url),
  "utf8",
);
const readiness = readFileSync(
  new URL("../../src/app/bridge/productReadinessState.ts", import.meta.url),
  "utf8",
);
const worker = readFileSync(
  new URL("../../src/app/bridge/workerCapabilities.ts", import.meta.url),
  "utf8",
);

test("runtime product state is a compatibility surface only", () => {
  assert.match(root, /from "\.\/productMeetingState"/);
  assert.match(root, /from "\.\/productReadinessState"/);
  assert.match(root, /from "\.\/workerCapabilities"/);
  assert.doesNotMatch(root, /function mapProductMeetingState/);
  assert.doesNotMatch(root, /function mapProductReadiness/);
});

test("Meeting and readiness policies have separate owners", () => {
  assert.match(meeting, /export function mapProductMeetingState/);
  assert.match(meeting, /export function meetingPreflight/);
  assert.match(readiness, /export function mapProductReadiness/);
  assert.match(readiness, /resolveMeetingVoiceGate/);
  assert.doesNotMatch(meeting, /parseWorkerCapabilities/);
});

test("worker capability parsing remains isolated", () => {
  assert.match(worker, /export function parseWorkerCapabilities/);
  assert.doesNotMatch(worker, /mapProductMeetingState|mapProductReadiness/);
});

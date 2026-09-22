import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const frontend = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);
const backend = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/intents.rs", import.meta.url),
  "utf8",
);

function frontendIntents(): string[] {
  const match = frontend.match(/export type ProductIntent\s*=([\s\S]*?);/);
  assert.ok(match, "ProductIntent union must exist");
  return [...match[1].matchAll(/"([a-z_0-9]+)"/g)]
    .map((item) => item[1])
    .sort();
}

function backendIntents(): string[] {
  const dispatch = backend.match(/pub fn dispatch\(intent: &str\)[\s\S]*?match intent \{([\s\S]*?)\n\s*\}/);
  assert.ok(dispatch, "backend intent dispatcher must exist");
  return [...dispatch[1].matchAll(/"([a-z_0-9]+)"\s*=>/g)]
    .map((item) => item[1])
    .sort();
}

test("frontend ProductIntent union matches backend dispatcher exactly", () => {
  assert.deepEqual(frontendIntents(), backendIntents());
});

test("unsupported intent remains fail-closed", () => {
  assert.match(backend, /"unsupported_intent"/);
});

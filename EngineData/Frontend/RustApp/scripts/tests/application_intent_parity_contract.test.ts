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
  assert.match(backend, /pub fn dispatch\(intent: &str\)/);
  assert.match(backend, /match intent \{/);
  // Match Rust arms at line boundaries: stopping at the first nested } loses intents.
  return [...backend.matchAll(/^\s*"([a-z_0-9]+)"\s*=>/gm)]
    .map((item) => item[1])
    .sort();
}

test("frontend ProductIntent union matches backend dispatcher exactly", () => {
  assert.deepEqual(frontendIntents(), backendIntents());
});

test("unsupported intent remains fail-closed", () => {
  assert.match(backend, /"unsupported_intent"/);
});

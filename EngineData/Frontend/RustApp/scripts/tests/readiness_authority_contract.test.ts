import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const setup = readFileSync(
  new URL("../../src/app/bridge/productSetupFacade.ts", import.meta.url),
  "utf8",
);
const applicationApi = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);
const runtimeApi = readFileSync(
  new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url),
  "utf8",
);
const intents = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/intents.rs", import.meta.url),
  "utf8",
);
const registry = readFileSync(
  new URL("../../src-tauri/src/commands/registry.rs", import.meta.url),
  "utf8",
);

test("readiness verification is owned by ApplicationRuntime", () => {
  assert.match(setup, /dispatchIntent\("check_readiness"\)/);
  assert.match(applicationApi, /"check_readiness"/);
  assert.match(intents, /"check_readiness" =>/);
});

test("direct readiness bridge is closed", () => {
  assert.doesNotMatch(runtimeApi, /verifyRequiredOutboundAiReadiness/);
  assert.doesNotMatch(runtimeApi, /"verify_required_outbound_ai_readiness"/);
  assert.doesNotMatch(registry, /verify_required_outbound_ai_readiness/);
});

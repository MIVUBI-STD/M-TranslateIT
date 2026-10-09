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

const mutationOwner = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/mutations.rs", import.meta.url), "utf8");
const mutationModule = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/mod.rs", import.meta.url), "utf8");
const tauriRegistry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");

test("Tauri mutation commands register at their defining module, not a function-only re-export", () => {
  assert.match(mutationModule, /pub\(crate\) mod mutations;/);
  assert.doesNotMatch(mutationModule, /pub use mutations::/);
  for (const name of ["select_product_audio_device","save_product_settings","apply_product_meeting_preset","start_product_voice_recording","stop_product_voice_recording","start_product_voice_build","cancel_product_voice_build","approve_product_voice_candidate","select_product_builtin_voice"]) {
    assert.ok(mutationOwner.includes(`#[tauri::command]\npub fn ${name}(`), name);
    assert.ok(tauriRegistry.includes(`crate::commands::application_runtime::mutations::${name},`), name);
  }
});

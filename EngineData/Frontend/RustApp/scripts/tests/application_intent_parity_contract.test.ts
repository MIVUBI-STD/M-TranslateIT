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
  for (const name of ["select_product_audio_device","save_product_settings","start_product_voice_recording","stop_product_voice_recording","start_product_voice_build","cancel_product_voice_build","approve_product_voice_candidate","select_product_builtin_voice"]) {
    const declaration = name === "select_product_builtin_voice" ? "pub async fn" : "pub fn";
    assert.ok(mutationOwner.includes(`#[tauri::command]\n${declaration} ${name}(`), name);
    assert.ok(tauriRegistry.includes(`crate::commands::application_runtime::mutations::${name},`), name);
  }
});

const recoveryFacade = readFileSync(new URL("../../src/app/bridge/productSetupFacade.ts", import.meta.url), "utf8");
const diagnosticsPage = readFileSync(new URL("../../src/components/settings/SettingsDiagnostics.svelte", import.meta.url), "utf8");

test("blocked worker recovery is explicit and guarded by active owners", () => {
  assert.match(backend, /"retry_blocked_worker"\s*=>\s*retry_blocked_worker\(\)/);
  assert.match(backend, /if helper\.state != "blocked"/);
  for (const guard of [
    "snapshot.summaries.meeting.has_session",
    "snapshot.resources.audio_locked",
    "snapshot.active_owner.is_some()",
    "snapshot.summaries.voice.build_active",
  ]) {
    assert.ok(backend.includes(guard), `recovery missing guard ${guard}`);
  }
  assert.match(recoveryFacade, /dispatchIntent\("retry_blocked_worker"\)/);
  assert.match(diagnosticsPage, /snapshot\.helper\?\.state === "blocked"/);
  assert.match(diagnosticsPage, /onSetupAction\("retry-worker"\)/);
});

const textCommand = readFileSync(new URL("../../src-tauri/src/commands/text_translation.rs", import.meta.url), "utf8");
const helperCommand = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge.rs", import.meta.url), "utf8");
const modelCommand = readFileSync(new URL("../../src-tauri/src/commands/runtime.rs", import.meta.url), "utf8");

test("slow translation, AI worker status and disk inventory are off the UI event thread", () => {
  assert.match(textCommand, /pub async fn translate_text\(/);
  assert.match(textCommand, /spawn_blocking\(move \|\| translate_text_blocking\(source\)\)/);
  assert.match(helperCommand, /pub async fn helper_bridge_worker_status\(/);
  assert.match(helperCommand, /spawn_blocking\(\|\| send_worker_task\("status", json!\(\{\}\)\)\)/);
  assert.match(modelCommand, /pub async fn verify_models\(/);
  assert.match(modelCommand, /spawn_blocking\(runtime_inventory::verify_models\)/);
});

const applicationModule = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/mod.rs", import.meta.url), "utf8");

test("AI preparation and native snapshots use bounded asynchronous Tauri command entrypoints", () => {
  for (const command of ["get_application_snapshot", "dispatch_product_intent"]) {
    assert.ok(applicationModule.includes(`pub async fn ${command}(`), command);
  }
  assert.match(applicationModule, /spawn_blocking\(current_application_snapshot\)/);
  assert.match(applicationModule, /spawn_blocking\(move \|\| \{/);
  assert.match(mutationOwner, /pub async fn select_product_builtin_voice\(/);
  for (const selection of ["select_product_saved_personal_voice", "select_product_voice_version"]) {
    assert.ok(mutationOwner.includes(`pub async fn ${selection}(`), selection);
  }
  assert.match(mutationOwner, /spawn_blocking\(move \|\| \{/);
});

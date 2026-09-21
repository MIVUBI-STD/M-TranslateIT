import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtimeRoot = join(appRoot, "src-tauri", "src", "commands", "application_runtime");

const files = readdirSync(runtimeRoot)
  .filter((name) => name.endsWith(".rs"))
  .map((name) => join(runtimeRoot, name));

const failures = [];
const maxModuleBytes = 9_000;

for (const path of files) {
  const name = path.split(/[\\/]/).at(-1);
  const body = readFileSync(path, "utf8");
  const size = statSync(path).size;

  if (size > maxModuleBytes) {
    failures.push(`${name}: ${size} bytes exceeds application-runtime module budget ${maxModuleBytes}`);
  }
  if (/serde_json::(?:Value|json)/.test(body)) {
    failures.push(`${name}: application runtime must use typed contracts, not serde_json::Value/json`);
  }
  if (/tauri::WebviewWindow|WindowBuilder|@tauri|frontend|svelte/i.test(body) && name !== "events.rs") {
    failures.push(`${name}: application runtime kernel must not own UI/window concerns`);
  }
}

const modSource = readFileSync(join(runtimeRoot, "mod.rs"), "utf8");
for (const required of [
  "mod capabilities;",
  "mod contract;",
  "mod events;",
  "mod intents;",
  "mod lifecycle;",
  "mod problems;",
  "mod resources;",
  "mod snapshot;",
  "mod summaries;",
]) {
  if (!modSource.includes(required)) failures.push(`mod.rs missing modular boundary: ${required}`);
}

const snapshotSource = readFileSync(join(runtimeRoot, "snapshot.rs"), "utf8");
for (const forbidden of [
  "start_meeting_translation",
  "stop_meeting_translation",
  "start_capture",
  "stop_capture",
  "verify_required_outbound_ai_readiness",
]) {
  if (snapshotSource.includes(forbidden)) {
    failures.push(`snapshot.rs must be read-only and may not execute intent: ${forbidden}`);
  }
}

const intentsSource = readFileSync(join(runtimeRoot, "intents.rs"), "utf8");
if (!intentsSource.includes("pub fn dispatch")) {
  failures.push("intents.rs must own the product-intent dispatch boundary");
}

const facadeSource = readFileSync(join(appRoot, "src", "app", "bridge", "runtimeProductFacade.ts"), "utf8");
for (const forbidden of [
  "runtimeApi.startHelperBridge(",
  "runtimeApi.getHelperBridgeStatus(",
  "runtimeApi.helperBridgeWorkerStatus(",
  "runtimeApi.getMeetingSessionStatus(",
]) {
  if (facadeSource.includes(forbidden)) {
    failures.push(`runtimeProductFacade.ts must not own cross-feature lifecycle: ${forbidden}`);
  }
}


if (failures.length > 0) {
  console.error("Application runtime architecture validation failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`[application-runtime-architecture] ${files.length} modular kernel files passed boundary checks.`);

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtimeRoot = join(appRoot, "src-tauri", "src", "commands", "application_runtime");

function collectRustFiles(directory) {
  const collected = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collected.push(...collectRustFiles(path));
    else if (entry.isFile() && entry.name.endsWith(".rs")) collected.push(path);
  }
  return collected;
}

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
  "mod mutations;",
  "mod problems;",
  "mod resources;",
  "mod snapshot;",
  "mod shutdown;",
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
  "helper_bridge_worker_status",
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


const rustSourceRoot = join(appRoot, "src-tauri", "src");
for (const path of collectRustFiles(rustSourceRoot)) {
  if (path.endsWith(join("engine", "runtime_state.rs"))) continue;
  const body = readFileSync(path, "utf8");
  for (const literal of [
    '"translateit_application_meeting"',
    '"translateit_mic_test"',
    '"translateit_voice_recording"',
  ]) {
    if (body.includes(literal)) {
      failures.push(
        `${path}: runtime owner literal ${literal} must come from engine/runtime_state.rs canonical constants`,
      );
    }
  }
}

const runtimeStateSource = readFileSync(
  join(appRoot, "src-tauri", "src", "engine", "runtime_state.rs"),
  "utf8",
);
for (const required of [
  'MIC_TEST_OWNER_ID: &str = "translateit_mic_test"',
  'VOICE_RECORDING_OWNER_ID: &str = "translateit_voice_recording"',
  "pub fn begin_mic_test_session()",
  "pub fn begin_voice_recording_session()",
]) {
  if (!runtimeStateSource.includes(required)) {
    failures.push(`runtime_state.rs missing distinct shared-resource ownership contract: ${required}`);
  }
}
if (runtimeStateSource.includes("translateit_rust_live_capture")) {
  failures.push("runtime_state.rs must not collapse Mic Test and My Voice into one live-capture owner");
}

const sharedTypesSource = readFileSync(
  join(appRoot, "src", "app", "shared", "types.ts"),
  "utf8",
);
const meetingOwnerLiteral = '"translateit_application_meeting"';
if (!runtimeStateSource.includes(
  `APPLICATION_MEETING_OWNER_ID: &str = ${meetingOwnerLiteral}`,
)) {
  failures.push("runtime_state.rs missing canonical application Meeting owner literal");
}
if (!sharedTypesSource.includes(
  `APPLICATION_MEETING_OWNER_ID = ${meetingOwnerLiteral}`,
)) {
  failures.push("frontend Meeting owner constant must match Rust runtime owner identity");
}

const applicationContractSource = readFileSync(
  join(runtimeRoot, "contract.rs"),
  "utf8",
);
const applicationApiSource = readFileSync(
  join(appRoot, "src", "app", "bridge", "applicationRuntimeApi.ts"),
  "utf8",
);
if (applicationContractSource.includes("device_count")) {
  failures.push("ApplicationInputStatus must not expose synthetic device_count");
}
if (applicationApiSource.includes("device_count")) {
  failures.push("frontend ApplicationInputStatus must not restore synthetic device_count");
}
for (const overclaim of [
  "text_translation",
  "meeting_audio_available",
]) {
  if (applicationContractSource.includes(overclaim)) {
    failures.push(`cheap ApplicationSnapshot contract must not overclaim unproven field: ${overclaim}`);
  }
  if (applicationApiSource.includes(overclaim)) {
    failures.push(`frontend cheap ApplicationSnapshot contract must not restore unproven field: ${overclaim}`);
  }
}
if (applicationContractSource.includes("pub ready: bool")) {
  failures.push("WorkerSummary must name process readiness explicitly as process_ready");
}
if (!applicationContractSource.includes("pub process_ready: bool")) {
  failures.push("WorkerSummary missing explicit process_ready field");
}

const runtimeApiSource = readFileSync(join(appRoot, "src", "app", "bridge", "runtimeApi.ts"), "utf8");
for (const forbidden of [
  '"select_audio_device"',
  '"save_runtime_settings"',
  '"apply_meeting_preset"',
  '"get_helper_bridge_status"',
  '"start_helper_bridge"',
  '"get_input_status"',
]) {
  if (runtimeApiSource.includes(forbidden)) {
    failures.push(`runtimeApi.ts must route settings/audio mutations through application authority: ${forbidden}`);
  }
}
const myVoiceApiSource = readFileSync(join(appRoot, "src", "app", "bridge", "myVoiceApi.ts"), "utf8");
for (const forbidden of ['"start_voice_lab_guided_take"', '"stop_voice_lab_guided_take"']) {
  if (myVoiceApiSource.includes(forbidden)) {
    failures.push(`myVoiceApi.ts must route microphone mutations through application authority: ${forbidden}`);
  }
}

const applicationEventsSource = readFileSync(
  join(runtimeRoot, "events.rs"),
  "utf8",
);
const meetingEventsSource = readFileSync(
  join(appRoot, "src-tauri", "src", "engine", "runtime_events.rs"),
  "utf8",
);
const reliabilityEventSource = readFileSync(
  join(appRoot, "src", "app", "runtime", "reliabilityMonitor.ts"),
  "utf8",
);

for (const [label, backend, frontend, eventName] of [
  [
    "application runtime",
    applicationEventsSource,
    applicationApiSource,
    "translateit://application-runtime",
  ],
  [
    "Meeting runtime",
    meetingEventsSource,
    reliabilityEventSource,
    "translateit://meeting-runtime",
  ],
]) {
  if (!backend.includes(eventName)) {
    failures.push(`${label} backend event contract missing: ${eventName}`);
  }
  if (!frontend.includes(eventName)) {
    failures.push(`${label} frontend event contract missing: ${eventName}`);
  }
}

for (const field of ["reason", "snapshot"]) {
  if (!applicationEventsSource.includes(`${field}:`)) {
    failures.push(`ApplicationRuntimeEvent backend missing field: ${field}`);
  }
  if (!applicationApiSource.includes(`${field}:`)) {
    failures.push(`ApplicationRuntimeEvent frontend missing field: ${field}`);
  }
}
for (const field of ["revision", "reason", "session_id", "sequence"]) {
  if (!meetingEventsSource.includes(`pub ${field}:`)) {
    failures.push(`MeetingRuntimeEvent backend missing field: ${field}`);
  }
  if (!reliabilityEventSource.includes(`${field}:`)) {
    failures.push(`MeetingRuntimeEvent frontend missing field: ${field}`);
  }
}

const nativeCloseSource = readFileSync(join(appRoot, "src", "app", "runtime", "nativeCloseRuntime.ts"), "utf8");
for (const forbidden of [
  "runtimeApi.getMeetingSessionStatus(",
  "myVoiceBuildApi.getStatus(",
]) {
  if (nativeCloseSource.includes(forbidden)) {
    failures.push(`nativeCloseRuntime.ts must use canonical application snapshot: ${forbidden}`);
  }
}

const appShellSource = readFileSync(join(appRoot, "src", "App.svelte"), "utf8");
for (const forbidden of [
  "let runtimeSettings = $state",
  "let helperStatus = $state",
  "let workerStatus = $state",
  "let inputStatus = $state",
  "let approvedVoiceReady = $state",
]) {
  if (appShellSource.includes(forbidden)) {
    failures.push("App.svelte must not duplicate product runtime state outside applicationController: " + forbidden);
  }
}
for (const required of [
  "createApplicationController(",
  "createMeetingLiveController(",
  "createCloseController(",
  "applicationState.snapshot",
]) {
  if (!appShellSource.includes(required)) {
    failures.push("App.svelte missing canonical frontend controller contract: " + required);
  }
}

for (const forbidden of [
  "let closeDialogOpen = $state",
  "let closeDialogTitle = $state",
  "let closeDialogMessage = $state",
  "let meetingPollInFlight",
  "let lastTranscriptStatusKey",
  "let lastOverlayMeetingRevision",
]) {
  if (appShellSource.includes(forbidden)) {
    failures.push("App.svelte must not reclaim Meeting/close controller state: " + forbidden);
  }
}

const productFacadeSource = readFileSync(
  join(appRoot, "src", "app", "bridge", "runtimeProductFacade.ts"),
  "utf8",
);
for (const required of [
  'from "./productAudioFacade"',
  'from "./productTranslationFacade"',
  'from "./productSetupFacade"',
]) {
  if (!productFacadeSource.includes(required)) {
    failures.push("runtimeProductFacade.ts must remain split by domain: " + required);
  }
}

const productStateSource = readFileSync(
  join(appRoot, "src", "app", "bridge", "runtimeProductState.ts"),
  "utf8",
);
for (const required of [
  'from "./productMeetingState"',
  'from "./productReadinessState"',
  'from "./workerCapabilities"',
]) {
  if (!productStateSource.includes(required)) {
    failures.push("runtimeProductState.ts must remain a thin compatibility re-export: " + required);
  }
}
if (productStateSource.includes("function mapProductMeetingState") || productStateSource.includes("function mapProductReadiness")) {
  failures.push("runtimeProductState.ts must not reclaim Meeting/readiness implementation");
}

const meetingLiveControllerSource = readFileSync(
  join(appRoot, "src", "app", "runtime", "meetingLiveController.ts"),
  "utf8",
);
if (!meetingLiveControllerSource.includes('from "./meetingReconcileReader"')) {
  failures.push("meetingLiveController.ts must use meetingReconcileReader semantic boundary");
}
if (meetingLiveControllerSource.includes('from "./meetingPoll"')) {
  failures.push("meetingLiveController.ts must not restore obsolete meetingPoll module naming");
}

const ownershipDocs = readFileSync(
  join(appRoot, "..", "..", "..", "docs", "knowledge", "source-ownership.md"),
  "utf8",
);
for (const stale of [
  "meetingPoll.ts",
  "Readiness / Meeting-state mapping | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductState.ts`",
  "Native process-exit fail-safe / helper shutdown | `src-tauri/src/main.rs` + `commands/helper_bridge.rs`",
]) {
  if (ownershipDocs.includes(stale)) {
    failures.push("source-ownership.md contains stale architecture reference: " + stale);
  }
}

const meetingPageSource = readFileSync(join(appRoot, "src", "pages", "Meeting.svelte"), "utf8");
if (meetingPageSource.includes("startRuntimePoll")) {
  failures.push("Meeting.svelte must not restore generic UI polling; use event-first sync or explicit on-demand refresh");
}

const reliabilityMonitorSource = readFileSync(
  join(appRoot, "src", "app", "runtime", "reliabilityMonitor.ts"),
  "utf8",
);
for (const required of [
  'MEETING_RUNTIME_EVENT = "translateit://meeting-runtime"',
  "MEETING_RECONCILIATION_POLL_MS = 10_000",
  "listen<MeetingRuntimeEvent>",
]) {
  if (!reliabilityMonitorSource.includes(required)) {
    failures.push(`reliabilityMonitor.ts missing event-first Meeting contract: ${required}`);
  }
}
if (reliabilityMonitorSource.includes("1_200")) {
  failures.push("reliabilityMonitor.ts must not restore 1.2s Meeting state polling");
}

const myVoiceBuildApiSource = readFileSync(join(appRoot, "src", "app", "bridge", "myVoiceBuildApi.ts"), "utf8");
for (const forbidden of [
  '"start_voice_lab_build"',
  '"cancel_voice_lab_build"',
  '"approve_voice_lab_candidate"',
  '"select_builtin_voice"',
]) {
  if (myVoiceBuildApiSource.includes(forbidden)) {
    failures.push(`myVoiceBuildApi.ts must route Voice mutations through application authority: ${forbidden}`);
  }
}


if (failures.length > 0) {
  console.error("Application runtime architecture validation failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`[application-runtime-architecture] ${files.length} modular kernel files passed boundary checks.`);

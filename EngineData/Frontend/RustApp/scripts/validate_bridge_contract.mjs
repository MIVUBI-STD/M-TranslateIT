import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const bridgeRoot = join(root, "src", "app");

function source(relativePath) {
  return readFileSync(join(root, relativePath), "utf8");
}

function registeredCommands() {
  const names = new Set();
  for (const match of source("src-tauri/src/commands/registry.rs").matchAll(
    /crate::(?:commands|engine)::[a-z_0-9:]+::([a-z_0-9]+)/g,
  )) {
    names.add(match[1]);
  }
  return names;
}

function invokedCommands() {
  const names = new Set();
  const stack = [bridgeRoot];
  while (stack.length > 0) {
    const dir = stack.pop();
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(path);
        continue;
      }
      if (!/\.(ts|js|svelte)$/.test(entry.name)) continue;
      const body = readFileSync(path, "utf8");
      const callPattern = /(?:\brunCommand|\binvoke[A-Za-z0-9_]*)(?:<[\s\S]{0,160}?>)?\(\s*"([a-z_0-9]+)"/g;
      for (const match of body.matchAll(callPattern)) names.add(match[1]);
    }
  }
  return names;
}

function rustStructFields(relativePath, structName) {
  const body = source(relativePath);
  const match = body.match(new RegExp(`pub struct ${structName}\\s*\\{([\\s\\S]*?)\\n\\}`, "m"));
  if (!match) throw new Error(`bridge-contract: Rust struct not found: ${relativePath}::${structName}`);
  return new Set([...match[1].matchAll(/^\s*pub\s+([a-z_][a-z_0-9]*)\s*:/gm)].map((item) => item[1]));
}

function tsTypeBlock(relativePath, typeName) {
  const body = source(relativePath);
  const match = body.match(new RegExp(`export type ${typeName}\\s*=\\s*\\{([\\s\\S]*?)\\n\\};`, "m"));
  if (!match) throw new Error(`bridge-contract: TypeScript type not found: ${relativePath}::${typeName}`);
  return match[1];
}

function tsTypeFields(relativePath, typeName) {
  const block = tsTypeBlock(relativePath, typeName);
  return new Set([...block.matchAll(/^\s*([A-Za-z_][A-Za-z0-9_]*)\??\s*:/gm)].map((item) => item[1]));
}

function sortedDifference(left, right) {
  return [...left].filter((item) => !right.has(item)).sort();
}

const failures = [];
const rustCommands = registeredCommands();
const frontendCommands = invokedCommands();

if (rustCommands.size === 0) failures.push("no registered Tauri commands found");
if (frontendCommands.size === 0) failures.push("no frontend bridge command invocations found");
for (const name of sortedDifference(rustCommands, frontendCommands)) {
  failures.push(`registered but never invoked by frontend: ${name}`);
}
for (const name of sortedDifference(frontendCommands, rustCommands)) {
  failures.push(`invoked by frontend but not registered: ${name}`);
}

const contractPairs = [
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationProblem", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationProblem"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationCapabilities", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationCapabilities"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "MeetingSummary", "src/app/bridge/applicationRuntimeApi.ts", "MeetingSummary"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "WorkerSummary", "src/app/bridge/applicationRuntimeApi.ts", "WorkerSummary"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "AudioSummary", "src/app/bridge/applicationRuntimeApi.ts", "AudioSummary"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationInputStatus", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationInputStatus"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "VoiceSummary", "src/app/bridge/applicationRuntimeApi.ts", "VoiceSummary"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationSubsystemSummaries", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationSubsystemSummaries"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ResourceArbitration", "src/app/bridge/applicationRuntimeApi.ts", "ResourceArbitration"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationSnapshot", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationSnapshot"],
  ["src-tauri/src/commands/application_runtime/contract.rs", "ApplicationIntentResult", "src/app/bridge/applicationRuntimeApi.ts", "ApplicationIntentResult"],
  ["src-tauri/src/commands/reliability_snapshot.rs", "MeetingReliabilitySnapshot", "src/app/bridge/reliabilityApi.ts", "MeetingReliabilitySnapshot"],
  ["src-tauri/src/commands/helper_bridge_runtime.rs", "HelperBridgeStatus", "src/app/shared/types.ts", "HelperBridgeStatus"],
  ["src-tauri/src/commands/helper_bridge_runtime.rs", "HelperBridgeActionResult", "src/app/shared/types.ts", "HelperBridgeActionResult"],
  ["src-tauri/src/commands/helper_bridge.rs", "HelperBridgeWorkerResponse", "src/app/shared/types.ts", "HelperBridgeWorkerResponse"],
  ["src-tauri/src/commands/virtual_mic_route.rs", "VirtualMicRouteContractStatus", "src/app/bridge/runtimeApi.ts", "VirtualMicRouteContractStatus"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingSessionPreflightStatus", "src/app/bridge/runtimeApi.ts", "MeetingSessionPreflightStatus"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingOutboundTiming", "src/app/bridge/runtimeApi.ts", "MeetingOutboundTiming"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingOutboundRuntimeStatus", "src/app/bridge/runtimeApi.ts", "MeetingOutboundRuntimeStatus"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingIncomingRuntimeStatus", "src/app/bridge/runtimeApi.ts", "MeetingIncomingRuntimeStatus"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingSessionStatus", "src/app/bridge/runtimeApi.ts", "MeetingSessionStatus"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingSessionActionResult", "src/app/bridge/runtimeApi.ts", "MeetingSessionActionResult"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingCommittedTurn", "src/app/bridge/runtimeApi.ts", "MeetingCommittedTurn"],
  ["src-tauri/src/commands/meeting_session.rs", "MeetingCommittedTurnsSnapshot", "src/app/bridge/runtimeApi.ts", "MeetingCommittedTurnsSnapshot"],
  ["src-tauri/src/commands/meeting_session/transcript_export.rs", "MeetingTranscriptExportStatus", "src/app/bridge/runtimeApi.ts", "MeetingTranscriptExportStatus"],
  ["src-tauri/src/commands/meeting_session/transcript_export.rs", "MeetingTranscriptExportResult", "src/app/bridge/runtimeApi.ts", "MeetingTranscriptExportResult"],
];

for (const [rustFile, rustType, tsFile, tsType] of contractPairs) {
  const rustFields = rustStructFields(rustFile, rustType);
  const tsFields = tsTypeFields(tsFile, tsType);
  const missing = sortedDifference(rustFields, tsFields);
  const extra = sortedDifference(tsFields, rustFields);
  if (missing.length > 0) failures.push(`${tsType} missing Rust fields: ${missing.join(", ")}`);
  if (extra.length > 0) failures.push(`${tsType} has non-Rust fields: ${extra.join(", ")}`);
  if (/\[\s*key\s*:\s*string\s*\]/.test(tsTypeBlock(tsFile, tsType))) {
    failures.push(`${tsType} must not hide bridge drift behind a string index signature`);
  }
}

const boundaryFiles = [
  "src/app/shared/types.ts",
  "src/app/shared/tauriBridge.ts",
  "src/app/bridge/applicationRuntimeApi.ts",
  "src/app/bridge/runtimeApi.ts",
  "src/app/bridge/runtimeProductFacade.ts",
  "src/app/bridge/runtimeProductState.ts",
  "src/app/bridge/runtimeProductTypes.ts",
  "src/app/bridge/myVoiceApi.ts",
  "src/app/bridge/myVoiceBuildApi.ts",
];
const forbiddenAny = [
  ["Record<string, any>", /Record\s*<\s*string\s*,\s*any\s*>/g],
  ["any index signature", /\[[^\]]+\]\s*:\s*any\b/g],
  ["as any assertion", /\bas\s+any\b/g],
  ["explicit any annotation", /:\s*any(?:\[\])?\b/g],
  ["generic any argument", /<\s*any\s*>/g],
];
function withoutComments(value) {
  return value.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
}
for (const relativePath of boundaryFiles) {
  const body = withoutComments(source(relativePath));
  for (const [label, pattern] of forbiddenAny) {
    pattern.lastIndex = 0;
    if (pattern.test(body)) failures.push(`${relativePath}: ${label}`);
  }
}

if (failures.length > 0) {
  console.error("Bridge contract validation failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`[bridge-contract] ${rustCommands.size} Tauri commands are 1:1 aligned with frontend invocations.`);
console.log(`[bridge-contract] ${contractPairs.length} Rust/TypeScript response shapes are field-aligned.`);
console.log("[bridge-contract] bridge boundaries contain no explicit any escape hatches.");

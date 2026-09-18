import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const files = {
  meeting: resolve(root, "src-tauri/src/commands/meeting_session.rs"),
  meetingLifecycle: resolve(root, "src-tauri/src/commands/meeting_session/lifecycle.rs"),
  outboundPipeline: resolve(root, "src-tauri/src/commands/meeting_session/outbound_pipeline.rs"),
  route: resolve(root, "src-tauri/src/commands/virtual_mic_route.rs"),
  meetingOutput: resolve(root, "src-tauri/src/engine/audio/meeting_output.rs"),
  meetingOutputRuntime: resolve(root, "src-tauri/src/engine/audio/meeting_output_runtime.rs"),
  audioMod: resolve(root, "src-tauri/src/engine/audio/mod.rs"),
  commandsMod: resolve(root, "src-tauri/src/commands/mod.rs"),
  registry: resolve(root, "src-tauri/src/commands/registry.rs"),
};
const retired = [
  resolve(root, "src-tauri/src/commands/virtual_audio_route_runtime.rs"),
  resolve(root, "../../Backend/LocalWorker/WorkerRuntime/virtual_audio_route_provider.py"),
];

const source = {};
for (const [name, path] of Object.entries(files)) {
  if (!existsSync(path)) throw new Error(`Missing ${name}: ${path}`);
  source[name] = readFileSync(path, "utf8");
}
for (const path of retired) if (existsSync(path)) throw new Error(`Retired Python/command route owner still exists: ${path}`);

function requireMarkers(body, label, markers) {
  for (const marker of markers) if (!body.includes(marker)) throw new Error(`${label} marker missing: ${marker}`);
}
function forbidMarkers(body, label, markers) {
  for (const marker of markers) if (body.includes(marker)) throw new Error(`${label} forbidden marker found: ${marker}`);
}

requireMarkers(source.audioMod, "Windows audio ownership", ["pub mod meeting_output;"]);
forbidMarkers(source.commandsMod, "retired command-layer route owner", ["pub mod virtual_audio_route_runtime;"]);
requireMarkers(source.meeting, "Meeting session output orchestration facade", [
  "mod lifecycle;",
  "mod outbound_pipeline;",
  "process_outbound_wav",
]);
requireMarkers(source.meetingLifecycle, "Meeting lifecycle output orchestration", [
  "bind_prepared_virtual_mic_route_to_generation",
  "prepare_meeting_output_device",
  "cancel_meeting_output_for_generation",
  "get_virtual_mic_route_selection",
]);
requireMarkers(source.outboundPipeline, "Meeting outbound delivery ownership", [
  "pub(super) fn process_outbound_wav(",
  "deliver_meeting_output_wav",
  "get_bound_virtual_mic_output_device",
]);
forbidMarkers(`${source.meeting}\n${source.meetingLifecycle}\n${source.outboundPipeline}`, "retired Python route ownership", [
  "TRANSLATEIT_ENABLE_VIRTUAL_AUDIO_ROUTE_PROVIDER",
  "dispatch_meeting_virtual_audio_route_provider",
  "prepare_meeting_virtual_audio_route_provider",
  "route_execution_guard_ready",
]);
requireMarkers(source.meetingOutput, "guarded Meeting output facade", [
  "mod runtime;",
  "pub fn deliver_meeting_output_wav(",
  "validate_source_riff_boundary",
  "meeting_output:wav_riff_size_mismatch",
  "runtime::deliver_meeting_output_wav",
]);
requireMarkers(source.meetingOutputRuntime, "native Meeting output runtime", [
  "pub fn prepare_meeting_output_device(",
  "pub fn deliver_meeting_output_wav(",
  "pub fn cancel_meeting_output_for_generation(",
  ".build_output_stream(",
  "runtime_generation_is_authoritative",
  "meeting_output:delivery_deadline_exceeded",
  "decode_wav_bytes",
  "prepare_output_samples",
]);
requireMarkers(source.route, "matched read-only Meeting route", [
  "pub fn prepare_current_virtual_mic_route_for_meeting()",
  "pub fn bind_prepared_virtual_mic_route_to_generation",
  "pub fn get_virtual_mic_route_selection()",
  "pub fn get_virtual_mic_route_contract_status()",
  "fn matched_pair_identity(",
  "fn matched_pair_candidates(",
]);
forbidMarkers(source.route, "unowned persisted route preference/stub", [
  "VirtualMicRoutePreference",
  "virtual_mic_route_preference.json",
  "set_preferred_virtual_mic_route_devices",
  "prepare_virtual_mic_output_route_runtime_stub",
]);
requireMarkers(source.registry, "read-only route diagnostics surface", [
  "crate::commands::virtual_mic_route::get_virtual_mic_route_contract_status",
]);
forbidMarkers(source.registry, "mutating/manual route surface", [
  "set_preferred_virtual_mic_route_devices",
  "prepare_virtual_mic_output_route_runtime_stub",
  "dispatch_guarded_virtual_audio_route_provider",
]);

console.log("[virtual-route] Meeting session orchestration, guarded output facade, and native CPAL delivery ownership are explicit; Rust owns one matched generation-bound virtual pair, and retired Python/manual route owners are absent.");

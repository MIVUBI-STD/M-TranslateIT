import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { classifyVirtualMicRoute } from "../../src/app/bridge/virtualMicRoutePresentation.ts";
import type { VirtualMicRouteContractStatus } from "../../src/app/bridge/runtimeApi.ts";

const route = (route_ready: boolean, blocker: string, runtime_claim: string) =>
  ({ route_ready, blocker, runtime_claim } as VirtualMicRouteContractStatus);

test("virtual microphone evidence distinguishes missing devices from bridge failures", () => {
  assert.equal(classifyVirtualMicRoute(null), "checking");
  assert.equal(classifyVirtualMicRoute(null), "checking");
  assert.equal(classifyVirtualMicRoute(route(true, "", "virtual_mic_matched_route_pair_selection_source_side_not_audio_delivery_proof")), "ready");
  assert.equal(classifyVirtualMicRoute(route(false, "virtual_mic:no_supported_pair", "virtual_mic_matched_route_pair_selection_source_side_not_audio_delivery_proof")), "setup_needed");
  assert.equal(classifyVirtualMicRoute(route(false, "frontend_bridge_unavailable", "frontend_bridge_unavailable")), "unavailable");
  assert.equal(classifyVirtualMicRoute(route(false, "frontend_bridge_unavailable", "native_route")), "unavailable");
});

const source = (relative: string) =>
  readFileSync(new URL(`../../src/${relative}`, import.meta.url), "utf8");

test("Settings and First Setup own virtual-route evidence; Meeting only navigates for recovery", () => {
  const meeting = source("pages/Meeting.svelte");
  const settings = source("pages/Settings.svelte");
  const setup = source("pages/FirstSetup.svelte");
  const voice = source("components/setup/SetupVoiceChoice.svelte");

  assert.doesNotMatch(meeting, /classifyVirtualMicRoute\(routeStatus\)/);
  assert.match(meeting, /onNavigate\("settings"\)/);
  assert.doesNotMatch(meeting, /routeStatus\?\.ok === false/);
  assert.match(settings, /classifyVirtualMicRoute\(routeStatus\)/);
  assert.match(settings, /routeCheck === "unavailable" \? "Unable to check"/);
  assert.match(settings, /if \(!selected \|\| !devices\?\.ok\) return false/);
  assert.match(setup, /snapshot\?\.readiness\.meetingRouteReady === true && routeCheck === "ready"/);
  assert.match(setup, /routeCheck !== "ready"\) return/);
  assert.match(setup, /current && devices\?\.ok/);
  assert.match(voice, /routeCheck: VirtualMicRouteCheck/);
  assert.match(voice, /status=\{routeCheck === "ready" \? "Available"/);
});

test("failed My Voice recording bridge calls preserve authoritative state and navigation safety", () => {
  const api = source("app/bridge/myVoiceApi.ts");
  const voice = source("pages/MyVoice.svelte");
  const app = source("App.svelte");

  assert.match(api, /getState\(\): Promise<GuidedRecordingState \| null>/);
  assert.match(api, /recording: GuidedRecordingState \| null/);
  assert.match(api, /state: "frontend_bridge_error", message, recording: null/);
  assert.doesNotMatch(api, /\?\? unavailableState\(\)/);

  const start = voice.indexOf("async function applyResult(");
  const stateApply = voice.indexOf("applyState(result.recording)", start);
  const guard = voice.indexOf('result.state === "frontend_bridge_error" || !result.recording', start);
  assert.ok(guard > start && stateApply > guard, "unverified recording must never become a synthetic stopped state");
  assert.match(voice, /recordingStatusUnknown = true;\s*onRecordingChange\(true\);/);
  assert.match(voice, /if \(!next\) \{\s*recordingStatusUnknown = true;\s*onRecordingChange\(true\);/);
  assert.match(voice, /Retry status<\/button>/);
  assert.match(voice, /disabled=\{busy\} onclick=\{\(\) => void refresh\(\)\}>Retry status<\/button>/);
  assert.match(app, /myVoiceRecording \|\| snapshot\?\.resources\.owner_kind === "voice_recording"/);
  assert.match(app, /if \(voiceRecordingLock && next !== "my-voice"\)/);
  assert.match(app, /myVoiceRecording=\{voiceRecordingLock\}/);
});

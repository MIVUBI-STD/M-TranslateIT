import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const preview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_preview.rs", import.meta.url), "utf8");
const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url), "utf8");
const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
const status = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
const actor = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");

test("quick preview is an isolated one-WAV child with existing VoiceLab authority", () => {
  assert.match(preview, /begin_voice_lab_build\(\)/);
  assert.match(preview, /fail_voice_lab_build\(generation\)/);
  assert.match(preview, /ACTIVE_PREVIEW_GENERATION\.store\(generation, Ordering::Release\)/);
  assert.match(preview, /preview_dir\(\)/);
  assert.match(preview, /join\("QuickPreview"\)/);
  assert.match(preview, /const PREVIEW_WAV: &str = "quick_voice_preview\.wav"/);
  assert.match(preview, /voice_lab_quick_preview\.py/);
  assert.match(preview, /Timeout|TIMEOUT/);
  assert.doesNotMatch(preview, /promote_voice_actor_candidate|install_builtin_voice|select_builtin_voice|dispatch_product_intent/);
});

test("preview cancel is restricted to its own generation, never trained build", () => {
  assert.match(preview, /ACTIVE_PREVIEW_GENERATION\.load\(Ordering::Acquire\) == generation/);
  assert.match(preview, /current\.phase == "preparing"/);
  assert.match(preview, /request_voice_lab_build_cancel\(generation\)/);
  assert.match(preview, /child\.kill\(\)/);
  assert.match(actor, /approve_voice_lab_candidate\(reviewed_line_ids: Vec<u32>\)/);
  assert.doesNotMatch(preview, /approve_voice_lab_candidate/);
});

test("typed Quick Preview bridge exposes only generation, cancellation and reading", () => {
  for (const command of [
    "generate_voice_lab_quick_preview",
    "cancel_voice_lab_quick_preview",
    "get_voice_lab_quick_preview_audio",
  ]) {
    assert.ok(registry.includes(command), command);
    assert.ok(bridge.includes('"' + command + '"'), command);
  }
  assert.match(bridge, /QuickVoicePreviewResult/);
  assert.doesNotMatch(preview, /get_meeting_session_status|start_meeting_translation/);
});

test("Quick Preview remains training-only in the visible My Voice UI", () => {
  assert.match(ui, /Quick Voice Preview · Training only/);
  assert.match(ui, /Generate Quick Preview/);
  assert.match(ui, /Listen to Preview/);
  assert.match(ui, /Stop Preview/);
  assert.match(ui, /myVoiceBuildApi\.generateQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.cancelQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.getQuickPreviewAudio\(\)/);
  assert.match(ui, /quickPreviewBusy \|\| build\.preview_active/);
  assert.match(status, /preview_active: super::voice_lab_preview::quick_voice_preview_active\(\)/);
  assert.match(preview, /pub fn quick_voice_preview_active\(\) -> bool/);
  const start = ui.indexOf("async function generateQuickPreview()");
  const end = ui.indexOf("async function startBuild()", start);
  const block = ui.slice(start, end);
  assert.doesNotMatch(block, /onMeetingVoiceChanged|approve\(|selectBuiltin|dispatchProductIntent/);
});

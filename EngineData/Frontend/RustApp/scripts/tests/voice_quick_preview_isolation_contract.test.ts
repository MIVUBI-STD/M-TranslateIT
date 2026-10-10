import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const preview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_preview.rs", import.meta.url), "utf8");
const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url), "utf8");
const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
const status = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
const phase = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/phase.rs", import.meta.url), "utf8");
const releaseConfig = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.release.conf.json", import.meta.url), "utf8"));
const releaseContract = readFileSync(new URL("../validate_release_package_contract.mjs", import.meta.url), "utf8");
const releasePayload = readFileSync(new URL("../validate_release_payload.mjs", import.meta.url), "utf8");
const previewChild = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_quick_preview.py", import.meta.url), "utf8");
const buildChild = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_build.py", import.meta.url), "utf8");
const actor = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
const mutations = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/mutations.rs", import.meta.url), "utf8");
const recordingUi = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
const guidedAudio = readFileSync(new URL("../../src-tauri/src/engine/audio/guided_take.rs", import.meta.url), "utf8");
const evaluation = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/evaluation.rs", import.meta.url), "utf8");
const synthesis = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits_build.py", import.meta.url), "utf8");

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
  assert.match(actor, /approve_voice_lab_candidate\(\s*reviewed_line_ids: Vec<u32>,\s*quality_confirmed: bool,/);
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


test("preview success rechecks cancellation after child exit", () => {
  const exit = preview.indexOf("Ok(Some(status))");
  const finalCheck = preview.lastIndexOf("let authority = current_voice_lab_build_snapshot();");
  const output = preview.indexOf("let info = fs::metadata(&file)");
  assert.ok(exit >= 0 && finalCheck > exit && output > finalCheck);
  assert.match(preview.slice(finalCheck, output), /authority\.cancel_requested/);
});


test("reference ranking belongs to the Python training selector", () => {
  assert.match(preview, /accepted_guided_recordings\(\)/);
  assert.match(preview, /--reference-candidates-stdin/);
  assert.match(preview, /MAX_REFERENCE_PAYLOAD_BYTES/);
  assert.match(preview, /Stdio::piped\(\)/);
  assert.match(preview, /stdin\.write_all\(&payload\)/);
  assert.match(preview, /let writer = thread::spawn/);
  assert.match(preview, /writer\.join\(\)/);
  assert.doesNotMatch(preview, /command\.arg\("--reference-candidate"\)/);
  assert.match(preview, /"line_id": take.line_id/);
  assert.doesNotMatch(preview, /TARGET_REFERENCE_BYTES|min_by_key/);
});

test("preview readiness is invalidated when accepted recording revision or training changes", () => {
  assert.match(ui, /let seenRefreshRevision = refreshRevision/);
  assert.match(ui, /if \(revision !== seenRefreshRevision\)/);
  assert.match(ui, /seenRefreshRevision = revision;\s*quickPreviewReady = false;\s*stopAudio\(\)/);
  assert.match(ui, /if \(next.active && !next.preview_active\)/);
  assert.match(ui, /quickPreviewReady = false;\s*stopAudio\(\)/);
});

test("preview phase cannot be reconciled from stale trained-build status", () => {
  assert.match(status, /mod phase;/);
  assert.match(status, /use phase::reconcile_phase;/);
  assert.match(phase, /fn reconcile_phase\(/);
  assert.match(phase, /if super::super::voice_lab_preview::quick_voice_preview_active\(\) \{\s*return;/);
  assert.match(status, /let child = if preview_active \{ None \} else \{ child_status\(&paths\) \}/);
  assert.match(status, /"Creating Quick Preview locally\."/);
  assert.match(status, /"Stopping Quick Preview safely\."/);
});

test("preview cancellation and timeout remain effective while input is written", () => {
  const writer = preview.indexOf("let writer = thread::spawn");
  const deadline = preview.indexOf("let deadline = Instant::now() + TIMEOUT");
  const monitor = preview.indexOf("let execution: Result<(), String> = loop");
  const join = preview.indexOf("writer.join()");
  assert.ok(deadline >= 0 && deadline < writer && writer < monitor && monitor < join);
  assert.match(preview.slice(monitor, join), /authority\.cancel_requested/);
  assert.match(preview.slice(monitor, join), /Instant::now\(\) >= deadline/);
  assert.match(preview.slice(monitor, join), /child\.kill\(\)/);
  assert.match(preview, /fail_voice_lab_build\(generation\)[\s\S]*ACTIVE_PREVIEW_GENERATION\.store\(0, Ordering::Release\)/);
});

test("Setup packages both My Voice children and their shared canonical training module", () => {
  for (const name of ["voice_lab_quick_preview.py", "voice_lab_gpt_sovits_build.py"]) {
    const from = `../../../Backend/LocalWorker/WorkerRuntime/${name}`;
    const to = `EngineData/Backend/LocalWorker/WorkerRuntime/${name}`;
    assert.equal(releaseConfig.bundle.resources[from], to, `missing packaged WorkerRuntime module: ${name}`);
    assert.ok(releaseContract.includes(JSON.stringify(from)), `release contract must pin: ${name}`);
    assert.ok(releasePayload.includes(JSON.stringify(name)), `payload validator must require: ${name}`);
  }
  assert.match(previewChild, /from voice_lab_gpt_sovits_build import select_reference/);
  assert.match(buildChild, /from voice_lab_gpt_sovits_build import build_candidate/);
});

test("trained voice approval requires audible review and affirmative human quality confirmation", () => {
  assert.match(ui, /qualityConfirmed = \$state\(false\)/);
  assert.match(ui, /!evaluationReviewComplete \|\| !qualityConfirmed/);
  assert.match(ui, /myVoiceBuildApi\.approve\(reviewedLineIds, qualityConfirmed, build\.evaluation_review_id\)/);
  assert.match(bridge, /reviewedLineIds,\s*qualityConfirmed,\s*reviewId,/);
  assert.match(mutations, /approve_voice_lab_candidate\(reviewed_line_ids, quality_confirmed, review_id\)/);
  assert.match(actor, /if !evaluation_review_complete\(&evaluation, &reviewed_line_ids\)/);
  assert.match(actor, /if !quality_confirmed\s*\{\s*return result\(\s*false,\s*"evaluation_quality_confirmation_required"/);
  assert.ok(actor.indexOf("if !quality_confirmed") < actor.indexOf("match promote_voice_actor_candidate"));
  assert.match(ui, /Create Another Candidate/);
});

test("guided-take signal reasons provide actionable recording guidance without a new scorer", () => {
  for (const reason of ["rejected_silence", "severe_clipping", "signal_too_low", "dc_offset_too_high"]) {
    assert.ok(guidedAudio.includes("voice_lab:take_signal_unusable:" + reason), reason);
    assert.ok(recordingUi.includes("voice_lab:take_signal_unusable:" + reason), reason);
  }
  assert.match(recordingUi, /recordingQualityGuidance\(recordingState\.pending_review\.quality_blocker\)/);
});

test("My Voice listening and approval require the same synthesized review identity", () => {
  assert.match(status, /evaluation_review_id: evaluation\.as_ref\(\)\.map\(/);
  assert.match(ui, /build\.evaluation_review_id !== next\.evaluation_review_id/);
  assert.match(ui, /myVoiceBuildApi\.getEvaluationAudio\(lineId, reviewId, expectedSha256\)/);
  assert.match(bridge, /get_voice_lab_evaluation_audio", \{ lineId, reviewId \}/);
  assert.match(actor, /if review_id != evaluation\.review_id/);
  assert.match(actor, /if review_id != manifest\.review_id/);
});

test("review playback hashes the actual returned WAV and never accepts a stale clip", () => {
  assert.match(synthesis, /"sha256": str\(sample\["sha256"\]\)/);
  assert.match(evaluation, /pub sha256: String/);
  assert.match(evaluation, /!valid_sha256\(&sample\.sha256\)/);
  assert.match(bridge, /subtle\.digest\("SHA-256", bytes\)/);
  assert.match(bridge, /actual === expectedSha256 \? bytes : null/);
  assert.match(ui, /sample\.line_id === lineId\)\?\.sha256/);
  assert.match(ui, /reviewId === build\.evaluation_review_id && !reviewedLineIds\.includes\(lineId\)/);
});

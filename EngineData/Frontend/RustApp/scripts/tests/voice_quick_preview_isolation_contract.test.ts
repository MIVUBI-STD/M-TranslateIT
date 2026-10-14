import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const preview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_preview.rs", import.meta.url), "utf8");
const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url), "utf8");
const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
const quickPreviewUi = readFileSync(new URL("../../src/components/my-voice/QuickVoicePreview.svelte", import.meta.url), "utf8");
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

test("My Voice UI has one collapsible voice selector and a stage-aware recording workspace", () => {
  const selector = readFileSync(new URL("../../src/components/my-voice/BuiltinVoicePicker.svelte", import.meta.url), "utf8");
  assert.match(selector, /Current Meeting voice/);
  assert.match(selector, /aria-expanded={choicesOpen \|\| builtinPendingId !== null}/);
  assert.match(selector, /hidden={!choicesOpen && !builtinPendingId}/);
  assert.match(recordingUi, /Create My Voice/);
  assert.match(recordingUi, /const activeStage = \$derived/);
  assert.match(recordingUi, /hidden={!showRecordPanel}/);
  assert.match(recordingUi, /hidden={!showBuildPanel}/);
  assert.match(recordingUi, /<MyVoiceBuild \{onNotice\}/);
  assert.match(ui, /onBuildStatus\?\.\(next\)/);
  assert.match(recordingUi, /Browse lines/);
  // The recording catalog contains 600 prompts, but a single build still
  // accepts at most 128 takes. Never hard-code the old prompt count in UI.
  assert.match(recordingUi, /You don't have to record all \{recordingState\.lines\.length\} sentences\./);
  assert.doesNotMatch(recordingUi, /You don't have to record all 128/);
  assert.match(status, /takes\.len\(\) <= MAX_TRAINING_TAKES/);
  assert.match(recordingUi, /!buildSnapshot\.can_build/);
  assert.match(recordingUi, /buildSnapshot\.minimum_duration_ms/);
});

test("keyboard focus follows explicit recording and Browse lines actions, not background polling", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.match(page, /import \{ onMount, tick \} from "svelte"/);
  assert.match(page, /bind:this=\{lineHeading\}/);
  assert.match(page, /bind:this=\{reviewReplayButton\}/);
  assert.match(page, /bind:this=\{recordActionButton\}/);
  assert.match(page, /async function chooseBrowseLine\(lineId: number\)/);
  assert.match(page, /browseLinesOpen = false;[\s\S]*?await tick\(\);[\s\S]*?lineHeading\?\.focus\(\)/);
  assert.match(page, /aria-label=\{`Line \$\{line\.line_id\}, \$\{line\.accepted/);
  assert.match(page, /result\.state === "needs_review"[\s\S]*?reviewReplayButton\?\.focus\(\)/);
  assert.match(page, /selectedPane = "record";[\s\S]*?recordActionButton\?\.focus\(\)/);
  const status = page.slice(page.indexOf("function applyState("), page.indexOf("async function checkMicrophone()"));
  assert.doesNotMatch(status, /\.focus\(/);
  assert.match(page, /xl:grid-cols-\[minmax\(0,1fr\)_280px\]/);
});

test("native Stop and Retry remain visible when recording and creation status disagree", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.match(page, /showRecordPanel = \$derived\(activeStage === "record" \|\| isRecording \|\| recordingStatusUnknown\)/);
  assert.match(page, /showBuildPanel = \$derived\(/);
  assert.match(page, /buildStatusUnavailable \|\| buildSnapshot\?\.active === true \|\| buildSnapshot\?\.preview_active === true/);
  assert.match(page, /hidden={!showRecordPanel}/);
  assert.match(page, /hidden={!showBuildPanel}/);
  assert.match(page, /A status check needs attention/);
  assert.match(page, /if \(busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /async function acceptTake\(\): Promise<void> \{\s*if \(busy \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /disabled=\{busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active === true/);
  assert.match(page, /<MyVoiceBuild \{onNotice\}/);
});

test("completed evaluations take priority over Create and unknown status locks mutations", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  const build = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const stage = page.slice(page.indexOf("const activeStage = $derived"), page.indexOf("const acceptedCount = $derived"));
  assert.ok(stage.indexOf('selectedPane === "record"') < stage.indexOf("buildSnapshot?.evaluation_ready"));
  assert.ok(stage.indexOf("buildSnapshot?.evaluation_ready") < stage.indexOf('selectedPane === "create"'));
  assert.match(page, /voiceChangesLocked=\{voiceChangesLocked \|\| buildStatusUnavailable/);
  assert.match(page, /if \(busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /disabled=\{recordingStatusUnknown \|\| buildStatusUnavailable/);
  assert.match(build, /if \(busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| quickPreviewBusy/);
  assert.match(build, /if \(busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| build\.active/);
  assert.match(build, /disabled=\{busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| !evaluationReviewComplete/);
  assert.match(build, /onBuildStatusUnavailable\?\.\(\)/);
});

test("Quick Preview remains training-only in the visible My Voice UI", () => {
  assert.match(ui, /<QuickVoicePreview/);
  assert.match(quickPreviewUi, /not for meetings|never usable in Meeting/i);
  assert.match(quickPreviewUi, /Generate Quick Preview/);
  assert.match(quickPreviewUi, /Listen to Preview/);
  assert.match(ui, /Stop Preview/);
  assert.match(ui, /myVoiceBuildApi\.generateQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.cancelQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.getQuickPreviewAudio\(\)/);
  assert.match(quickPreviewUi, /quickPreviewBusy \|\| build\.preview_active/);
  assert.match(ui, /bind:previewAuthorized/);
  assert.match(status, /let preview_active = super::voice_lab_preview::quick_voice_preview_active\(\);/);
  assert.match(status, /VoiceLabBuildStatus \{[\s\S]*?\n\s*preview_active,/);
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
  assert.match(ui, /let seenRefreshRevision = untrack\(\(\) => refreshRevision\)/);
  assert.match(ui, /if \(revision !== seenRefreshRevision\)/);
  assert.match(ui, /seenRefreshRevision = revision;\s*quickPreviewReady = false;\s*reviewedLineIds = \[\];\s*qualityConfirmed = false;\s*stopAudio\(\)/);
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

test("My Voice microphone check reuses the selected physical input functional probe", () => {
  assert.match(recordingUi, /runtimeApi\.loadSettings\(\)/);
  assert.match(recordingUi, /probeProductAudioDevice\("microphone", settings\.audio\.input_device_id\)/);
  assert.match(recordingUi, /Record a sample to check its sound quality/);
  assert.match(recordingUi, /microphoneCheck === "checking"/);
  assert.doesNotMatch(recordingUi, /getUserMedia\(/);
});

test("guided take recording environment hints are advisory and do not rewrite the voice", () => {
  assert.match(guidedAudio, /fn recording_environment_hints\(/);
  assert.match(guidedAudio, /voice_lab:review:long_edge_pause/);
  assert.match(guidedAudio, /voice_lab:review:possible_room_sound/);
  assert.match(recordingUi, /voice_lab:review:long_edge_pause/);
  assert.match(recordingUi, /voice_lab:review:possible_room_sound/);
  assert.match(guidedAudio, /recording_environment_hints\(&samples_mono, &mut review_advice\)/);
  assert.match(guidedAudio, /return; \/\/ No comparable quiet context/);
  assert.match(guidedAudio, /samples_mono: Vec<f32>/);
  assert.match(guidedAudio, /quality_blocker = guided_take_quality_blocker/);
});

test("guided take offers conservative replay advice without changing the approved engine or hard quality gates", () => {
  const recording = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_recording.rs", import.meta.url), "utf8");
  const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceApi.ts", import.meta.url), "utf8");
  assert.match(guidedAudio, /review_advice: Vec<String>/);
  assert.match(guidedAudio, /voice_lab:review:possible_clipping/);
  assert.match(recording, /fn unusually_short_for_line\(/);
  assert.match(recording, /voice_lab:review:verify_all_words/);
  assert.match(recording, /review\.quality_blocker\.is_empty\(\) && unusually_short_for_line/);
  assert.match(bridge, /review_advice: string\[\]/);
  assert.match(recordingUi, /recordingReviewAdvice\(advice\)/);
  assert.match(recordingUi, /read every word exactly as shown/);
  assert.match(recording, /fn accept_voice_lab_guided_take/);
  assert.doesNotMatch(recording, /run.*asr|whisper.*transcribe/i);
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
  // A stale onended callback must not satisfy the held-out review. The
  // active player, request generation and current review ID must all match.
  const playback = ui.slice(ui.indexOf("async function playEvaluation("));
  const ended = playback.slice(playback.indexOf("player.onended ="), playback.indexOf("player.onerror ="));
  assert.match(ended, /audio !== player \|\| playbackVersion !== audioRequestVersion \|\| reviewId !== build\.evaluation_review_id/);
  assert.ok(ended.indexOf("audio !== player") < ended.indexOf("reviewedLineIds ="));
});

test("My Voice puts the compact current Meeting voice above an accessible three-stage workspace and advanced libraries below it", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  const picker = readFileSync(new URL("../../src/components/my-voice/BuiltinVoicePicker.svelte", import.meta.url), "utf8");
  const versions = readFileSync(new URL("../../src/components/my-voice/VoiceVersionBrowser.svelte", import.meta.url), "utf8");

  const current = page.indexOf("<BuiltinVoicePicker");
  const create = page.indexOf('aria-labelledby="my-voice-create-title"');
  const library = page.indexOf('aria-label="Recording library"');
  const history = page.indexOf("<VoiceVersionBrowser");
  const backup = page.indexOf("<VoiceBackupRestore");
  assert.ok(current >= 0 && current < create, "current voice stays above creation");
  assert.ok(create < library && library < history && history < backup,
    "record/create/review must precede secondary saved-version and backup controls");
  assert.match(picker, /Current Meeting voice/);
  assert.match(picker, /p-3 sm:p-4/);
  assert.match(picker, /onPreview=|onSelect|onConfirm/);
  assert.match(page, /onPreview=\{previewBuiltin\}/);
  assert.match(page, /onSelect=\{selectBuiltin\}/);
  assert.match(page, /onConfirm=\{confirmPendingBuiltin\}/);
  assert.match(page, /onSelectPersonal=\{selectStoredPersonalVoice\}/);
  assert.match(page, /onVoiceActivated=\{async \(versionId, message\)/);
  assert.match(versions, /<details class="mt-2 text-xs/);
  assert.match(versions, /\{voice\.version_id\}<\/code>/);
  assert.doesNotMatch(versions, /created_at|createdAt|last_used_at/,
    "do not invent dates or provenance which the version API cannot supply");

  const nav = page.slice(page.indexOf('<nav class="mt-4'),
    page.indexOf("hidden={!showRecordPanel}"));
  assert.match(nav, /<nav class=/);
  assert.match(nav, /aria-current=\{activeStage === "review" \? "step"/);
  assert.match(nav, /disabled=\{busy \|\| recordingStatusUnknown \|\| buildStatusUnavailable/);
  assert.match(nav, /onclick=\{\(\) => \{ selectedPane = "record"; \}\}/);
  assert.match(nav, /onclick=\{\(\) => \{ selectedPane = "create"; \}\}/);
  assert.match(nav, /onclick=\{\(\) => \{ selectedPane = "review"; \}\}/);
  assert.match(page, /\{#if buildSnapshot\.evaluation_ready && !isRecording && pendingLineId === null\}/);
  assert.match(page, /Return to Review/);
  assert.match(page, /let selectedPane = \$state<"auto" \| "record" \| "create" \| "review">/);
});

test("My Voice keeps audio load feedback separate from verified playback and invalidates pending requests", () => {
  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const preview = readFileSync(new URL("../../src/components/my-voice/QuickVoicePreview.svelte", import.meta.url), "utf8");
  const stop = ui.slice(ui.indexOf("function stopAudio()"), ui.indexOf("function schedulePoll()"));
  const quick = ui.slice(ui.indexOf("async function playQuickPreview("), ui.indexOf("async function startBuild("));
  const evaluation = ui.slice(ui.indexOf("async function playEvaluation("), ui.indexOf("  $effect("));
  assert.match(ui, /let evaluationLoadingLineId = \$state<number \| null>\(null\)/);
  assert.match(ui, /let quickPreviewLoading = \$state\(false\)/);
  assert.match(stop, /audioRequestVersion \+= 1/);
  assert.match(stop, /evaluationLoadingLineId = null/);
  assert.match(stop, /quickPreviewLoading = false/);
  assert.match(quick, /quickPreviewLoading = true/);
  assert.match(quick, /finally \{\s*if \(!disposed && requestVersion === audioRequestVersion\) quickPreviewLoading = false/);
  assert.match(evaluation, /evaluationLoadingLineId = lineId/);
  assert.match(evaluation, /finally \{\s*if \(!disposed && requestVersion === audioRequestVersion\) evaluationLoadingLineId = null/);
  assert.match(ui, /aria-busy=\{evaluationLoadingLineId === sample\.line_id\}/);
  assert.match(ui, /evaluationLoadingLineId === sample\.line_id \? "Loading audio\.\.\."/);
  assert.match(ui, /\{quickPreviewLoading\}/);
  assert.match(preview, /aria-busy=\{quickPreviewLoading\}/);
  assert.match(preview, /quickPreviewLoading \? "Loading audio\.\.\."/);
  assert.match(evaluation, /reviewId !== build\.evaluation_review_id/);
  assert.match(evaluation, /audio !== player \|\| playbackVersion !== audioRequestVersion/);
});

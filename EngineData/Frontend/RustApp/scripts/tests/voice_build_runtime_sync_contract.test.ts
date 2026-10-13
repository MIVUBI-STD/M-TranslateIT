import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const build = readFileSync(
  new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url),
  "utf8",
);
const runtimeEvents = readFileSync(
  new URL("../../src-tauri/src/engine/runtime_events.rs", import.meta.url),
  "utf8",
);
const api = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);

test("Voice build terminal state emits a global backend runtime event", () => {
  assert.match(build, /emit_voice_build_runtime_event\(event_reason, generation\)/);
  assert.match(build, /"voice_build_completed"/);
  assert.match(build, /"voice_build_cancelled"/);
  assert.match(build, /"voice_build_failed"/);
  assert.match(runtimeEvents, /VOICE_BUILD_RUNTIME_EVENT/);
  assert.match(runtimeEvents, /translateit:\/\/voice-build-runtime/);
});

test("application runtime subscription folds Voice build completion into canonical reconciliation", () => {
  assert.match(api, /listen<VoiceBuildRuntimeEvent>/);
  assert.match(api, /translateit:\/\/voice-build-runtime/);
  assert.match(api, /getApplicationSnapshot\(\)/);
  assert.match(api, /listener\(\{ reason: event\.payload\.reason, snapshot \}\)/);
});

test("partial combined subscription installation cleans up the first listener", () => {
  assert.match(api, /catch \(error\) \{\s*disposed = true;\s*applicationStop\(\);\s*throw error;/);
});

test("combined runtime subscription suppresses callbacks after unsubscribe", () => {
  assert.match(api, /let disposed = false;/);
  assert.match(api, /listen<ApplicationRuntimeEvent>[\s\S]*?if \(!disposed\) listener\(event\.payload\)/);
  assert.match(api, /getApplicationSnapshot\(\)\.then\(\(snapshot\) => \{\s*if \(disposed\) return;/);
  assert.match(api, /return \(\) => \{\s*disposed = true;\s*applicationStop\(\);\s*voiceBuildStop\(\);/);
});

test("My Voice polling and audio reads cannot resume after view disposal", () => {
  const build = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.ok(build.includes("if (disposed || !build.active) return;"));
  assert.match(build, /disposed = true;\s*if \(timer\) clearTimeout\(timer\);/);
  assert.ok(build.includes("if (disposed || reviewId !== build.evaluation_review_id) return;"));
  assert.ok(page.includes("if (disposed) return;"));
  assert.match(page, /disposed = true;\s*stopReplay\(\);/);
});

test("approved My Voice distinguishes selected actor from separately saved copy and preserves archive warnings", () => {
  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const warning = build.match(/result\(true, "approved_archive_attention",[\s\S]{0,250}/)?.[0] ?? "";
  assert.match(warning, /separate saved copy could not be verified/);
  assert.match(ui, /case "approved_archive_attention":\s*return result\.message/);
  assert.match(ui, /result\.ok && \["approved", "approved_cleanup_attention", "approved_archive_attention"\]\.includes\(result\.state\)/);
  assert.match(ui, /\{#if build\.selected_voice_kind === "personal"\}/);
  assert.match(ui, /Personal voice selected<\/span>/);
  assert.match(ui, /else if build\.saved_personal_voice_available/);
});

test("a verified My Voice action clears prior unavailable-bridge warning without erasing unknown state", () => {
  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  assert.match(ui, /if \(result\.build\.phase === "unavailable"\) \{\s*statusCheckUnavailable = true;/);
  assert.match(ui, /else \{\s*\/\/ A verified native response supersedes a previous bridge failure\.\s*statusCheckUnavailable = false;\s*applyStatus\(result\.build\);/);
});

test("approved My Voice cannot replace the selected actor during an active Meeting", () => {
  const start = build.indexOf("pub fn approve_voice_lab_candidate(");
  const end = build.indexOf("pub fn get_voice_lab_evaluation_audio(", start);
  assert.ok(start >= 0 && end > start);
  const approval = build.slice(start, end);
  const meetingChecks = [...approval.matchAll(/if meeting_blocks_voice_change\(\)/g)];
  assert.ok(meetingChecks.length >= 2, "Meeting guard must be checked before promotion and after selection lock");
  assert.ok(approval.indexOf('return result(false, "meeting_active"') >= 0);
  assert.ok(approval.lastIndexOf("if meeting_blocks_voice_change()")
    < approval.indexOf("match promote_voice_actor_candidate("));
  assert.match(approval, /if !evaluation_review_complete\(&evaluation, &reviewed_line_ids\)/);
  assert.match(approval, /if !quality_confirmed/);

  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const parent = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.match(parent, /<MyVoiceBuild \{onNotice\}\s*voiceChangesLocked=\{voiceChangesLocked \|\| recordingStatusUnknown \|\| isRecording \|\| pendingLineId !== null\}/);
  assert.match(ui, /voiceChangesLocked\?: boolean/);
  assert.match(ui, /async function approve\(\): Promise<void> \{\s*if \(busy \|\| statusCheckUnavailable \|\| voiceChangesLocked/);
  assert.match(ui, /disabled=\{busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| !evaluationReviewComplete \|\| !qualityConfirmed\}/);
  assert.match(ui, /case "meeting_active":\s*return "Stop Meeting translation before selecting My Voice\."/);
});

test("voice switching attempts validated single-backup recovery before declaring a permanent recovery block", () => {
  const voiceStorage = readFileSync(new URL("../../src-tauri/src/commands/voice_lab/storage.rs", import.meta.url), "utf8");
  const voiceOwner = readFileSync(new URL("../../src-tauri/src/commands/voice_lab.rs", import.meta.url), "utf8");
  assert.match(voiceOwner, /recover_selected_actor_if_missing/);
  assert.match(voiceStorage, /fn recover_selected_actor_if_missing\(/);
  assert.match(voiceStorage, /paths\.approved_actor_dir\.exists\(\) \|\| pending\.len\(\) != 1/);
  assert.match(voiceStorage, /metadata\.file_type\(\)\.is_symlink\(\) \|\| !metadata\.is_dir\(\)/);
  assert.match(voiceStorage, /validate_actor_package\(backup\)\?;/);
  assert.match(voiceStorage, /fs::rename\(backup, &paths\.approved_actor_dir\)/);

  for (const name of ["pub fn select_builtin_voice(", "pub fn select_stored_personal_voice()"]) {
    const start = build.indexOf(name);
    const end = build.indexOf("\n}", start);
    assert.ok(start >= 0 && end > start);
    const action = build.slice(start, end);
    assert.ok(action.indexOf("recover_selected_actor_if_missing(")
      < action.indexOf("voice_selection_recovery_pending("));
    assert.match(action, /if meeting_blocks_voice_change\(\)/);
    assert.match(action, /voice_selection_guard\(\)/);
  }
});

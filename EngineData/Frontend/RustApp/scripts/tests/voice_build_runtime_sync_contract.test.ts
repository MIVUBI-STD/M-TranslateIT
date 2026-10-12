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
  assert.match(ui, /if \(build\.selected_voice_kind === "personal"\)/);
  assert.match(ui, />Personal voice selected<\/span>/);
  assert.match(ui, /else if build\.saved_personal_voice_available/);
});

test("a verified My Voice action clears prior unavailable-bridge warning without erasing unknown state", () => {
  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  assert.match(ui, /if \(result\.build\.phase === "unavailable"\) \{\s*statusCheckUnavailable = true;/);
  assert.match(ui, /else \{\s*\/\/ A verified native response supersedes a previous bridge failure\.\s*statusCheckUnavailable = false;\s*applyStatus\(result\.build\);/);
});

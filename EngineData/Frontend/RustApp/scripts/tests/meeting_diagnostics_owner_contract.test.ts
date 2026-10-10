import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
const settingsDiagnostics = readFileSync(new URL("../../src/components/settings/SettingsDiagnostics.svelte", import.meta.url), "utf8");
const reliability = readFileSync(new URL("../../src/components/settings/RuntimeReliabilityDiagnostics.svelte", import.meta.url), "utf8");
const performance = readFileSync(new URL("../../src/components/settings/MeetingPerformanceDiagnostics.svelte", import.meta.url), "utf8");

function rust(path: string): string {
  return readFileSync(
    new URL(`../../src-tauri/src/commands/${path}`, import.meta.url),
    "utf8",
  );
}

test("Meeting diagnostics are scoped to the canonical Meeting runtime owner", () => {
  for (const path of [
    "runtime_watchdog.rs",
    "device_loss_guard.rs",
    "long_session_health.rs",
  ]) {
    const source = rust(path);
    assert.match(source, /APPLICATION_MEETING_OWNER_ID/, path);
    assert.match(source, /owner_id\.as_deref\(\)/, path);
  }
});

test("long-session counters do not attribute generic runtime sessions to Meeting", () => {
  const source = rust("long_session_health.rs");
  assert.match(source, /let meeting_owned/);
  assert.match(source, /if meeting_owned/);
  assert.match(source, /if !meeting_owned/);
});

test("Check Again refreshes the existing Reliability panel without background polling", () => {
  assert.match(settings, /diagnosticsRefreshRevision \+= 1/);
  assert.match(settings, /refreshRevision=\{diagnosticsRefreshRevision\}/);
  assert.match(settingsDiagnostics, /<RuntimeReliabilityDiagnostics \{refreshRevision\}/);
  assert.match(reliability, /\$effect\(\(\) =>/);
  assert.match(reliability, /revision === refreshRevision/);
  assert.match(reliability, /if \(!isCurrent\(\)\) return/);
  assert.doesNotMatch(reliability, /setInterval|setTimeout/);
});

test("unavailable recovery never appears as clean, and audio diagnosis uses actual stages", () => {
  assert.match(reliability, /loading \|\| !recovery \? "Not checked"/);
  assert.match(reliability, /!recovery\.cleanup_ok \? "Not verified"/);
  assert.match(reliability, /sanitizeDiagnosticText\(incidents\.incidents\[0\]\.note\)/);
  assert.match(performance, /timing\?\.outbound_latency_ms == null/);
  assert.match(performance, /Largest recorded stage:/);
  assert.match(performance, /not proof of the underlying bottleneck/);
});

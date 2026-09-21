import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

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

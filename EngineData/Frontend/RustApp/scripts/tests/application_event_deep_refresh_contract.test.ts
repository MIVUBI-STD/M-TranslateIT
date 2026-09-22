import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);

test("ordinary runtime events may reuse cached deep readiness", () => {
  for (const reason of [
    "stop_meeting",
    "start_mic_test",
    "stop_mic_test",
    "select_audio_device",
    "save_settings",
    "apply_meeting_preset",
    "start_voice_recording",
    "stop_voice_recording",
  ]) {
    assert.match(facade, new RegExp(`"${reason}"`));
  }
  assert.match(facade, /WORKER_REUSE_SAFE_REASONS/);
  assert.match(facade, /VOICE_REUSE_SAFE_REASONS/);
});

test("unknown runtime event reasons fail closed into deep refresh", () => {
  assert.match(
    facade,
    /!WORKER_REUSE_SAFE_REASONS\.has\(reason \?\? ""\)/,
  );
  assert.match(
    facade,
    /!VOICE_REUSE_SAFE_REASONS\.has\(reason \?\? ""\)/,
  );
});

test("event reconciliation can reuse previous worker and voice readiness", () => {
  assert.match(facade, /Promise\.resolve\(previous\.workerStatus\)/);
  assert.match(
    facade,
    /Promise\.resolve\(previous\.readiness\.approvedVoiceReady\)/,
  );
});

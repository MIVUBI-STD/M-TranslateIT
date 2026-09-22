import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const applicationFrontend = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);
const applicationBackend = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/events.rs", import.meta.url),
  "utf8",
);
const meetingFrontend = readFileSync(
  new URL("../../src/app/runtime/reliabilityMonitor.ts", import.meta.url),
  "utf8",
);
const meetingBackend = readFileSync(
  new URL("../../src-tauri/src/engine/runtime_events.rs", import.meta.url),
  "utf8",
);
const voiceFrontend = readFileSync(
  new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url),
  "utf8",
);

test("application runtime event name and payload fields stay aligned", () => {
  assert.match(applicationFrontend, /translateit:\/\/application-runtime/);
  assert.match(applicationBackend, /translateit:\/\/application-runtime/);
  for (const field of ["reason", "snapshot"]) {
    assert.match(applicationFrontend, new RegExp(`\\b${field}:`));
    assert.match(applicationBackend, new RegExp(`\\b${field}:`));
  }
});

test("Meeting runtime event name and lightweight payload fields stay aligned", () => {
  assert.match(meetingFrontend, /translateit:\/\/meeting-runtime/);
  assert.match(meetingBackend, /translateit:\/\/meeting-runtime/);
  for (const field of ["revision", "reason", "session_id", "sequence"]) {
    assert.match(meetingFrontend, new RegExp(`\\b${field}:`));
    assert.match(meetingBackend, new RegExp(`\\b${field}:`));
  }
  assert.doesNotMatch(meetingBackend, /translated_text|source_text/);
});


test("Voice build runtime event name and payload fields stay aligned", () => {
  assert.match(voiceFrontend, /translateit:\/\/voice-build-runtime/);
  assert.match(meetingBackend, /translateit:\/\/voice-build-runtime/);
  for (const field of ["revision", "reason", "generation"]) {
    assert.match(voiceFrontend, new RegExp(`\\b${field}:`));
    assert.match(meetingBackend, new RegExp(`\\b${field}:`));
  }
});

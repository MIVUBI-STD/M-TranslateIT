import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const lifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
const committed = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url), "utf8");
const exporter = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/transcript_export.rs", import.meta.url), "utf8");
const ui = readFileSync(new URL("../../src/components/meeting/MeetingTranscriptExport.svelte", import.meta.url), "utf8");

test("ended transcript is retained only in memory before committed state cleanup", () => {
  assert.match(lifecycle, /retain_committed_turns_for_export\(&session_id\)/);
  assert.match(committed, /LAST_ENDED_MEETING_TRANSCRIPT/);
  assert.match(committed, /snapshot\.has_session = false/);
  assert.match(committed, /reset_committed_turns[\s\S]*\*ended = None/);
  assert.doesNotMatch(committed, /fs::write|File::create/);
});

test("transcript export is explicit and limited to markdown or txt", () => {
  assert.match(ui, /Export transcript/);
  assert.match(ui, /Markdown/);
  assert.match(ui, />TXT</);
  assert.doesNotMatch(ui, /onMount\([\s\S]{0,250}exportMeetingTranscript/);
  assert.match(exporter, /"txt"/);
  assert.match(exporter, /"md" \| "markdown"/);
  assert.doesNotMatch(exporter, /pdf|docx|srt|vtt|csv/i);
});

test("export contains source and translated text and writes only to app-owned saved project", () => {
  assert.match(exporter, /Source/);
  assert.match(exporter, /Translation/);
  assert.match(exporter, /user_saved_dir/);
  assert.match(exporter, /join\("Transcripts"\)/);
});

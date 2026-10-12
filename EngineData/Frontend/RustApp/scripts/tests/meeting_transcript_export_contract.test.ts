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

test("export feedback remains local and avoids putting absolute file paths in header", () => {
  const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");
  assert.doesNotMatch(ui, /onNotice\(result\.file_path/);
  assert.match(ui, /onNotice\("Transcript exported\."\)/);
  assert.match(ui, /Copy location/);
  assert.match(ui, /navigator\.clipboard\.writeText\(receipt\.path\)/);
  assert.doesNotMatch(header, /title=\{notice\}/);
});
test("partial transcript export shows an explicit warning", () => {
  assert.match(ui, /status\?\.truncated && !receipt/);
  assert.match(ui, /result\.truncated/);
  assert.match(ui, /Earlier phrases were not retained/);
});

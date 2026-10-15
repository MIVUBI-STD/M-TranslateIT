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

test("manual exports are isolated in a per-session folder without automatic session history writes", () => {
  assert.match(exporter, /\.join\(format!\("Meeting_\{session\}"\)\)/);
  assert.match(exporter, /format!\("Transcript_\{\}\.\{\}", unix_ms\(\), normalized\)/);
  assert.match(exporter, /let Some\(snapshot\) = exportable_committed_turn_snapshot\(\)/);
  assert.match(exporter, /if fs::write\(&path, body\.as_bytes\(\)\)\.is_err\(\)/);
  assert.match(committed, /\*ended = None/);
});
test("manual exports label incoming audio without fabricating participant identity", () => {
  assert.match(exporter, /Meeting audio = combined incoming sound/);
  assert.match(exporter, /individual speakers are not identified/);
  assert.match(exporter, /"Meeting audio"/);
  assert.match(exporter, /"MEETING AUDIO"/);
});

test("manual transcript export retains original source after translation failure", () => {
  assert.match(exporter, /turn\.translation_state == "failed"/);
  assert.match(exporter, /"Translation unavailable"/);
  assert.match(committed, /snapshot\.has_session = false/);
  assert.match(committed, /turn\.translation_state == "pending"/);
});

test("long-session export is larger than the lightweight 240-entry transcript view", () => {
  const meeting = readFileSync(new URL("../../src-tauri/src/commands/meeting_session.rs", import.meta.url), "utf8");
  assert.match(committed, /const MAX_SESSION_COMMITTED_TURNS: usize = 4_096/);
  assert.match(committed, /const MAX_SESSION_TRANSCRIPT_BYTES: usize = 8 \* 1024 \* 1024/);
  assert.match(committed, /fn snapshot_for_export\(&self\)/);
  assert.match(committed, /\.take\(MAX_LIVE_COMMITTED_TURNS\)/);
  assert.match(committed, /let mut snapshot = store\.snapshot_for_export\(\)/);
  assert.match(meeting, /snapshot\.turns = snapshot\.turns\.split_off\(hidden\)/);
  assert.match(exporter, /exportable_committed_turn_snapshot\(\)/);
});

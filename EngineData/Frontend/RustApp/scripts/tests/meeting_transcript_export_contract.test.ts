import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const file = (path: string) => readFileSync(new URL("../../" + path, import.meta.url), "utf8");
const meeting = file("src-tauri/src/commands/meeting_session.rs");
const registry = file("src-tauri/src/commands/registry.rs");
const memory = file("src-tauri/src/commands/meeting_session/committed_turns.rs");
const lifecycle = file("src-tauri/src/commands/meeting_session/lifecycle.rs");
const archive = file("src-tauri/src/commands/meeting_session/transcript_archive.rs");
const html = file("src-tauri/src/commands/meeting_session/transcript_export.rs");
const ui = file("src/components/meeting/MeetingTranscriptExport.svelte");
const page = file("src/pages/Meeting.svelte");
const review = file("src/components/meeting/MeetingSessionReview.svelte");

test("persistent archive is session-bound and starts before live microphone capture", () => {
  assert.match(lifecycle, /transcript_archive::start_session\(&session_id\)/);
  const setup = lifecycle.indexOf("transcript_archive::start_session(&session_id)");
  const start = lifecycle.indexOf("let capture = start_live_capture_runtime(");
  assert.ok(setup > 0 && setup < start);
  assert.match(memory, /transcript_archive::queue_turn\(turn\)/);
  assert.match(memory, /transcript_archive::finish_session\(session_id\)/);
  assert.match(archive, /mpsc::sync_channel\(WRITE_QUEUE_SIZE\)/);
  assert.match(archive, /sender\.try_send\(Record::Turn\(turn\.clone\(\)\)\)/);
  assert.match(archive, /errors\.fetch_add\(1, Ordering::Relaxed\)/);
});

test("archive is retained across Stop, new meeting and export until explicit delete", () => {
  assert.match(archive, /join\("Transcripts"\)\.join\("Sessions"\)/);
  assert.match(archive, /fn atomic_turn_write/);
  assert.match(archive, /file\.sync_all\(\)/);
  assert.match(archive, /fs::rename\(&temporary, &destination\)/);
  assert.match(archive, /pub fn list_saved_sessions\(\)/);
  assert.match(archive, /pub fn delete_saved_session\(session_id: String\)/);
  assert.match(archive, /fs::remove_dir_all\(dir\)/);
  assert.match(archive, /Stop this Meeting before deleting/);
  assert.match(meeting, /list_saved_meeting_transcripts/);
  assert.match(registry, /delete_saved_meeting_transcript/);
});

test("HTML is offline, escaped, same-session and written as a complete file", () => {
  assert.match(html, /fn html_escape\(/);
  assert.match(html, /html_escape\(&turn\.source_text\)/);
  assert.match(html, /html_escape\(&turn\.translated_text\)/);
  assert.match(html, /stream_saved_turns\(&session_id/);
  assert.match(html, /File::create_new\(&temporary\)/);
  assert.match(html, /w\.get_ref\(\)\.sync_all\(\)/);
  assert.match(html, /fs::rename\(&temporary, &destination\)/);
  assert.match(html, /<style>\{STYLE\}<\/style>/);
  assert.match(html, /No scripts or external resources/);
  assert.doesNotMatch(html.split("#[cfg(test)]")[0], /<script|https:\/\/|@import|<link/i);
  assert.match(html, /#view-id:checked/);
  assert.match(html, /#view-en:checked/);
  assert.match(html, /Bilingual/);
  assert.match(html, /Bahasa Indonesia/);
  assert.match(html, /English/);
  assert.match(html, /@media print/);
  assert.match(html, /Translation unavailable/);
});

test("saved transcript controls expose one honest history and explicit deletion", () => {
  assert.match(page, /<MeetingTranscriptExport hasSession=\{meeting\.hasSession\}/);
  assert.doesNotMatch(review, /<MeetingTranscriptExport/);
  assert.match(ui, /Saved Transcripts/);
  assert.match(ui, /Export HTML/);
  assert.match(ui, /Delete saved transcript/);
  assert.match(ui, /Bilingual/);
  assert.match(ui, /Bahasa Indonesia/);
  assert.match(ui, /English/);
  assert.match(ui, /Storage finalization unverified/);
  assert.match(ui, /Open File/);
  assert.match(ui, /Open Folder/);
  assert.match(ui, /Copy Location/);
  assert.match(ui, /navigator\.clipboard\.writeText\(receipt\.path\)/);
  assert.doesNotMatch(ui, /onNotice\(receipt\.path/);
});

test("legacy live preview remains capped while export reads the disk-backed session", () => {
  assert.match(memory, /const MAX_LIVE_COMMITTED_TURNS: usize = 240/);
  assert.match(memory, /const MAX_SESSION_COMMITTED_TURNS: usize = 4_096/);
  assert.match(memory, /const MAX_SESSION_TRANSCRIPT_BYTES: usize = 8 \* 1024 \* 1024/);
  assert.match(memory, /\.take\(MAX_LIVE_COMMITTED_TURNS\)/);
  assert.match(archive, /MAX_SAVED_PREVIEW: usize = 240/);
  assert.match(html, /stream_saved_turns\(&session_id/);
  assert.doesNotMatch(html, /snapshot_for_export\(/);
});

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
  assert.match(archive, /writer\.sender\.try_send\(Record::Turn\(record\)\)/);
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

test("explicit Delete removes matching in-memory last-session review as well as archive", () => {
  assert.match(archive, /forget_ended_saved_session\(&session_id\)/);
  assert.match(memory, /pub\(super\) fn forget_ended_saved_session\(session_id: &str\)/);
  assert.match(memory, /snapshot\.session_id\.as_deref\(\)\) == Some\(session_id\)/);
  assert.match(page, /\{#key savedDeletionRevision\}/);
  assert.match(ui, /onDeleted\?\.\(\)/);
  assert.match(ui, /Existing exported HTML files will remain/);
});

test("HTML preview uses the real offline export renderer, not a second layout", () => {
  assert.match(html, /pub fn preview_saved_html\(session_id: String\)/);
  assert.match(html, /MAX_HTML_PREVIEW_ENTRIES: usize = 12/);
  assert.match(html, /write_header\(&mut buffer, &session_id, turns\.len\(\), true\)/);
  assert.match(html, /write_turn\(&mut buffer, turn\)/);
  assert.match(html, /write_footer\(&mut buffer, incomplete\)/);
  assert.match(ui, /runtimeApi\.previewSavedMeetingTranscriptHtml\(sessionId\)/);
  assert.match(ui, /Preview HTML/);
  assert.match(ui, /srcdoc=\{htmlSample\.html\}/);
  assert.match(ui, /sandbox=""/);
  assert.match(ui, /const revision = \+\+htmlPreviewRevision/);
  assert.match(ui, /if \(revision !== htmlPreviewRevision\) return/);
  assert.match(ui, /htmlPreviewRevision \+= 1/);
  assert.match(meeting, /preview_saved_meeting_transcript_html/);
  assert.match(registry, /preview_saved_meeting_transcript_html/);
});

test("archive Retry Stop keeps the same finish receipt and JoinHandle on timeout", () => {
  assert.match(archive, /finish_reply: Option<mpsc::Receiver<bool>>/);
  assert.match(archive, /finish_acknowledged: bool/);
  assert.match(archive, /match reply\.recv_timeout\(FINISH_WAIT\)/);
  assert.match(archive, /RecvTimeoutError::Timeout\) => return false/);
  assert.match(archive, /while writer\.thread\.as_ref\(\)\.is_some_and/);
  assert.match(archive, /guard\.take\(\)\.expect\("matching archive owner retained"\)/);
});

test("large transcript disk operations use Tauri blocking workers, not synchronous UI commands", () => {
  for (const command of [
    "list_saved_meeting_transcripts",
    "get_saved_meeting_transcript_preview",
    "delete_saved_meeting_transcript",
    "export_saved_meeting_transcript_html",
    "preview_saved_meeting_transcript_html",
    "open_saved_meeting_html",
  ]) {
    assert.match(meeting, new RegExp(`pub async fn ${command}\\(`));
  }
  assert.match(meeting, /tauri::async_runtime::spawn_blocking\(transcript_archive::list_saved_sessions\)/);
  assert.match(meeting, /spawn_blocking\(move \|\| transcript_export::export_saved_html\(session_id\)\)/);
  assert.match(meeting, /spawn_blocking\(move \|\| transcript_archive::delete_saved_session\(session_id\)\)/);
  assert.match(meeting, /HTML export was interrupted/);
});

test("zero-entry saved sessions remain visible for deliberate deletion without empty HTML export", () => {
  assert.match(archive, /Retain empty archives in History as well/);
  assert.match(archive, /match stored_turn_paths\(&entry\.path\(\)\) \{\s*\/\//);
  assert.match(archive, /Ok\(turns\) => \{/);
  assert.doesNotMatch(archive, /Ok\(turns\) if !turns\.is_empty\(\)/);
  assert.match(ui, /No recognized entries/);
  assert.match(ui, /session\.entry_count === 0/);
  assert.match(ui, /onclick=\{\(\) => \{ confirming = session\.session_id/);
});

test("accepted turn survives immediate bounded-memory eviction before disk queue", () => {
  // The canonical owner returns an accepted record independently of whether
  // the bounded Live VecDeque still contains that entry.
  assert.match(memory, /\) -> Option<MeetingCommittedTurn> \{/);
  assert.match(memory, /let archived = turn\.clone\(\);/);
  assert.match(memory, /self\.enforce_session_limits\(\);\s*Some\(archived\)/);
  assert.match(memory, /let accepted = committed\.is_some\(\);/);
  assert.match(memory, /if let Some\(turn\) = committed \{\s*transcript_archive::queue_turn\(turn\);/);
  assert.match(memory, /let accepted = updated\.is_some\(\);/);
  assert.match(memory, /if let Some\(turn\) = updated \{\s*transcript_archive::queue_turn\(turn\);/);
  assert.match(memory, /fn archive_receives_new_source_even_when_memory_evicts_it_immediately\(/);
  assert.match(memory, /fn archive_receives_completed_translation_even_if_updated_turn_is_evicted\(/);
  assert.doesNotMatch(memory, /if let Some\(turn\) = store\.turns\.iter\(\)\.find\(\|turn\| turn\.sequence == sequence\)/);
  assert.match(archive, /pub\(super\) fn queue_turn\(turn: MeetingCommittedTurn\)/);
  assert.match(archive, /Record::Turn\(record\)/);
});

test("archive queue has bounded record count and estimated UTF-8 payload size", () => {
  assert.match(archive, /MAX_QUEUED_ESTIMATED_BYTES: usize = 128 \* 1024 \* 1024/);
  assert.match(archive, /MAX_QUEUED_ESTIMATED_BYTES/);
  assert.match(archive, /fn reserve_queued_bytes\(counter: &AtomicUsize, reservation: usize\)/);
  assert.match(archive, /counter\.fetch_update\(/);
  assert.match(archive, /current\.checked_add\(reservation\)/);
  assert.match(archive, /writer\.queued_bytes\.fetch_sub\(reservation, Ordering::AcqRel\)/);
  assert.match(archive, /queued_bytes\.fetch_sub\(record\.reserved_bytes, Ordering::AcqRel\)/);
  assert.match(archive, /writer\.errors\.fetch_add\(1, Ordering::Relaxed\)/);
  assert.match(archive, /fn queued_record_byte_budget_is_bounded_and_released\(/);
  assert.match(archive, /serde_json::to_writer\(&mut buffered, turn\)/);
  assert.match(archive, /buffered\.flush\(\)/);
  assert.doesNotMatch(archive, /serde_json::to_vec\(turn\)/);
  assert.match(archive, /mpsc::sync_channel\(WRITE_QUEUE_SIZE\)/);
});

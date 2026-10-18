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
  assert.match(archive, /errors\.fetch_add\(1, Ordering::AcqRel\)/);
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
  assert.match(html, /write_html_escaped\(w, &turn\.source_text\)/);
  assert.match(html, /write_html_escaped\(w, &turn\.translated_text\)/);
  assert.match(html, /fn write_html_escaped<W: Write>\(w: &mut W, text: &str\)/);
  assert.doesNotMatch(html, /html_escape\(&turn\.(?:source_text|translated_text)\)/);
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
  assert.match(html, /write_header\(&mut buffer, &session_id, turns\.len\(\), true, incomplete\)/);
  assert.match(html, /write_turn\(&mut buffer, turn\)/);
  assert.match(html, /write_footer\(&mut buffer\)/);
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

test("no-session Stop preserves transcript until the orphan writer is released", () => {
  assert.match(archive, /fn runtime_confirmed_idle\(/);
  assert.match(archive, /fn finish_writer_without_session\(/);
  assert.match(archive, /fn inactive_writer_may_need_stop\(/);
  assert.match(archive, /fn orphan_writer_recovery_requires_verified_idle_runtime\(/);
  const stopBody = lifecycle.slice(
    lifecycle.indexOf("let Some(snapshot) = current.snapshot.as_ref() else {"),
    lifecycle.indexOf("let generation = snapshot.generation;"),
  );
  assert.match(stopBody, /finish_writer_without_session\(\)/);
  assert.match(stopBody, /clear_transcript_only_after_retention\(/);
  assert.ok(stopBody.indexOf("finish_writer_without_session()") <
    stopBody.indexOf("clear_transcript_only_after_retention("));
  assert.doesNotMatch(stopBody, /let transcript_cleanup_ok = clear_inactive_committed_turns\(\)/);
  assert.match(lifecycle, /inactive_writer_may_need_stop\(\)/);
});

test("saved transcript Delete cannot bypass an unknown or active archive writer", () => {
  const deleteBody = archive.slice(
    archive.indexOf("pub fn delete_saved_session("),
    archive.indexOf("#[cfg(test)]", archive.indexOf("pub fn delete_saved_session(")),
  );
  assert.match(deleteBody, /saved_session_operation_guard\(\)/);
  assert.match(deleteBody, /ensure_saved_session_writer_finished\(&session_id\)/);
  assert.doesNotMatch(deleteBody, /writer_store\(\)\.lock\(\)\.ok\(\)/);
  assert.ok(deleteBody.indexOf("ensure_saved_session_writer_finished(&session_id)") <
    deleteBody.indexOf("fs::remove_dir_all(dir)"),
    "writer must be verifiably finished before archive removal");
  const writerGate = archive.slice(
    archive.indexOf("pub(super) fn ensure_saved_session_writer_finished("),
    archive.indexOf("pub(super) fn finish_session("),
  );
  assert.match(writerGate, /let guard = writer_store\(\)\.lock\(\)/);
  assert.match(writerGate, /\.map_err\(\|_\|/);
  assert.match(writerGate, /writer\.session_id == session_id/);
});

test("saved listing and preview fail closed when writer health cannot be read", () => {
  const errorProbe = archive.slice(
    archive.indexOf("pub(super) fn active_write_errors("),
    archive.indexOf("pub(super) fn ensure_saved_session_writer_finished("),
  );
  assert.match(errorProbe, /-> Result<u64, String>/);
  assert.match(errorProbe, /let guard = writer_store\(\)\.lock\(\)/);
  assert.match(errorProbe, /\.map_err\(\|_\|/);
  assert.doesNotMatch(errorProbe, /\.lock\(\)\.ok\(\)/);
  const listing = archive.slice(
    archive.indexOf("pub fn list_saved_sessions()"),
    archive.indexOf("pub(super) struct SavedExportInventory"),
  );
  assert.match(listing, /let active = match writer_store\(\)\.lock\(\)/);
  assert.match(listing, /Err\(_\) => return SavedMeetingSessions/);
  assert.match(listing, /ok: false,/);
  assert.doesNotMatch(listing, /writer_store\(\)\.lock\(\)\.ok\(\)/);
  // Missing archive root is the sole benign empty state. Access failures are
  // surfaced as errors, not a fabricated empty saved transcript history.
  assert.match(listing, /let dir_entries = match fs::read_dir\(&root\)/);
  assert.match(listing, /Err\(error\) if error\.kind\(\) == io::ErrorKind::NotFound => return result/);
  assert.match(listing, /Err\(_\) => return SavedMeetingSessions \{/);
  assert.doesNotMatch(listing, /if !root\.exists\(\)/);
  assert.match(archive, /active_write_errors\(id\)\? > 0/);
  assert.match(html, /active_write_errors\(&session_id\)\.map_or\(true, \|errors\| errors > 0\)/);
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
  assert.match(archive, /match scan_stored_turns\(&entry\.path\(\)\) \{\s*\/\//);
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
  // Only initial recognition/translation may bypass the bounded buffer:
  // their owned accepted records reach the queue before eviction.
  const sourceAndTranslation = memory.slice(
    memory.indexOf("pub(super) fn commit_meeting_turn("),
    memory.indexOf("// Sequence + source identity and queued state"),
  );
  assert.doesNotMatch(sourceAndTranslation, /store\.turns\.iter\(\)\.find\(\|turn\| turn\.sequence == sequence\)/);
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
  assert.match(archive, /writer\.errors\.fetch_add\(1, Ordering::AcqRel\)/);
  assert.match(archive, /fn queued_record_byte_budget_is_bounded_and_released\(/);
  assert.match(archive, /serde_json::to_writer\(&mut buffered, turn\)/);
  assert.match(archive, /buffered\.flush\(\)/);
  assert.doesNotMatch(archive, /serde_json::to_vec\(turn\)/);
  assert.match(archive, /mpsc::sync_channel\(WRITE_QUEUE_SIZE\)/);
});

test("finished receipt counts surviving archived turns and rejects crash staging files", () => {
  assert.match(archive, /const FINISHED_RECEIPT_HEADER: &str = "TranslateIT Meeting archive receipt v1"/);
  assert.match(archive, /fn write_finished_receipt\(dir: &Path\)/);
  assert.match(archive, /fn finished_receipt_count\(dir: &Path\)/);
  assert.match(archive, /write_finished_receipt\(&dir\)\.is_ok\(\)/);
  assert.match(archive, /expected == turns\.len\(\)/);
  assert.match(archive, /expected == scan\.paths\.len\(\)/);
  assert.match(archive, /scan\.has_unpublished_files/);
  assert.match(archive, /name\.ends_with\("\.tmp"\)/);
  assert.match(archive, /fn finished_receipt_rejects_missing_and_legacy_metadata\(/);
  assert.match(archive, /write_finished_receipt\(&dir\)\.is_err\(\)/);
});

test("new Meeting start cannot reopen a previous archive folder with only an interrupted temp file", () => {
  assert.match(archive, /fs::create_dir_all\(&root\)/);
  assert.match(archive, /fs::create_dir\(&dir\)/);
  assert.match(archive, /io::ErrorKind::AlreadyExists/);
  assert.doesNotMatch(archive.split("#[cfg(test)]")[0], /fs::create_dir_all\(&dir\)/);
});

test("saved JSON entry identity and content validation is shared by Preview and HTML export", () => {
  assert.match(archive, /fn read_validated_turn\(path: &Path, session_id: &str\)/);
  assert.match(archive, /serde_json::from_reader\(BufReader::new\(file\)\)/);
  assert.match(archive, /file_sequence != Some\(turn\.sequence\)/);
  assert.match(archive, /turn\.session_id != session_id/);
  assert.match(archive, /turn\.source_language == turn\.target_language/);
  assert.match(archive, /turn\.source_text\.trim\(\)\.is_empty\(\)/);
  assert.equal((archive.match(/read_validated_turn\(&path, id\)/g) ?? []).length, 2);
  assert.match(archive, /fn invalid_saved_entry_never_silently_enters_preview_or_html_export\(/);
  assert.match(archive, /truncated or damaged JSON must fail closed/);
  assert.match(html, /stream_saved_turns\(&session_id, &inventory\.paths, \|turn\|/);
  assert.match(html, /fs::remove_file\(&temporary\)/);
  assert.match(html, /fs::rename\(&temporary, &destination\)/);
});

test("single-pass HTML export and Delete share a disk-only operation guard", () => {
  assert.match(archive, /static SAVED_SESSION_OPERATIONS: OnceLock<Mutex<\(\)>>/);
  assert.match(archive, /fn saved_session_operation_guard\(\)/);
  assert.match(archive, /pub fn delete_saved_session[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(html, /pub fn export_saved_html[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(html, /saved_export_inventory\(&session_id\)/);
  assert.equal((html.match(/stream_saved_turns\(&session_id/g) ?? []).length, 1);
  assert.doesNotMatch(html, /stream_saved_turns\(&session_id, \|_\|/);
});

test("missing translation is never mislabeled as translated target-language text", () => {
  assert.match(html, /ORIGINAL FALLBACK · \{source_lang\}/);
  assert.match(html, /lang=\\"\{source_code\}\\"/);
  assert.match(html, /\.original-fallback \{ display:none;/);
  assert.match(html, /body:has\(#view-id:checked\) \.original-fallback/);
  assert.match(html, /body:has\(#view-en:checked\) \.original-fallback/);
  assert.match(html, /fn language_mode_never_labels_untranslated_source_as_target_language\(/);
});

test("incomplete transcript warning is shown before the first conversation entry", () => {
  assert.match(html, /write_header\(&mut w, &session_id, count, false, incomplete\)/);
  assert.match(html, /write_header\(&mut buffer, &session_id, turns\.len\(\), true, incomplete\)/);
  assert.match(html, /Transcript completeness unverified\./);
  assert.match(html, /fn incomplete_storage_warning_appears_before_conversation_entries\(/);
  assert.match(html, /Reading view:/);
  assert.match(html, /body:has\(#view-id:checked\) \.print-mode-id/);
  assert.match(html, /body:has\(#view-en:checked\) \.print-mode-en/);
  assert.match(html, /@media print/);
});

test("Live transcript surfaces the first actual storage failure with existing Meeting events", () => {
  const live = file("src/components/meeting/MeetingActivity.svelte");
  const reader = file("src/app/runtime/meetingReconcileReader.ts");
  const eventMonitors = file("src/app/runtime/reliabilityMonitor.ts");
  assert.match(meeting, /pub archive_write_errors: u64/);
  assert.match(memory, /archive_write_errors: transcript_archive::active_write_errors\(&self\.session_id\)/);
  assert.match(archive, /let first_error = errors\.fetch_add\(1, Ordering::AcqRel\) == 0/);
  assert.match(archive, /let first_error = writer\.errors\.fetch_add\(1, Ordering::AcqRel\) == 0/);
  assert.match(archive, /emit_meeting_runtime_event\("transcript_storage_error"/);
  assert.match(live, /turns\.archive_write_errors/);
  assert.match(live, /role="alert"/);
  assert.match(live, /saved archive/);
  assert.match(eventMonitors, /void pollMeeting\(true\)/);
  assert.match(reader, /if \(statusKey === previousStatusKey && !forceTurns\)/);
});

test("saved transcript previews share the existing Delete-Export archive lock", () => {
  assert.match(archive, /static SAVED_SESSION_OPERATIONS: OnceLock<Mutex<\(\)>>/);
  assert.match(archive, /pub fn saved_session_preview\(session_id: String\)[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(html, /pub fn preview_saved_html\(session_id: String\)[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(html, /pub fn export_saved_html\(session_id: String\)[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(archive, /pub fn delete_saved_session\(session_id: String\)[\s\S]*?saved_session_operation_guard\(\)/);
  assert.match(archive, /transcript_archive:read_lock_failed/);
  assert.doesNotMatch(memory, /saved_session_operation_guard\(/);
});

test("Live HTML sample distinguishes ongoing translation from failure or interrupted archive", () => {
  assert.match(html, /let \(message, tone\) = if turn\.translation_state == "pending"/);
  assert.match(html, /\("Translation in progress\.", "pending"\)/);
  assert.match(html, /\("Translation unavailable\.", "unavailable"\)/);
  assert.match(html, /\.pending \{ color:var\(--muted\); \}/);
  assert.match(html, /fn pending_translation_is_not_reported_as_failed_while_meeting_is_active\(/);
  assert.match(html, /let active = latest_runtime_session_state\(\)\.snapshot\.as_ref\(\)\.is_some_and/);
  assert.match(html, /let incomplete = if active \{\s*active_write_errors\(&session_id\) > 0/);
  assert.match(html, /Meeting in progress\./);
  assert.match(html, /New recognized entries and pending translations may still change/);
  assert.match(archive, /Resolve it once rather than taking the runtime state lock per entry/);
  assert.match(archive, /if !active && turn\.translation_state == "pending"/);
});

test("committed delivery, playback timing and Stop interruption reach the same disk writer", () => {
  const claim = memory.slice(memory.indexOf("pub(super) fn begin_committed_playback("),
    memory.indexOf("pub(super) fn update_committed_turn_delivery_state("));
  const delivery = memory.slice(memory.indexOf("pub(super) fn update_committed_turn_delivery_state("),
    memory.indexOf("pub(super) fn update_committed_turn_outbound_timing("));
  const timing = memory.slice(memory.indexOf("pub(super) fn update_committed_turn_outbound_timing("),
    memory.indexOf("pub(super) fn interrupt_committed_turns_for_generation("));
  const stop = memory.slice(memory.indexOf("pub(super) fn interrupt_committed_turns_for_generation("),
    memory.indexOf("fn empty_committed_turn_snapshot("));
  for (const section of [claim, delivery, timing, stop]) {
    assert.match(section, /transcript_archive::queue_turn\(/);
  }
  assert.match(memory, /fn interrupt_generation_at\([\s\S]*?\) -> Vec<MeetingCommittedTurn>/);
  assert.match(memory, /changed\.push\(turn\.clone\(\)\)/);
  assert.match(stop, /for turn in store\.interrupt_generation_at\(generation, unix_ms\(\)\)/);
  assert.match(memory, /fn stop_returns_only_newly_interrupted_turns_for_disk_update\(/);
  assert.match(memory, /fn final_playback_state_and_timing_remain_attached_to_one_turn\(/);
  assert.match(archive, /Record::Turn\(record\)/);
});

test("Stop persists unfinished incoming translations without fabricating playback", () => {
  const stopMutation = memory.slice(
    memory.indexOf("fn interrupt_generation_at("),
    memory.indexOf("fn outbound_latency_distribution("),
  );
  assert.match(stopMutation, /let unfinished_incoming = turn\.lane == "incoming"/);
  assert.match(stopMutation, /&& turn\.translation_state == "pending"/);
  assert.match(stopMutation, /if interrupted_outbound \{\s*turn\.delivery_state = Some\("interrupted"/);
  assert.match(stopMutation, /if turn\.translation_state == "pending" \{\s*turn\.translation_state = "failed"/);
  assert.match(stopMutation, /changed\.push\(turn\.clone\(\)\)/);
  assert.match(memory, /fn stop_finalizes_pending_incoming_without_rewriting_translated_incoming\(/);
  assert.match(memory, /incoming captions must never invent outbound playback state/);
  assert.match(memory, /Retry Stop must not produce a second pending-incoming update/);
  assert.match(memory, /for turn in store\.interrupt_generation_at\(generation, unix_ms\(\)\)/);
  assert.match(archive, /Record::Finish\(reply\)/);
});

test("HTML transcript escaping streams large UTF-8 text without cloned escaped strings", () => {
  assert.match(html, /for \(index, c\) in text\.char_indices\(\)/);
  assert.match(html, /w\.write_all\(text\[start\.\.index\]\.as_bytes\(\)\)/);
  assert.match(html, /w\.write_all\(replacement\.as_bytes\(\)\)/);
  assert.match(html, /w\.write_all\(text\[start\.\.\]\.as_bytes\(\)\)/);
  assert.match(html, /write_html_escaped\(w, &turn\.source_text\)/);
  assert.match(html, /write_html_escaped\(w, &turn\.translated_text\)/);
  assert.match(html, /fn streaming_html_escape_preserves_utf8_and_rejects_markup_injection\(/);
  assert.match(html, /output\.windows\(9\)/);
  assert.doesNotMatch(html, /html_escape\(&turn\.source_text\)|html_escape\(&turn\.translated_text\)/);
});

test("full HTML export snapshots one saved-session file inventory and decodes once", () => {
  assert.match(archive, /pub\(super\) struct SavedExportInventory/);
  assert.match(archive, /pub\(super\) fn saved_export_inventory\(id: &str\)/);
  assert.match(archive, /let scan = scan_stored_turns\(&dir\)/);
  assert.match(archive, /SavedExportInventory \{ paths: scan\.paths, incomplete \}/);
  assert.match(archive, /pub\(super\) fn stream_saved_turns\(\s*id: &str,\s*paths: &\[PathBuf\]/);
  assert.match(html, /saved_export_inventory\(&session_id\)/);
  assert.match(html, /let incomplete = inventory\.incomplete/);
  assert.match(html, /let count = inventory\.paths\.len\(\)/);
  assert.match(html, /stream_saved_turns\(&session_id, &inventory\.paths, \|turn\|/);
  assert.doesNotMatch(html, /saved_session_entry_count\(&session_id\)/);
  assert.equal((html.match(/stream_saved_turns\(&session_id/g) ?? []).length, 1);
});

test("desktop preview has an encoded-byte limit independently of full HTML export", () => {
  assert.match(archive, /const MAX_SAVED_READ_BYTES: u64 = 4 \* 1024 \* 1024/);
  assert.match(archive, /fn admitted_preview_bytes\(current: u64, next: u64, limit: u64\)/);
  assert.match(archive, /fs::metadata\(&path\)/);
  assert.match(archive, /admitted_preview_bytes\(encoded_bytes, file_bytes, max_encoded_bytes\)/);
  assert.match(archive, /fn preview_encoded_byte_budget_rejects_oversized_and_overflow_entries\(/);
  assert.match(html, /MAX_HTML_PREVIEW_ENCODED_BYTES: u64 = 512 \* 1024/);
  assert.match(html, /MAX_HTML_PREVIEW_ENTRIES, MAX_HTML_PREVIEW_ENCODED_BYTES/);
  assert.match(ui, /if \(!result\.ok\)/);
  assert.match(html, /stream_saved_turns\(&session_id, &inventory\.paths/);
  assert.doesNotMatch(html.slice(html.indexOf("pub fn export_saved_html")), /MAX_HTML_PREVIEW_ENCODED_BYTES/);
});

test("stable HTML export rejects a still-owned archive writer after Stop starts", () => {
  assert.match(archive, /pub\(super\) fn ensure_saved_session_writer_finished\(session_id: &str\)/);
  assert.match(archive, /let guard = writer_store\(\)\.lock\(\)/);
  assert.match(archive, /writer\.session_id == session_id/);
  assert.match(archive, /Transcript storage is still finishing\. Retry Stop before exporting HTML/);
  const exportBody = html.slice(html.indexOf("pub fn export_saved_html("),
    html.indexOf("pub fn open_saved_html("));
  assert.match(exportBody, /saved_session_operation_guard\(\)/);
  assert.match(exportBody, /ensure_saved_session_writer_finished\(&session_id\)/);
  assert.ok(exportBody.indexOf("ensure_saved_session_writer_finished(&session_id)") <
    exportBody.indexOf("saved_export_inventory(&session_id)"),
    "writer must be confirmed stopped before acquiring its export inventory");
  const previewBody = html.slice(html.indexOf("pub fn preview_saved_html("),
    html.indexOf("pub fn export_saved_html("));
  assert.doesNotMatch(previewBody, /ensure_saved_session_writer_finished/);
});

test("explicit HTML export cleans only app-owned interrupted temporary output", () => {
  assert.match(html, /fn is_owned_partial_html\(name: &str\)/);
  assert.match(html, /fn clean_abandoned_html_partials\(root: &Path\)/);
  assert.match(html, /strip_prefix\("Transcript_"\)/);
  assert.match(html, /strip_suffix\("\.html\.partial"\)/);
  assert.match(html, /entry\.file_type\(\)\.is_ok_and\(\|kind\| kind\.is_file\(\)\)/);
  assert.match(html, /fs::remove_file\(entry\.path\(\)\)/);
  assert.match(html, /clean_abandoned_html_partials\(&root\)/);
  assert.match(html, /fn interrupted_html_cleanup_only_removes_generated_partial_files\(/);
  assert.match(html, /Cleaned \{removed_partials\} abandoned unfinished HTML export file/);
  assert.match(html, /fs::remove_file\(&temporary\)/);
  assert.match(html, /fs::rename\(&temporary, &destination\)/);
  assert.match(html, /An unfinished HTML file may remain in the export folder/);
  assert.match(html, /The saved transcript source was preserved/);
  assert.doesNotMatch(html, /remove_dir_all\(/);
});

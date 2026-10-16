// Persistent, local-only Meeting transcript archive. Transcript authority remains
// in committed_turns.rs. This owner only writes immutable session identity and
// replaceable snapshots of individual authorized turns off the audio/AI threads.
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use crate::engine::runtime_events::emit_meeting_runtime_event;

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};
use super::{MeetingCommittedTurn, MeetingCommittedTurnsSnapshot};

const WRITE_QUEUE_SIZE: usize = 256;
// A record-count cap alone is insufficient when a single source/translation
// is large. Account for UTF-8 payload plus a small fixed metadata allowance.
// This is a producer-memory budget, NOT a disk quota or HTML size guarantee.
const MAX_QUEUED_ESTIMATED_BYTES: usize = 128 * 1024 * 1024;
const PER_QUEUED_RECORD_OVERHEAD: usize = 4096;
const FINISH_WAIT: Duration = Duration::from_secs(5);
const MAX_SAVED_PREVIEW: usize = 240;

#[derive(Debug, Clone, Serialize)]
pub struct SavedMeetingSession {
    pub session_id: String,
    pub entry_count: usize,
    pub complete: bool,
    pub storage_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedMeetingSessions {
    pub ok: bool,
    pub sessions: Vec<SavedMeetingSession>,
    pub message: String,
    pub active_write_errors: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveActionResult {
    pub ok: bool,
    pub message: String,
}

struct QueuedTurn {
    turn: MeetingCommittedTurn,
    reserved_bytes: usize,
}

enum Record {
    Turn(QueuedTurn),
    Finish(mpsc::Sender<bool>),
}

struct ArchiveWriter {
    session_id: String,
    sender: SyncSender<Record>,
    thread: Option<JoinHandle<()>>,
    errors: Arc<AtomicU64>,
    queued_bytes: Arc<AtomicUsize>,
    finish_reply: Option<mpsc::Receiver<bool>>,
    finish_acknowledged: bool,
}

static WRITER: OnceLock<Mutex<Option<ArchiveWriter>>> = OnceLock::new();
// Serialize explicit saved-session deletion and HTML publication. The lock
// never participates in the live audio/translation writer pipeline.
static SAVED_SESSION_OPERATIONS: OnceLock<Mutex<()>> = OnceLock::new();

fn writer_store() -> &'static Mutex<Option<ArchiveWriter>> {
    WRITER.get_or_init(|| Mutex::new(None))
}

pub(super) fn saved_session_operation_guard() -> Result<MutexGuard<'static, ()>, String> {
    SAVED_SESSION_OPERATIONS
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Saved transcript operations are unavailable; retry after restarting.".to_string())
}

fn valid_id(id: &str) -> bool {
    id.starts_with("meeting_") && id.len() <= 96
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

fn archive_root() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().user_saved_dir)
        .join("Transcripts").join("Sessions")
}

fn session_dir(id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("Invalid Meeting session identity.".to_string());
    }
    Ok(archive_root().join(id))
}

fn turn_name(sequence: u64) -> String {
    format!("turn_{sequence:020}.json")
}

struct StoredTurnScan {
    paths: Vec<PathBuf>,
    has_unpublished_files: bool,
}

fn scan_stored_turns(dir: &Path) -> io::Result<StoredTurnScan> {
    let mut scan = StoredTurnScan { paths: Vec::new(), has_unpublished_files: false };
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            scan.has_unpublished_files = true;
            continue;
        };
        if name.starts_with("turn_") && name.ends_with(".json")
            && name[5..name.len()-5].len() == 20
            && name[5..name.len()-5].bytes().all(|b| b.is_ascii_digit())
        {
            if entry.file_type()?.is_file() {
                scan.paths.push(entry.path());
            } else {
                // Refuse symlinks or directories as archive entries, while
                // keeping the session visible so the user can delete it.
                scan.has_unpublished_files = true;
            }
        } else if name.starts_with("turn_") || name.ends_with(".tmp") {
            // A crash can leave a staged write without its final rename.
            // Never silently treat that session as a fully finalized archive.
            scan.has_unpublished_files = true;
        }
    }
    scan.paths.sort();
    Ok(scan)
}

fn stored_turn_paths(dir: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(scan_stored_turns(dir)?.paths)
}

fn atomic_turn_write(dir: &Path, turn: &MeetingCommittedTurn) -> io::Result<()> {
    // The writer is the only updater for this session. An interrupted .tmp is
    // never mistaken for an archived turn by listing/export.
    let destination = dir.join(turn_name(turn.sequence));
    let temporary = dir.join(format!("turn_{:020}.tmp", turn.sequence));
    let mut file = File::create(&temporary)?;
    // Stream JSON into the temporary file instead of materializing an
    // additional encoded Vec, which can be many times the original text
    // size for long or heavily escaped utterances.
    {
        let mut buffered = BufWriter::new(&mut file);
        serde_json::to_writer(&mut buffered, turn).map_err(io::Error::other)?;
        buffered.flush()?;
    }
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &destination)?;
    Ok(())
}

const FINISHED_RECEIPT_HEADER: &str = "TranslateIT Meeting archive receipt v1";

fn write_atomic_marker(dir: &Path, marker: &str, contents: &[u8]) -> io::Result<()> {
    // A final marker is visible only after writing and syncing its temporary
    // counterpart. A crash before rename leaves an untrusted *.tmp.
    let temporary = dir.join(format!("{marker}.tmp"));
    let final_path = dir.join(marker);
    let mut file = File::create(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, final_path)
}

fn write_marker(dir: &Path, marker: &str) -> io::Result<()> {
    write_atomic_marker(dir, marker, b"TranslateIT local Meeting transcript archive\n")
}

fn write_finished_receipt(dir: &Path) -> io::Result<()> {
    // This is a finalization/count receipt, NOT evidence that every spoken
    // utterance was recognized or that all data is forensically durable.
    let scan = scan_stored_turns(dir)?;
    if scan.has_unpublished_files {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unpublished archive entries remain"));
    }
    let body = format!("{FINISHED_RECEIPT_HEADER}\nentries={}\n", scan.paths.len());
    write_atomic_marker(dir, "finished", body.as_bytes())
}

fn finished_receipt_count(dir: &Path) -> io::Result<Option<usize>> {
    let body = match fs::read_to_string(dir.join("finished")) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut lines = body.lines();
    if lines.next() != Some(FINISHED_RECEIPT_HEADER) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown transcript receipt version"));
    }
    let count = lines.next()
        .and_then(|line| line.strip_prefix("entries="))
        .and_then(|value| value.parse::<usize>().ok());
    if lines.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected receipt data"));
    }
    count.map(Some).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid receipt count"))
}

fn run_writer(
    dir: PathBuf,
    receiver: mpsc::Receiver<Record>,
    errors: Arc<AtomicU64>,
    queued_bytes: Arc<AtomicUsize>,
) {
    for record in receiver {
        match record {
            Record::Turn(record) => {
                let result = atomic_turn_write(&dir, &record.turn);
                queued_bytes.fetch_sub(record.reserved_bytes, Ordering::AcqRel);
                if result.is_err() {
                    let first_error = errors.fetch_add(1, Ordering::AcqRel) == 0;
                    let _ = write_marker(&dir, "storage_warning");
                    if first_error {
                        emit_meeting_runtime_event(
                            "transcript_storage_error",
                            Some(&record.turn.session_id),
                            Some(record.turn.sequence),
                        );
                    }
                }
            }
            Record::Finish(reply) => {
                let clean = errors.load(Ordering::Acquire) == 0;
                let marked = if clean {
                    write_finished_receipt(&dir).is_ok()
                } else {
                    write_marker(&dir, "storage_warning").is_ok()
                };
                if !marked { let _ = write_marker(&dir, "storage_warning"); }
                let _ = reply.send(marked);
                break;
            }
        }
    }
}

pub(super) fn start_session(session_id: &str) -> Result<(), String> {
    let dir = session_dir(session_id)?;
    let mut guard = writer_store().lock().map_err(|_| "Transcript archive owner unavailable.".to_string())?;
    if guard.is_some() {
        return Err("Previous transcript storage is not finished. Retry Stop before starting another Meeting.".to_string());
    }
    // Never reopen any pre-existing session directory, even if a crash left
    // ONLY a staged .tmp file and no committed JSON entry or finish marker.
    // create_dir itself is atomic against a colliding process/session owner.
    let root = archive_root();
    fs::create_dir_all(&root)
        .map_err(|_| "Could not create the local transcript archive root.".to_string())?;
    fs::create_dir(&dir)
        .map_err(|error| if error.kind() == io::ErrorKind::AlreadyExists {
            "Meeting transcript archive identity already exists.".to_string()
        } else {
            "Could not create the local transcript archive folder.".to_string()
        })?;
    let (sender, receiver) = mpsc::sync_channel(WRITE_QUEUE_SIZE);
    let errors = Arc::new(AtomicU64::new(0));
    let queued_bytes = Arc::new(AtomicUsize::new(0));
    let worker_errors = Arc::clone(&errors);
    let worker_bytes = Arc::clone(&queued_bytes);
    let handle = thread::Builder::new().name("translateit-transcript-storage".to_string())
        .spawn(move || run_writer(dir, receiver, worker_errors, worker_bytes))
        .map_err(|_| "Could not start local transcript storage.".to_string())?;
    *guard = Some(ArchiveWriter {
        session_id: session_id.to_string(), sender, thread: Some(handle), errors,
        queued_bytes, finish_reply: None, finish_acknowledged: false,
    });
    Ok(())
}

fn reserve_queued_bytes(counter: &AtomicUsize, reservation: usize) -> bool {
    reservation <= MAX_QUEUED_ESTIMATED_BYTES
        && counter.fetch_update(
            Ordering::AcqRel, Ordering::Relaxed,
            |current| current.checked_add(reservation)
                .filter(|total| *total <= MAX_QUEUED_ESTIMATED_BYTES),
        ).is_ok()
}

pub(super) fn queue_turn(turn: MeetingCommittedTurn) {
    let Ok(guard) = writer_store().lock() else { return };
    let Some(writer) = guard.as_ref() else { return };
    if writer.session_id != turn.session_id { return }

    // Bounded reservation is taken before moving the text into the queue.
    // The writer releases the reservation only after the record's disk
    // write is finished. A full queue or byte budget is an explicit loss
    // counted toward this session's storage_warning at finalization.
    let reservation = turn.source_text.len()
        .saturating_add(turn.translated_text.len())
        .saturating_add(PER_QUEUED_RECORD_OVERHEAD);
    if !reserve_queued_bytes(&writer.queued_bytes, reservation) {
        let first_error = writer.errors.fetch_add(1, Ordering::AcqRel) == 0;
        let session_id = if first_error { Some(writer.session_id.clone()) } else { None };
        drop(guard);
        if let Some(id) = session_id {
            emit_meeting_runtime_event("transcript_storage_error", Some(&id), None);
        }
        return;
    }
    // Reuse the owned source accepted before Live buffer eviction.
    let record = QueuedTurn { turn, reserved_bytes: reservation };
    match writer.sender.try_send(Record::Turn(record)) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            writer.queued_bytes.fetch_sub(reservation, Ordering::AcqRel);
            let first_error = writer.errors.fetch_add(1, Ordering::AcqRel) == 0;
            let session_id = if first_error { Some(writer.session_id.clone()) } else { None };
            drop(guard);
            if let Some(id) = session_id {
                emit_meeting_runtime_event("transcript_storage_error", Some(&id), None);
            }
        }
    }
}

pub(super) fn active_write_errors(session_id: &str) -> u64 {
    writer_store().lock().ok().and_then(|g| g.as_ref()
        .filter(|w| w.session_id == session_id)
        .map(|w| w.errors.load(Ordering::Acquire))).unwrap_or(0)
}

pub(super) fn finish_session(session_id: &str) -> bool {
    let Ok(mut guard) = writer_store().lock() else { return false };
    let Some(writer) = guard.as_mut() else { return true };
    if writer.session_id != session_id { return false }

    // A Stop timeout does not mean Finish was never accepted. Keep the same
    // receiver and JoinHandle so Retry Stop cannot enqueue a second Finish
    // behind the now-disconnected writer and get stuck indefinitely.
    if writer.finish_reply.is_none() && !writer.finish_acknowledged {
        let (tx, rx) = mpsc::channel();
        if writer.sender.try_send(Record::Finish(tx)).is_err() {
            return false;
        }
        writer.finish_reply = Some(rx);
    }
    if !writer.finish_acknowledged {
        let reply = writer.finish_reply.as_ref().expect("finish receiver retained");
        match reply.recv_timeout(FINISH_WAIT) {
            Ok(_marker_written) => {
                // A marker-write failure is reflected by the lack of a valid
                // finished marker, never confused with session cleanup success.
                writer.finish_acknowledged = true;
                writer.finish_reply = None;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // An unexpectedly exited worker cannot be joined blindly.
                // Keep its owner unless its actual thread exit is confirmed.
                if writer.thread.as_ref().is_some_and(|t| !t.is_finished()) {
                    return false;
                }
                if let Ok(dir) = session_dir(session_id) {
                    let _ = write_marker(&dir, "storage_warning");
                }
                writer.finish_acknowledged = true;
                writer.finish_reply = None;
            }
        }
    }

    let started = Instant::now();
    while writer.thread.as_ref().is_some_and(|thread| !thread.is_finished()) {
        let remaining = Duration::from_millis(250).saturating_sub(started.elapsed());
        if remaining.is_zero() { return false }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
    let mut owned = guard.take().expect("matching archive owner retained");
    owned.thread.take().is_none_or(|handle| handle.join().is_ok())
}

pub fn list_saved_sessions() -> SavedMeetingSessions {
    let root = archive_root();
    let mut result = SavedMeetingSessions {
        ok: true, sessions: Vec::new(), message: String::new(), active_write_errors: 0,
    };
    let active = writer_store().lock().ok().and_then(|g| g.as_ref().map(|w| (
        w.session_id.clone(), w.errors.load(Ordering::Acquire),
    )));
    if let Some((_, errors)) = active.as_ref() { result.active_write_errors = *errors; }
    if !root.exists() {
        return result;
    }
    let dir_entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return SavedMeetingSessions { ok: false, message: "Saved transcripts could not be listed.".to_string(), ..result },
    };
    for entry in dir_entries {
        let Ok(entry) = entry else { result.ok = false; continue };
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if !valid_id(&id) { continue }
        if !entry.file_type().is_ok_and(|f| f.is_dir()) { result.ok = false; continue }
        match scan_stored_turns(&entry.path()) {
            // Retain empty archives in History as well. They can result from
            // sessions with no recognized speech or from interrupted storage.
            // Hiding them would prevent the user's explicit Delete action.
            Ok(scan) => {
                let turns = &scan.paths;
                // A finished marker alone is insufficient: after a crash or
                // accidental deletion, compare its expected count with the
                // actual surviving entry filenames. Old unversioned markers
                // fail closed as unverified, never silently upgraded.
                let count_matches = matches!(
                    finished_receipt_count(&entry.path()),
                    Ok(Some(expected)) if expected == turns.len()
                );
                let storage_warning = entry.path().join("storage_warning").exists()
                    || scan.has_unpublished_files
                    || !count_matches
                    || active.as_ref().is_some_and(|(current, issues)| current == &id && *issues > 0);
                let complete = count_matches && !storage_warning;
                result.sessions.push(SavedMeetingSession {
                    session_id: id, entry_count: turns.len(), complete, storage_warning,
                });
            }
            Err(_) => result.ok = false,
        }
    }
    result.sessions.sort_by(|a,b| b.session_id.cmp(&a.session_id));
    if !result.ok {
        result.message = "Some saved sessions could not be inspected.".to_string();
    }
    result
}

pub(super) fn saved_session_entry_count(id: &str) -> Result<usize, String> {
    let dir = session_dir(id)?;
    Ok(stored_turn_paths(&dir)
        .map_err(|_| "Cannot list saved transcript entries.".to_string())?.len())
}

fn read_validated_turn(path: &Path, session_id: &str) -> Result<MeetingCommittedTurn, String> {
    // Reuse one strict reader for both Preview and Export. Streaming JSON
    // decoding avoids an additional allocation for the entire encoded file.
    let file = File::open(path)
        .map_err(|_| "A saved transcript entry could not be read.".to_string())?;
    let turn: MeetingCommittedTurn = serde_json::from_reader(BufReader::new(file))
        .map_err(|_| "A saved transcript entry is damaged or incomplete.".to_string())?;
    let file_sequence = path.file_name().and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("turn_"))
        .and_then(|name| name.strip_suffix(".json"))
        .and_then(|number| number.parse::<u64>().ok());
    if file_sequence != Some(turn.sequence) || turn.sequence == 0
        || turn.session_id != session_id
        || !matches!(turn.lane.as_str(), "you" | "incoming")
        || !matches!(turn.source_language.as_str(), "id" | "en")
        || !matches!(turn.target_language.as_str(), "id" | "en")
        || turn.source_language == turn.target_language
        || turn.source_text.trim().is_empty()
        || !matches!(turn.translation_state.as_str(), "pending" | "translated" | "failed")
        || (turn.translation_state == "translated" && turn.translated_text.trim().is_empty())
    {
        return Err("Saved transcript entry content or identity is invalid.".to_string());
    }
    Ok(turn)
}

pub(super) fn stream_saved_turns(
    id: &str,
    mut accept: impl FnMut(&MeetingCommittedTurn) -> Result<(), String>,
) -> Result<usize, String> {
    let dir = session_dir(id)?;
    let paths = stored_turn_paths(&dir)
        .map_err(|_| "Cannot list saved transcript entries.".to_string())?;
    let count = paths.len();
    for path in paths {
        let mut turn = read_validated_turn(&path, id)?;
        if turn.translation_state == "pending" {
            turn.translation_state = "failed".to_string();
            if turn.lane == "you" { turn.delivery_state = Some("interrupted".to_string()); }
        }
        accept(&turn)?;
    }
    Ok(count)
}

pub(super) fn session_warning(id: &str) -> Result<bool, String> {
    let dir = session_dir(id)?;
    if !dir.is_dir() { return Err("Saved Meeting transcript was not found.".to_string()) }
    let scan = scan_stored_turns(&dir)
        .map_err(|_| "Could not verify saved Meeting entry count.".to_string())?;
    let finalized = matches!(finished_receipt_count(&dir), Ok(Some(expected)) if expected == scan.paths.len());
    Ok(dir.join("storage_warning").exists()
        || scan.has_unpublished_files
        || !finalized
        || active_write_errors(id) > 0)
}

pub(super) fn read_saved_turns(id: &str, preview_limit: Option<usize>) -> Result<Vec<MeetingCommittedTurn>, String> {
    let dir = session_dir(id)?;
    let mut paths = stored_turn_paths(&dir).map_err(|_| "Cannot read saved transcript entries.".to_string())?;
    if let Some(limit) = preview_limit {
        let first = paths.len().saturating_sub(limit.min(MAX_SAVED_PREVIEW));
        paths.drain(..first);
    }
    // The session state does not change during a single bounded preview.
    // Resolve it once rather than taking the runtime state lock per entry.
    let active = latest_runtime_session_state().snapshot.as_ref().is_some_and(
        |session| session.owner_id == APPLICATION_MEETING_OWNER_ID && session.session_id == id
    );
    let mut turns = Vec::with_capacity(paths.len());
    for path in paths {
        let mut turn = read_validated_turn(&path, id)?;
        if !active && turn.translation_state == "pending" {
            turn.translation_state = "failed".to_string();
            if turn.lane == "you" { turn.delivery_state = Some("interrupted".to_string()); }
        }
        turns.push(turn);
    }
    Ok(turns)
}

pub fn saved_session_preview(session_id: String) -> MeetingCommittedTurnsSnapshot {
    // A saved read must not race explicit Delete. This disk-only owner is not
    // shared with the live Meeting audio, ASR, translation or writer queue.
    let _operation = match saved_session_operation_guard() {
        Ok(guard) => guard,
        Err(error) => return MeetingCommittedTurnsSnapshot {
            ok: false, has_session: false, session_id: Some(session_id),
            turns: Vec::new(), dropped_turn_count: 0, archive_write_errors: 0, truncated: false,
            blocker: "transcript_archive:read_lock_failed".to_string(), note: error,
            runtime_claim: "saved_transcript_disk_read_not_verified".to_string(),
        },
    };
    match read_saved_turns(&session_id, Some(MAX_SAVED_PREVIEW)) {
        Ok(turns) => MeetingCommittedTurnsSnapshot {
            ok: true, has_session: false, session_id: Some(session_id),
            turns, dropped_turn_count: 0, archive_write_errors: 0, truncated: false,
            blocker: String::new(),
            note: "Preview shows at most 240 recent saved entries. Export uses all archived entries.".to_string(),
            runtime_claim: "saved_transcript_disk_preview_source_not_windows_verified".to_string(),
        },
        Err(error) => MeetingCommittedTurnsSnapshot {
            ok: false, has_session: false, session_id: Some(session_id),
            turns: Vec::new(), dropped_turn_count: 0, archive_write_errors: 0, truncated: false,
            blocker: "transcript_archive:read_failed".to_string(), note: error,
            runtime_claim: "saved_transcript_disk_read_not_verified".to_string(),
        },
    }
}

pub fn delete_saved_session(session_id: String) -> ArchiveActionResult {
    // Export may be streaming this session's files. Wait until publication
    // finishes before allowing explicit deletion of the source archive.
    let _operation = match saved_session_operation_guard() {
        Ok(guard) => guard,
        Err(message) => return ArchiveActionResult { ok: false, message },
    };
    let dir = match session_dir(&session_id) {
        Ok(dir) => dir, Err(message) => return ArchiveActionResult { ok: false, message },
    };
    if latest_runtime_session_state().snapshot.as_ref().is_some_and(|session|
        session.owner_id == APPLICATION_MEETING_OWNER_ID && session.session_id == session_id
    ) {
        return ArchiveActionResult { ok: false, message: "Stop this Meeting before deleting its transcript.".to_string() };
    }
    if writer_store().lock().ok().and_then(|g| g.as_ref().map(|w| w.session_id == session_id)).unwrap_or(false) {
        return ArchiveActionResult { ok: false, message: "Transcript storage is still finishing. Retry after Stop.".to_string() };
    }
    let valid_dir = dir.is_dir() && !fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink());
    if !valid_dir {
        return ArchiveActionResult { ok: false, message: "Saved transcript folder is not available.".to_string() };
    }
    match fs::remove_dir_all(dir) {
        Ok(()) => {
            super::committed_turns::forget_ended_saved_session(&session_id);
            ArchiveActionResult { ok: true, message: "Saved transcript deleted from this device.".to_string() }
        },
        Err(_) => ArchiveActionResult { ok: false, message: "Saved transcript could not be deleted.".to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::{turn_name, valid_id, reserve_queued_bytes, finished_receipt_count,
        stored_turn_paths, scan_stored_turns, write_finished_receipt, MAX_QUEUED_ESTIMATED_BYTES};
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn saved_session_paths_accept_only_generated_meeting_identity() {
        assert!(valid_id("meeting_1760600000000_23"));
        assert!(!valid_id("../meeting_1"));
        assert!(!valid_id("meeting_../secret"));
        assert!(!valid_id("other_1760600000000_1"));
        assert_eq!(turn_name(22), "turn_00000000000000000022.json");
    }

    #[test]
    fn finished_receipt_rejects_missing_and_legacy_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "ti-archive-receipt-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos(),
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(finished_receipt_count(&dir).unwrap().is_none());
        write_finished_receipt(&dir).unwrap();
        assert_eq!(finished_receipt_count(&dir).unwrap(), Some(0));
        // The receipt no longer matches after a file appears/disappears.
        std::fs::write(dir.join(turn_name(1)), b"{}").unwrap();
        assert_ne!(finished_receipt_count(&dir).unwrap().unwrap(), stored_turn_paths(&dir).unwrap().len());
        std::fs::write(dir.join("turn_00000000000000000002.tmp"), b"partial").unwrap();
        assert!(scan_stored_turns(&dir).unwrap().has_unpublished_files);
        assert!(write_finished_receipt(&dir).is_err(), "staged entry must block successful finalization");
        std::fs::write(dir.join("finished"), b"legacy opaque marker").unwrap();
        assert!(finished_receipt_count(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_saved_entry_never_silently_enters_preview_or_html_export() {
        let session = "meeting_1760600000000_9876";
        let dir = std::env::temp_dir().join(format!(
            "ti-archive-entry-validation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos(),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(turn_name(1));
        let mut value = serde_json::json!({
            "session_id": session,
            "sequence": 1,
            "generation": null,
            "utterance_id": 1,
            "lane": "incoming",
            "source_language": "en",
            "target_language": "id",
            "source_text": "Document revision",
            "translated_text": "Revisi dokumen",
            "translation_state": "translated",
            "delivery_state": null,
            "outbound_timing": null,
            "created_unix_ms": 1760600000000_u128,
            "updated_unix_ms": 1760600000001_u128,
        });
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(super::read_validated_turn(&path, session).unwrap().sequence, 1);

        value["sequence"] = serde_json::json!(2);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(super::read_validated_turn(&path, session).is_err(),
            "filename/entry sequence mismatch must not be accepted");
        value["sequence"] = serde_json::json!(1);
        value["target_language"] = serde_json::json!("unknown");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(super::read_validated_turn(&path, session).is_err(),
            "invalid language contract must be rejected");
        std::fs::write(&path, b"{bad json").unwrap();
        assert!(super::read_validated_turn(&path, session).is_err(),
            "truncated or damaged JSON must fail closed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn queued_record_byte_budget_is_bounded_and_released() {
        let pending = AtomicUsize::new(MAX_QUEUED_ESTIMATED_BYTES - 32);
        assert!(reserve_queued_bytes(&pending, 32));
        assert_eq!(pending.load(Ordering::Relaxed), MAX_QUEUED_ESTIMATED_BYTES);
        assert!(!reserve_queued_bytes(&pending, 1));
        assert!(!reserve_queued_bytes(&pending, usize::MAX));
        pending.fetch_sub(32, Ordering::AcqRel);
        assert!(reserve_queued_bytes(&pending, 32));
    }
}

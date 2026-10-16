// Persistent, local-only Meeting transcript archive. Transcript authority remains
// in committed_turns.rs. This owner only writes immutable session identity and
// replaceable snapshots of individual authorized turns off the audio/AI threads.
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};
use super::{MeetingCommittedTurn, MeetingCommittedTurnsSnapshot};

const WRITE_QUEUE_SIZE: usize = 256;
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

enum Record {
    Turn(MeetingCommittedTurn),
    Finish(mpsc::Sender<bool>),
}

struct ArchiveWriter {
    session_id: String,
    sender: SyncSender<Record>,
    thread: Option<JoinHandle<()>>,
    errors: Arc<AtomicU64>,
    finish_reply: Option<mpsc::Receiver<bool>>,
    finish_acknowledged: bool,
}

static WRITER: OnceLock<Mutex<Option<ArchiveWriter>>> = OnceLock::new();

fn writer_store() -> &'static Mutex<Option<ArchiveWriter>> {
    WRITER.get_or_init(|| Mutex::new(None))
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

fn stored_turn_paths(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with("turn_") && name.ends_with(".json")
            && name[5..name.len()-5].len() == 20
            && name[5..name.len()-5].bytes().all(|b| b.is_ascii_digit())
        {
            if !entry.file_type()?.is_file() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Archive turn is not a regular file"));
            }
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn atomic_turn_write(dir: &Path, turn: &MeetingCommittedTurn) -> io::Result<()> {
    // The writer is the only updater for this session. An interrupted .tmp is
    // never mistaken for an archived turn by listing/export.
    let destination = dir.join(turn_name(turn.sequence));
    let temporary = dir.join(format!("turn_{:020}.tmp", turn.sequence));
    let bytes = serde_json::to_vec(turn).map_err(io::Error::other)?;
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &destination)?;
    Ok(())
}

fn write_marker(dir: &Path, marker: &str) -> io::Result<()> {
    // Do not leave a falsely completed marker if the process dies midway.
    let temporary = dir.join(format!("{marker}.tmp"));
    let final_path = dir.join(marker);
    let mut file = File::create(&temporary)?;
    file.write_all(b"TranslateIT local Meeting transcript archive\n")?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, final_path)
}

fn run_writer(dir: PathBuf, receiver: mpsc::Receiver<Record>, errors: Arc<AtomicU64>) {
    for record in receiver {
        match record {
            Record::Turn(turn) => {
                if atomic_turn_write(&dir, &turn).is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    let _ = write_marker(&dir, "storage_warning");
                }
            }
            Record::Finish(reply) => {
                let clean = errors.load(Ordering::Acquire) == 0;
                let marked = write_marker(&dir, if clean { "finished" } else { "storage_warning" }).is_ok();
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
    fs::create_dir_all(&dir).map_err(|_| "Could not create the local transcript archive folder.".to_string())?;
    // A session ID must never silently reuse a prior archive after restart.
    if stored_turn_paths(&dir).map_err(|_| "Could not inspect the transcript archive folder.".to_string())?.len() > 0
        || dir.join("finished").exists() || dir.join("storage_warning").exists()
    {
        return Err("Meeting transcript archive identity already exists.".to_string());
    }
    let (sender, receiver) = mpsc::sync_channel(WRITE_QUEUE_SIZE);
    let errors = Arc::new(AtomicU64::new(0));
    let worker_errors = Arc::clone(&errors);
    let handle = thread::Builder::new().name("translateit-transcript-storage".to_string())
        .spawn(move || run_writer(dir, receiver, worker_errors))
        .map_err(|_| "Could not start local transcript storage.".to_string())?;
    *guard = Some(ArchiveWriter {
        session_id: session_id.to_string(), sender, thread: Some(handle), errors,
        finish_reply: None, finish_acknowledged: false,
    });
    Ok(())
}

pub(super) fn queue_turn(turn: &MeetingCommittedTurn) {
    let Ok(guard) = writer_store().lock() else { return };
    let Some(writer) = guard.as_ref() else { return };
    if writer.session_id != turn.session_id { return }
    match writer.sender.try_send(Record::Turn(turn.clone())) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            writer.errors.fetch_add(1, Ordering::Relaxed);
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
        match stored_turn_paths(&entry.path()) {
            // Retain empty archives in History as well. They can result from
            // sessions with no recognized speech or from interrupted storage.
            // Hiding them would prevent the user's explicit Delete action.
            Ok(turns) => {
                let complete = entry.path().join("finished").exists();
                let storage_warning = entry.path().join("storage_warning").exists()
                    || !complete
                    || active.as_ref().is_some_and(|(current, issues)| current == &id && *issues > 0);
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

pub(super) fn stream_saved_turns(
    id: &str,
    mut accept: impl FnMut(&MeetingCommittedTurn) -> Result<(), String>,
) -> Result<usize, String> {
    let dir = session_dir(id)?;
    let paths = stored_turn_paths(&dir)
        .map_err(|_| "Cannot list saved transcript entries.".to_string())?;
    let count = paths.len();
    for path in paths {
        let bytes = fs::read(&path)
            .map_err(|_| "A saved transcript entry could not be read.".to_string())?;
        let mut turn: MeetingCommittedTurn = serde_json::from_slice(&bytes)
            .map_err(|_| "A saved transcript entry is damaged.".to_string())?;
        if turn.session_id != id {
            return Err("A saved transcript entry belongs to a different session.".to_string());
        }
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
    Ok(dir.join("storage_warning").exists()
        || !dir.join("finished").exists()
        || active_write_errors(id) > 0)
}

pub(super) fn read_saved_turns(id: &str, preview_limit: Option<usize>) -> Result<Vec<MeetingCommittedTurn>, String> {
    let dir = session_dir(id)?;
    let mut paths = stored_turn_paths(&dir).map_err(|_| "Cannot read saved transcript entries.".to_string())?;
    if let Some(limit) = preview_limit {
        let first = paths.len().saturating_sub(limit.min(MAX_SAVED_PREVIEW));
        paths.drain(..first);
    }
    let mut turns = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path).map_err(|_| "A saved transcript entry could not be read.".to_string())?;
        let mut turn: MeetingCommittedTurn = serde_json::from_slice(&bytes)
            .map_err(|_| "A saved transcript entry is damaged.".to_string())?;
        if turn.session_id != id { return Err("Saved transcript entry belongs to another session.".to_string()) }
        let active = latest_runtime_session_state().snapshot.as_ref().is_some_and(
            |session| session.owner_id == APPLICATION_MEETING_OWNER_ID && session.session_id == id
        );
        if !active && turn.translation_state == "pending" {
            turn.translation_state = "failed".to_string();
            if turn.lane == "you" { turn.delivery_state = Some("interrupted".to_string()); }
        }
        turns.push(turn);
    }
    Ok(turns)
}

pub fn saved_session_preview(session_id: String) -> MeetingCommittedTurnsSnapshot {
    match read_saved_turns(&session_id, Some(MAX_SAVED_PREVIEW)) {
        Ok(turns) => MeetingCommittedTurnsSnapshot {
            ok: true, has_session: false, session_id: Some(session_id),
            turns, dropped_turn_count: 0, truncated: false,
            blocker: String::new(),
            note: "Preview shows at most 240 recent saved entries. Export uses all archived entries.".to_string(),
            runtime_claim: "saved_transcript_disk_preview_source_not_windows_verified".to_string(),
        },
        Err(error) => MeetingCommittedTurnsSnapshot {
            ok: false, has_session: false, session_id: Some(session_id),
            turns: Vec::new(), dropped_turn_count: 0, truncated: false,
            blocker: "transcript_archive:read_failed".to_string(), note: error,
            runtime_claim: "saved_transcript_disk_read_not_verified".to_string(),
        },
    }
}

pub fn delete_saved_session(session_id: String) -> ArchiveActionResult {
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
    use super::{turn_name, valid_id};
    #[test]
    fn saved_session_paths_accept_only_generated_meeting_identity() {
        assert!(valid_id("meeting_1760600000000_23"));
        assert!(!valid_id("../meeting_1"));
        assert!(!valid_id("meeting_../secret"));
        assert!(!valid_id("other_1760600000000_1"));
        assert_eq!(turn_name(22), "turn_00000000000000000022.json");
    }
}

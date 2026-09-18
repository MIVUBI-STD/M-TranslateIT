use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::engine::audio::guided_take::{
    active_guided_take_line_id, arm_guided_take, cancel_guided_take, take_guided_audio,
    GuidedTakeReview, GUIDED_TAKE_CHANNELS, GUIDED_TAKE_SAMPLE_RATE_HZ,
};
use crate::engine::audio::live_capture::{start_live_capture_runtime, stop_live_capture_runtime};
use crate::engine::audio::live_segment_writer::write_pcm16_wav;
use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{
    begin_direct_live_capture_session, clear_runtime_session_if_generation,
    latest_runtime_session_state,
    mark_runtime_session_cleanup_incomplete, revoke_runtime_session_authority,
};

use super::voice_lab::{current_voice_lab_build_snapshot, VoiceLabStoragePaths};

mod storage_transaction;

use storage_transaction::{accept_review_take, discard_review_take};

const CAPTURE_OWNER_ID: &str = "translateit_rust_live_capture";
const MAX_REPLAY_WAV_BYTES: u64 = 8 * 1024 * 1024;

const GUIDED_LINES: &[(u32, &str)] = &[
    (1, "Good morning, everyone."),
    (2, "Thanks for joining today."),
    (3, "Yes, that works for me."),
    (4, "No, not yet."),
    (5, "Give me a moment."),
    (6, "I can hear you clearly."),
    (7, "Could you say that again?"),
    (8, "Let's keep going."),
    (9, "I'm ready when you are."),
    (10, "That sounds much better."),
    (11, "I need a little more context."),
    (12, "Please open the latest file."),
    (13, "I haven't seen that before."),
    (14, "Wait, I think I found it."),
    (15, "Sorry, I missed that."),
    (16, "Can everyone see my screen?"),
    (17, "The first option feels safer."),
    (18, "The second one is more flexible."),
    (19, "I prefer the simpler version."),
    (20, "We should test both."),
    (21, "Really? That's great news."),
    (22, "The call starts at nine."),
    (23, "I'll check it after lunch."),
    (24, "Let's talk tomorrow morning."),
    (25, "Thanks for making time. I'd like to review the plan before we decide."),
    (26, "Could you explain which part of the project needs the most attention?"),
    (27, "I agree with the direction, but I want the final result to feel natural."),
    (28, "I don't think we should rush this change before everyone understands the risk."),
    (29, "That's a fair question. Give me a moment to check the numbers first."),
    (30, "Would you mind repeating the last point a little more slowly, please?"),
    (31, "The bright morning light came through the window while we talked over breakfast."),
    (32, "She wore a blue shirt, a gray jacket, and comfortable walking shoes."),
    (33, "Three reviewers checked the draft carefully and shared their notes with the group."),
    (34, "We shipped twelve devices on Thursday, and eight more will arrive next week."),
    (35, "The meeting starts at nine thirty and should finish before eleven."),
    (36, "Please check the A P I response, the server status, and the latest version."),
    (37, "The wireless signal was weak for a moment, but the audio stayed clear."),
    (38, "I really appreciate your help. The result feels much better than yesterday."),
    (39, "Are we confident this will still work when the conversation gets faster?"),
    (40, "Sometimes I speak softly, and sometimes I emphasize the words that matter."),
    (41, "Before we finish, let's summarize what changed and who owns each action."),
    (42, "I wasn't expecting that result, but I'm glad we found the problem early."),
    (43, "Could we pause for a second and make sure we're viewing the same document?"),
    (44, "Absolutely. I can handle that task and send you an update when it's done."),
    (45, "I'm not fully convinced yet, so I'd like to hear one more example."),
    (46, "What happens if the network slows down while several people are speaking?"),
    (47, "Please don't worry about the delay. We still have enough time today."),
    (48, "The final price is three hundred forty eight dollars and fifty cents."),
    (49, "Our call begins at seven forty five and may last about ninety minutes."),
    (50, "I sent the revised proposal to Sarah, Michael, James, and Olivia this afternoon."),
    (51, "The room has glass walls, soft chairs, warm lights, and wooden shelves."),
    (52, "Please verify the username, project code, file path, and backup location."),
    (53, "After the update, the application opened quickly and responded without freezing."),
    (54, "The first attempt failed, but the second worked after we changed the settings."),
    (55, "When the room is quiet, my voice is relaxed and slightly softer."),
    (56, "I can hear a faint buzzing near the window, but it isn't distracting."),
    (57, "The weather changed quickly from bright sunshine to heavy rain this afternoon."),
    (58, "My brother bought vegetables, orange juice, chocolate, and a loaf of bread."),
    (59, "We crossed the bridge, walked through the station, and waited by the clock."),
    (60, "A small change in timing can make a conversation feel much smoother."),
    (61, "Please put the green folder beside the blue box on the desk."),
    (62, "Which option gives us the clearest result without creating unnecessary work?"),
    (63, "Why did the previous version behave differently when the settings looked the same?"),
    (64, "How quickly can we verify the fix without skipping the checks that matter?"),
    (65, "The latest build is version two point six, and one issue is still under review."),
    (66, "The first milestone is scheduled for Monday, September fourteenth."),
    (67, "There are twenty four tasks in total, and seven still need confirmation."),
    (68, "We can keep the current approach if it stays stable and easy to maintain."),
    (69, "The development team will test the change before it goes into the release."),
    (70, "The customer reported poor audio quality during a long online meeting."),
    (71, "We should focus on reliability, clear communication, and consistent performance."),
    (72, "My email address, phone number, and account details should remain private."),
    (73, "If something goes wrong, the system should recover safely and explain what happened."),
    (74, "The download is at ninety percent, so it should finish soon."),
    (75, "The warehouse counted one thousand two hundred seventy six items packed in forty three boxes."),
    (76, "The newest copy of Project Delta Review was saved on August twenty first."),
    (77, "Even when I speak faster, I try to keep names and numbers clear."),
    (78, "We need a clear yes or no before Friday so the supplier can plan."),
    (79, "Please write down the serial number, device name, and exact time of the issue."),
    (80, "I understand the concern, but the current approach still feels more reliable."),
    (81, "When several people speak at once, I wait for a short pause before answering."),
    (82, "If we change the behavior now, we should update the documentation and tests too."),
    (83, "This issue only appears after the application has been running for a while."),
    (84, "I want the final experience to feel calm and predictable when problems happen."),
    (85, "Could you compare these two recordings and tell me which one sounds clearer?"),
    (86, "Before we approve the release, let's test the full setup from start to finish."),
    (87, "The customer asked for Wednesday, but Friday morning is a safer target."),
    (88, "The interface looks simple, but several steps happen before the result is ready."),
    (89, "I don't mind waiting a few seconds if the translation is complete and natural."),
    (90, "The report covers current status, completed work, remaining risks, and next actions."),
    (91, "I wasn't sure about that word, so I slowed down and tried again."),
    (92, "A good conversation includes quick replies, normal explanations, and longer sentences."),
    (93, "Today is Tuesday, October sixth, and our next review is Friday morning."),
    (94, "The invoice total is two thousand nine hundred eighty four dollars including delivery."),
    (95, "Version three point four point one fixes the connection bug we reported."),
    (96, "Please send the file to Daniel in Jakarta and copy Maria from operations."),
    (97, "My laptop was nearly out of battery, so I lowered the screen brightness, closed a few applications, and connected the charger before the call."),
    (98, "The small wooden table near the window holds a silver lamp, two notebooks, a green bottle, and a pair of black headphones."),
    (99, "During the storm, the wind became stronger, the lights flickered twice, and the internet connection disappeared for a moment before returning."),
    (100, "I was surprised by the result at first, then relieved when we discovered that a simple setting was causing the problem."),
    (101, "If you disagree with my recommendation, that's completely fine. Tell me which assumption seems wrong, and we can review the evidence together."),
    (102, "The fastest solution isn't always the best one, especially when a shortcut creates extra work that someone has to handle later."),
    (103, "When I explain a technical problem, I usually start with what the user noticed, then describe the cause, the fix, and any remaining limitation."),
    (104, "Please let me know if the explanation stays clear when I speak slowly, at a normal pace, and a little faster."),
    (105, "We don't need to solve every possible future problem today. We only need to finish the work that actually matters right now."),
    (106, "After everyone reviews the proposal, we'll collect the comments, resolve the important disagreements, update the document, and send the final version."),
    (107, "Natural conversation can include hesitation, a quick correction, a short laugh, or a change in emphasis while the important words remain clear."),
    (108, "Thanks for taking the time to go through this with me. We've covered the main points, and I'm comfortable stopping here for today."),
    (109, "I like to leave a short pause before an important point because it gives the listener a moment to follow the change in direction."),
    (110, "If the connection drops while I'm speaking, I'll wait for it to return instead of repeating the whole explanation from the beginning."),
    (111, "The project is moving in the right direction, but I still want to review the final details before we call the work complete."),
    (112, "We tested the microphone in a quiet room, then repeated the same sentence with a fan running softly in the background."),
    (113, "When I joined the call this morning, the first thing I noticed was that everyone sounded clear and the delay was much lower."),
    (114, "The team agreed to keep the design simple, remove the unnecessary steps, and spend more time testing the parts that users actually notice."),
    (115, "I can explain the main idea in a few words, but I can also give more detail if the team needs a complete technical explanation."),
    (116, "The customer asked a simple question, but the answer depended on several details that we needed to verify before giving a confident response."),
    (117, "We reviewed the schedule together, moved two tasks to next week, and kept the final delivery date because the remaining work was still manageable."),
    (118, "I usually speak a little more slowly when I mention a name, a date, or a number because those details are easy to miss."),
    (119, "The new version starts faster, uses less memory, and feels smoother during normal use, but we still need to test longer sessions."),
    (120, "If the first plan doesn't work, we can try the simpler option tomorrow and compare the results before making a final decision."),
    (121, "The meeting was almost finished when someone raised one last question, so we stayed a few minutes longer to make sure the answer was clear."),
    (122, "I can hear a small difference between the two recordings. The second one sounds warmer, while the first one sounds slightly brighter and more distant."),
    (123, "We started with a rough idea, tested it with a few people, learned what was confusing, and then simplified the experience before the next review."),
    (124, "Sometimes a short answer is enough, but other times I need to explain the background so everyone understands why the decision matters."),
    (125, "I try to keep the same calm tone whether I'm asking a question, sharing an opinion, or explaining something that needs more detail."),
    (126, "If the team finds a serious problem during testing, we should describe exactly what happened, reproduce it carefully, and fix the cause instead of hiding the symptom."),
    (127, "I appreciate clear feedback, even when it challenges my first idea, because a better result matters more than defending a decision that no longer makes sense."),
    (128, "That's everything I wanted to cover today. If anything is still unclear, we can return to it later instead of rushing through the final few points."),
];

#[derive(Debug, Clone, Serialize)]
pub struct GuidedLineStatus {
    pub line_id: u32,
    pub text: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidedRecordingState {
    pub recording_line_id: Option<u32>,
    pub pending_review: Option<GuidedTakeReview>,
    pub lines: Vec<GuidedLineStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuidedRecordingActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub recording: GuidedRecordingState,
}

struct PendingDraft {
    line_id: u32,
    path: PathBuf,
    review: GuidedTakeReview,
}

static PENDING_DRAFT: OnceLock<Mutex<Option<PendingDraft>>> = OnceLock::new();

fn draft_store() -> &'static Mutex<Option<PendingDraft>> {
    PENDING_DRAFT.get_or_init(|| Mutex::new(None))
}

fn guided_text(line_id: u32) -> Option<&'static str> {
    GUIDED_LINES
        .iter()
        .find(|(id, _)| *id == line_id)
        .map(|(_, text)| *text)
}

fn take_file_name(line_id: u32) -> String {
    format!("take_{line_id:04}.wav")
}

fn storage_paths() -> VoiceLabStoragePaths {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
}

fn draft_path(line_id: u32) -> PathBuf {
    storage_paths()
        .cache_root
        .join("Draft")
        .join(take_file_name(line_id))
}

fn accepted_path(line_id: u32) -> PathBuf {
    storage_paths().takes_dir.join(take_file_name(line_id))
}

fn current_state() -> GuidedRecordingState {
    let pending_review = draft_store()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|draft| draft.review.clone()));
    GuidedRecordingState {
        recording_line_id: active_guided_take_line_id(),
        pending_review,
        lines: GUIDED_LINES
            .iter()
            .map(|(line_id, text)| GuidedLineStatus {
                line_id: *line_id,
                text: (*text).to_string(),
                accepted: accepted_path(*line_id).is_file(),
            })
            .collect(),
    }
}

fn result(ok: bool, state: &str, message: impl Into<String>) -> GuidedRecordingActionResult {
    GuidedRecordingActionResult {
        ok,
        state: state.to_string(),
        message: message.into(),
        recording: current_state(),
    }
}

#[tauri::command]
pub fn get_voice_lab_guided_recording_state() -> GuidedRecordingState {
    current_state()
}

#[tauri::command]
pub fn start_voice_lab_guided_take(
    line_id: u32,
    authorized_voice_confirmed: bool,
) -> GuidedRecordingActionResult {
    if !authorized_voice_confirmed {
        return result(false, "authorization_required", "Confirm that you own or are authorized to use this voice before recording.");
    }
    if guided_text(line_id).is_none() {
        return result(false, "invalid_line", "This guided reading line is not part of the current VoiceLab script.");
    }
    if current_voice_lab_build_snapshot().active {
        return result(false, "build_active", "Finish or cancel the current VoiceLab build before recording more lines.");
    }
    if draft_store().lock().ok().and_then(|guard| guard.as_ref().map(|draft| draft.line_id)).is_some() {
        return result(false, "review_pending", "Replay, retry, or accept the current take before recording another line.");
    }

    let session = begin_direct_live_capture_session();
    let Some(snapshot) = session.snapshot.as_ref() else {
        return result(false, "runtime_unavailable", "VoiceLab cannot verify microphone ownership right now.");
    };
    if !session.blocker.is_empty() || snapshot.owner_id != CAPTURE_OWNER_ID {
        return result(false, "microphone_in_use", "Stop Meeting translation or Mic Test before recording a VoiceLab line.");
    }
    let generation = snapshot.generation;
    if let Err(blocker) = arm_guided_take(line_id) {
        let _ = clear_runtime_session_if_generation(generation);
        return result(false, "capture_unavailable", blocker);
    }
    let capture = start_live_capture_runtime(session);
    if !capture.ok {
        cancel_guided_take();
        let _ = clear_runtime_session_if_generation(generation);
        return result(false, "capture_failed", capture.message);
    }
    result(true, "recording", "Recording started. Read the line naturally, then press Stop.")
}

#[tauri::command]
pub fn stop_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    if active_guided_take_line_id() != Some(line_id) {
        return result(false, "line_mismatch", "The requested line is not the guided take currently being recorded.");
    }
    let current = latest_runtime_session_state();
    let Some(snapshot) = current.snapshot.as_ref() else {
        return result(false, "runtime_unavailable", "VoiceLab cannot verify microphone ownership right now.");
    };
    if snapshot.owner_id != CAPTURE_OWNER_ID {
        return result(false, "owner_conflict", "VoiceLab does not own the active microphone session.");
    }
    let generation = snapshot.generation;
    if snapshot.authority_active {
        let revoked = revoke_runtime_session_authority(
            generation,
            "VoiceLab Stop accepted. Capture authority was revoked before microphone cleanup.",
        );
        if revoked.snapshot.as_ref().map(|value| value.authority_active).unwrap_or(true) {
            return result(false, "stop_failed", "VoiceLab could not revoke recording ownership safely.");
        }
    }

    let stopped = stop_live_capture_runtime();
    if !stopped.ok {
        let _ = mark_runtime_session_cleanup_incomplete(
            generation,
            true,
            "VoiceLab recording authority is revoked, but microphone cleanup still needs attention.",
        );
        return result(false, "stop_failed", stopped.message);
    }
    let captured = take_guided_audio();
    let cleared = clear_runtime_session_if_generation(generation);
    if cleared.has_active_session || cleared.snapshot.is_some() {
        return result(false, "cleanup_unverified", "The microphone stopped, but VoiceLab could not confirm that recording ownership was cleared.");
    }
    let Ok((captured, review)) = captured else {
        return result(false, "take_unusable", captured.err().unwrap_or_else(|| "voice_lab:take_unusable".to_string()));
    };
    if captured.line_id != line_id {
        return result(false, "line_mismatch", "The recorded audio does not belong to the requested guided line.");
    }
    let draft = draft_path(line_id);
    if let Err(error) = write_pcm16_wav(
        &draft,
        GUIDED_TAKE_SAMPLE_RATE_HZ,
        GUIDED_TAKE_CHANNELS,
        &captured.samples_mono,
    ) {
        let _ = fs::remove_file(&draft);
        return result(false, "draft_write_failed", format!("VoiceLab could not save the review take: {error}"));
    }
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => {
            let _ = fs::remove_file(&draft);
            return result(
                false,
                "draft_state_unavailable",
                "VoiceLab could not retain the review take state; the unowned review audio was removed.",
            );
        }
    };
    *guard = Some(PendingDraft { line_id, path: draft, review });
    drop(guard);
    result(true, "needs_review", "Recording stopped. Replay the take, then accept it or retry the line.")
}

#[tauri::command]
pub fn retry_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => return result(false, "draft_state_unavailable", "VoiceLab cannot access the pending review take."),
    };
    let Some(draft) = guard.as_ref() else {
        drop(guard);
        return result(true, "ready", "There is no pending review take to discard.");
    };
    if draft.line_id != line_id {
        drop(guard);
        return result(false, "line_mismatch", "The pending review take belongs to another guided line.");
    }
    let path = draft.path.clone();
    if let Err(error) = discard_review_take(&path) {
        drop(guard);
        return result(false, "discard_failed", error);
    }
    *guard = None;
    drop(guard);
    result(true, "ready", "The review take was discarded. The previous accepted take, if any, was kept.")
}

#[tauri::command]
pub fn accept_voice_lab_guided_take(line_id: u32) -> GuidedRecordingActionResult {
    let mut guard = match draft_store().lock() {
        Ok(guard) => guard,
        Err(_) => return result(false, "draft_state_unavailable", "VoiceLab cannot access the pending review take."),
    };
    let Some(draft) = guard.as_ref() else {
        drop(guard);
        return result(false, "no_review", "Record and review this line before accepting it.");
    };
    if draft.line_id != line_id {
        drop(guard);
        return result(false, "line_mismatch", "The pending review take belongs to another guided line.");
    }
    if !draft.review.quality_blocker.is_empty() {
        drop(guard);
        return result(false, "take_unusable", "This take is silent or empty. Retry the line before accepting it.");
    }
    let target = accepted_path(line_id);
    if let Some(parent) = target.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            drop(guard);
            return result(false, "save_failed", format!("VoiceLab could not prepare take storage: {error}"));
        }
    }
    if let Err(error) = accept_review_take(&draft.path, &target) {
        drop(guard);
        return result(false, "save_failed", error);
    }
    *guard = None;
    drop(guard);
    result(true, "accepted", "Take accepted and saved for the Voice Actor dataset.")
}

#[tauri::command]
pub fn get_voice_lab_guided_take_audio(line_id: u32) -> Result<tauri::ipc::Response, String> {
    if guided_text(line_id).is_none() {
        return Err("voice_lab:invalid_guided_line".to_string());
    }
    let pending_path = draft_store()
        .lock()
        .map_err(|_| "voice_lab:guided_draft_state_unavailable".to_string())?
        .as_ref()
        .filter(|draft| draft.line_id == line_id)
        .map(|draft| draft.path.clone());
    let path = pending_path.unwrap_or_else(|| accepted_path(line_id));
    let metadata = fs::metadata(&path).map_err(|_| "voice_lab:take_audio_missing".to_string())?;
    if !metadata.is_file() || metadata.len() < 44 || metadata.len() > MAX_REPLAY_WAV_BYTES {
        return Err("voice_lab:take_audio_invalid".to_string());
    }
    let bytes = fs::read(path).map_err(|_| "voice_lab:take_audio_read_failed".to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

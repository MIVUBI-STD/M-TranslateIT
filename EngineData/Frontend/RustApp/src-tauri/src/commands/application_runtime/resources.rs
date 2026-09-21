use crate::engine::runtime_state::{
    APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID,
};

use super::contract::{ApplicationSubsystemSummaries, ResourceArbitration};

fn owner_kind(owner: Option<&str>) -> &'static str {
    match owner {
        Some(APPLICATION_MEETING_OWNER_ID) => "meeting",
        Some(MIC_TEST_OWNER_ID) => "mic_test",
        Some(VOICE_RECORDING_OWNER_ID) => "voice_recording",
        Some(_) => "other",
        None => "none",
    }
}

pub fn resolve_resources(summaries: &ApplicationSubsystemSummaries) -> ResourceArbitration {
    let meeting = &summaries.meeting;
    let audio_locked = meeting.has_session;
    let blocker = if audio_locked {
        match meeting.owner_id.as_deref() {
            Some(owner) if !owner.is_empty() => format!("resource:audio_owned_by:{owner}"),
            _ => "resource:audio_owned_by_active_session".to_string(),
        }
    } else if !summaries.audio.ready {
        if summaries.audio.blocker.is_empty() {
            "resource:microphone_unavailable".to_string()
        } else {
            summaries.audio.blocker.clone()
        }
    } else {
        String::new()
    };

    ResourceArbitration {
        audio_locked,
        microphone_available: !audio_locked && summaries.audio.ready,
        meeting_audio_available: !audio_locked,
        active_owner: meeting.owner_id.clone(),
        owner_kind: owner_kind(meeting.owner_id.as_deref()).to_string(),
        blocker,
    }
}

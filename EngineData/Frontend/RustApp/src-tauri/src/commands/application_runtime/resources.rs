use super::contract::{ApplicationSubsystemSummaries, ResourceArbitration};

pub fn resolve_resources(summaries: &ApplicationSubsystemSummaries) -> ResourceArbitration {
    let meeting = &summaries.meeting;
    let occupied_by_session = meeting.has_session;
    let blocker = if occupied_by_session {
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
        microphone_available: !occupied_by_session && summaries.audio.ready,
        meeting_audio_available: !occupied_by_session,
        active_owner: meeting.owner_id.clone(),
        blocker,
    }
}

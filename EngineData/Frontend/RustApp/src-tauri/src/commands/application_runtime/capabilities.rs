use super::contract::{
    ApplicationCapabilities, ApplicationSubsystemSummaries, ResourceArbitration,
};

pub fn resolve_capabilities(
    summaries: &ApplicationSubsystemSummaries,
    resources: &ResourceArbitration,
) -> ApplicationCapabilities {
    let meeting = &summaries.meeting;
    let meeting_translation = if meeting.has_session {
        meeting.application_owned
    } else {
        meeting.ready_for_start
            && summaries.worker.ready
            && summaries.audio.ready
            && !summaries.voice.build_active
            && !summaries.voice.recording_active
    };

    ApplicationCapabilities {
        meeting_translation,
        mic_test: resources.microphone_available,
        text_translation: summaries.worker.ready,
    }
}

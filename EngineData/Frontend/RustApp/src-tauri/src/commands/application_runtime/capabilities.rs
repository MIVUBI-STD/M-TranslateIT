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
            && resources.microphone_available
            && !summaries.voice.build_active
            && !summaries.voice.recording_active
    };

    let mic_test = if resources.owner_kind == "mic_test" {
        true
    } else {
        resources.microphone_available
    };

    ApplicationCapabilities {
        meeting_translation,
        mic_test,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_capabilities;
    use crate::commands::application_runtime::contract::{
        ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, ResourceArbitration,
        VoiceSummary, WorkerSummary,
    };

    fn summaries(has_session: bool, application_owned: bool) -> ApplicationSubsystemSummaries {
        ApplicationSubsystemSummaries {
            meeting: MeetingSummary {
                lifecycle: if has_session { "live".to_string() } else { "idle".to_string() },
                has_session,
                application_owned,
                ready_for_start: !has_session,
                owner_id: None,
                blocker: String::new(),
                note: String::new(),
                preflight_blockers: Vec::new(),
            },
            worker: WorkerSummary {
                state: "ready".to_string(),
                process_ready: true,
                degraded: false,
                message: String::new(),
            },
            audio: AudioSummary {
                ready: true,
                running: has_session,
                blocker: String::new(),
                note: String::new(),
            },
            voice: VoiceSummary {
                recording_active: false,
                build_active: false,
                build_phase: "idle".to_string(),
            },
        }
    }

    #[test]
    fn active_mic_test_retains_its_own_capability() {
        let capabilities = resolve_capabilities(
            &summaries(true, false),
            &ResourceArbitration {
                audio_locked: true,
                microphone_available: false,
                active_owner: Some("translateit_mic_test".to_string()),
                owner_kind: "mic_test".to_string(),
                blocker: "resource:audio_owned_by:translateit_mic_test".to_string(),
            },
        );
        assert!(capabilities.mic_test);
        assert!(!capabilities.meeting_translation);
    }

    #[test]
    fn competing_audio_owner_blocks_mic_test_capability() {
        let capabilities = resolve_capabilities(
            &summaries(true, true),
            &ResourceArbitration {
                audio_locked: true,
                microphone_available: false,
                active_owner: Some("translateit_application_meeting".to_string()),
                owner_kind: "meeting".to_string(),
                blocker: "resource:audio_owned_by:translateit_application_meeting".to_string(),
            },
        );
        assert!(!capabilities.mic_test);
        assert!(capabilities.meeting_translation);
    }
}

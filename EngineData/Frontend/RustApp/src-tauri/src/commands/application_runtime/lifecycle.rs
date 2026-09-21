use crate::engine::runtime_state::{
    APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID,
};

use super::contract::ApplicationSubsystemSummaries;

pub fn derive_lifecycle(summaries: &ApplicationSubsystemSummaries) -> String {
    let meeting = &summaries.meeting;
    if meeting.has_session {
        match meeting.owner_id.as_deref() {
            Some(MIC_TEST_OWNER_ID) => return "mic_test_live".to_string(),
            Some(VOICE_RECORDING_OWNER_ID) => return "voice_recording_live".to_string(),
            _ if meeting.application_owned => {
                return match meeting.lifecycle.as_str() {
                    "live" | "listening" => "meeting_live".to_string(),
                    "starting" => "meeting_starting".to_string(),
                    "stopping" => "meeting_stopping".to_string(),
                    "cleanup_incomplete" => "recovering".to_string(),
                    value if value.is_empty() => "meeting_active".to_string(),
                    value => format!("meeting_{value}"),
                };
            }
            _ => return "audio_session_active".to_string(),
        }
    }

    if summaries.voice.build_active {
        return match summaries.voice.build_phase.as_str() {
            "cancelling" => "voice_build_stopping".to_string(),
            "evaluating" => "voice_build_evaluating".to_string(),
            _ => "voice_build_active".to_string(),
        };
    }

    if summaries.worker.process_ready && summaries.audio.ready {
        "ready".to_string()
    } else if summaries.worker.state == "frontend_bridge_error"
        || summaries.worker.state == "error"
    {
        "degraded".to_string()
    } else {
        "setup_required".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::derive_lifecycle;
    use crate::commands::application_runtime::contract::{
        ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, VoiceSummary, WorkerSummary,
    };
    use crate::engine::runtime_state::{MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID};

    fn summaries(
        meeting_lifecycle: &str,
        has_session: bool,
        owner_id: Option<&str>,
        worker_process_ready: bool,
        audio_ready: bool,
        voice_build_active: bool,
    ) -> ApplicationSubsystemSummaries {
        ApplicationSubsystemSummaries {
            meeting: MeetingSummary {
                lifecycle: meeting_lifecycle.to_string(),
                has_session,
                application_owned: owner_id == Some(APPLICATION_MEETING_OWNER_ID),
                ready_for_start: !has_session,
                owner_id: owner_id.map(str::to_string),
                blocker: String::new(),
                note: String::new(),
                preflight_blockers: Vec::new(),
            },
            worker: WorkerSummary {
                state: if worker_process_ready { "ready" } else { "not_started" }.to_string(),
                process_ready: worker_process_ready,
                degraded: false,
                message: String::new(),
            },
            audio: AudioSummary {
                ready: audio_ready,
                running: false,
                blocker: String::new(),
                note: String::new(),
            },
            voice: VoiceSummary {
                recording_active: owner_id == Some(VOICE_RECORDING_OWNER_ID),
                build_active: voice_build_active,
                build_phase: if voice_build_active { "training" } else { "idle" }.to_string(),
            },
        }
    }

    #[test]
    fn active_meeting_has_global_lifecycle_priority() {
        assert_eq!(
            derive_lifecycle(&summaries(
                "live",
                true,
                Some(APPLICATION_MEETING_OWNER_ID),
                true,
                true,
                false,
            )),
            "meeting_live"
        );
    }

    #[test]
    fn live_capture_owners_are_not_reported_as_meeting() {
        assert_eq!(
            derive_lifecycle(&summaries("live_capture_only", true, Some(MIC_TEST_OWNER_ID), true, true, false)),
            "mic_test_live"
        );
        assert_eq!(
            derive_lifecycle(&summaries("live_capture_only", true, Some(VOICE_RECORDING_OWNER_ID), true, true, false)),
            "voice_recording_live"
        );
    }

    #[test]
    fn voice_build_is_visible_in_global_lifecycle() {
        assert_eq!(
            derive_lifecycle(&summaries("idle", false, None, true, true, true)),
            "voice_build_active"
        );
    }

    #[test]
    fn ready_requires_worker_and_audio_summary_readiness() {
        assert_eq!(derive_lifecycle(&summaries("idle", false, None, true, true, false)), "ready");
        assert_eq!(derive_lifecycle(&summaries("idle", false, None, true, false, false)), "setup_required");
    }
}

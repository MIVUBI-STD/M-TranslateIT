use super::contract::ApplicationSubsystemSummaries;

pub fn derive_lifecycle(summaries: &ApplicationSubsystemSummaries) -> String {
    let meeting = &summaries.meeting;
    if meeting.has_session {
        return match meeting.lifecycle.as_str() {
            "live" | "listening" => "meeting_live".to_string(),
            "starting" => "meeting_starting".to_string(),
            "stopping" => "meeting_stopping".to_string(),
            "cleanup_incomplete" => "recovering".to_string(),
            value if value.is_empty() => "meeting_active".to_string(),
            value => format!("meeting_{value}"),
        };
    }

    if summaries.worker.ready && summaries.audio.ready {
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
        ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, WorkerSummary,
    };

    fn summaries(meeting_lifecycle: &str, has_session: bool, worker_ready: bool, audio_ready: bool) -> ApplicationSubsystemSummaries {
        ApplicationSubsystemSummaries {
            meeting: MeetingSummary {
                lifecycle: meeting_lifecycle.to_string(),
                has_session,
                application_owned: has_session,
                ready_for_start: !has_session,
                owner_id: None,
                blocker: String::new(),
                note: String::new(),
                preflight_blockers: Vec::new(),
            },
            worker: WorkerSummary {
                state: if worker_ready { "ready" } else { "not_started" }.to_string(),
                ready: worker_ready,
                degraded: false,
                message: String::new(),
            },
            audio: AudioSummary {
                ready: audio_ready,
                running: false,
                blocker: String::new(),
                note: String::new(),
            },
        }
    }

    #[test]
    fn active_meeting_has_global_lifecycle_priority() {
        assert_eq!(derive_lifecycle(&summaries("live", true, true, true)), "meeting_live");
    }

    #[test]
    fn ready_requires_worker_and_audio_summary_readiness() {
        assert_eq!(derive_lifecycle(&summaries("idle", false, true, true)), "ready");
        assert_eq!(derive_lifecycle(&summaries("idle", false, true, false)), "setup_required");
    }
}

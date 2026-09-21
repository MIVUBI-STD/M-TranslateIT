use crate::engine::audio::input::InputPreparationStatus;

use super::contract::{
    ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, WorkerSummary,
};
use super::super::helper_bridge_runtime::HelperBridgeStatus;
use super::super::meeting_session::MeetingSessionStatus;

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

pub fn build_subsystem_summaries(
    meeting: &MeetingSessionStatus,
    helper: &HelperBridgeStatus,
    input: &InputPreparationStatus,
) -> ApplicationSubsystemSummaries {
    ApplicationSubsystemSummaries {
        meeting: MeetingSummary {
            lifecycle: meeting.lifecycle.clone(),
            has_session: meeting.has_session,
            application_owned: meeting.owner_id.as_deref() == Some(APPLICATION_MEETING_OWNER_ID),
            ready_for_start: meeting.preflight.ready_for_start,
            owner_id: meeting.owner_id.clone(),
            blocker: meeting.blocker.clone(),
            note: meeting.note.clone(),
            preflight_blockers: meeting.preflight.blockers.clone(),
        },
        worker: WorkerSummary {
            state: helper.state.clone(),
            ready: helper.state == "ready",
            degraded: helper.degraded_mode,
            message: helper.message.clone(),
        },
        audio: AudioSummary {
            ready: input.prepared,
            running: input.running,
            blocker: input.blocker.clone(),
            note: input.note.clone(),
        },
    }
}

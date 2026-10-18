use crate::engine;
use crate::engine::state::{CommandResult, LifecycleState};

pub fn start_capture() -> CommandResult {
    // No-session Meeting Retry Stop may still own microphone teardown.
    // Do not grant a new Mic Test capture owner from the public command.
    if super::meeting_session::meeting_stop_cleanup_pending() {
        return CommandResult::blocked(
            LifecycleState::ConversionPending,
            "Finish Meeting cleanup using Retry Stop before starting Mic Test.",
        );
    }
    engine::start_capture()
}

pub fn stop_capture() -> CommandResult {
    engine::stop_capture()
}

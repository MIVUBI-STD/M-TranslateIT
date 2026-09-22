use crate::engine;
use crate::engine::state::CommandResult;

pub fn start_capture() -> CommandResult {
    engine::start_capture()
}

pub fn stop_capture() -> CommandResult {
    engine::stop_capture()
}

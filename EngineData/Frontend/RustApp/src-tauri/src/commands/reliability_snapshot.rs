use serde::Serialize;

use super::device_loss_guard::{get_device_loss_guard_status, DeviceLossGuardStatus};
use super::runtime_watchdog::{get_runtime_watchdog_status, RuntimeWatchdogStatus};

#[derive(Debug, Clone, Serialize)]
pub struct MeetingReliabilitySnapshot {
    pub watchdog: RuntimeWatchdogStatus,
    pub devices: DeviceLossGuardStatus,
}

#[tauri::command]
pub fn get_meeting_reliability_snapshot() -> MeetingReliabilitySnapshot {
    MeetingReliabilitySnapshot {
        watchdog: get_runtime_watchdog_status(),
        devices: get_device_loss_guard_status(),
    }
}

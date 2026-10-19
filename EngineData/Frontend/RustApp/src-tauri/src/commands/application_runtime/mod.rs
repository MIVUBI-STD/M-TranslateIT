mod capabilities;
mod contract;
mod events;
mod intents;
mod lifecycle;
pub(crate) mod mutations;
mod problems;
mod resources;
mod snapshot;
mod shutdown;
mod summaries;

pub use contract::{ApplicationIntentResult, ApplicationSnapshot};
pub use shutdown::prepare_for_app_exit;
pub use snapshot::current_application_snapshot;

pub(crate) fn notify_meeting_control_change(app: &tauri::AppHandle, reason: &str) {
    let snapshot = current_application_snapshot();
    events::emit_application_snapshot(app, reason, &snapshot);
}

// Native status may wait on the one worker I/O owner while CUDA/AI probes
// execute. Do not block the Windows WebView event thread waiting for that lock.
#[tauri::command]
pub async fn get_application_snapshot() -> Result<ApplicationSnapshot, String> {
    tauri::async_runtime::spawn_blocking(current_application_snapshot)
        .await
        .map_err(|_| "Application snapshot collection was interrupted.".to_string())
}

// Meeting startup and readiness probes can load multiple local AI models.
// Keep the existing single intent dispatcher and generation guards, but run
// the blocking operation on the dedicated Tauri blocking executor.
#[tauri::command]
pub async fn dispatch_product_intent(
    app: tauri::AppHandle,
    intent: String,
) -> Result<ApplicationIntentResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let normalized = intent.trim().to_ascii_lowercase();
        let outcome = intents::dispatch(&normalized);
        let snapshot = current_application_snapshot();
        events::emit_application_snapshot(&app, &normalized, &snapshot);

        ApplicationIntentResult {
            ok: outcome.ok,
            intent: normalized,
            state: outcome.state,
            message: outcome.message,
            snapshot,
        }
    })
    .await
    .map_err(|_| "Application intent worker was interrupted.".to_string())
}

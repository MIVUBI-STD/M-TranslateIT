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

#[tauri::command]
pub fn get_application_snapshot() -> ApplicationSnapshot {
    current_application_snapshot()
}

#[tauri::command]
pub fn dispatch_product_intent(
    app: tauri::AppHandle,
    intent: String,
) -> ApplicationIntentResult {
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
}

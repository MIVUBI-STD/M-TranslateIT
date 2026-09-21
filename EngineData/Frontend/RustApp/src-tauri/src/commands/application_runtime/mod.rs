mod capabilities;
mod contract;
mod events;
mod intents;
mod lifecycle;
mod problems;
mod resources;
mod snapshot;
mod summaries;

pub use contract::{
    ApplicationCapabilities, ApplicationInputStatus, ApplicationIntentResult, ApplicationProblem, ApplicationSnapshot,
    ApplicationSubsystemSummaries, AudioSummary, MeetingSummary, ResourceArbitration,
    WorkerSummary,
};
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

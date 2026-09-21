use serde::Serialize;
use tauri::Emitter;

use super::contract::ApplicationSnapshot;

#[derive(Serialize)]
struct ApplicationRuntimeEvent<'a> {
    reason: &'a str,
    snapshot: &'a ApplicationSnapshot,
}

pub fn emit_application_snapshot(
    app: &tauri::AppHandle,
    reason: &str,
    snapshot: &ApplicationSnapshot,
) {
    let _ = app.emit(
        "translateit://application-runtime",
        ApplicationRuntimeEvent { reason, snapshot },
    );
}

mod app_bootstrap;
mod commands;
mod engine;

use tauri::Manager;

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

fn main() {
    let app = commands::registry::register(
        tauri::Builder::default()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .setup(|app| app_bootstrap::configure_main_window(app)),
    )
    .build(tauri::generate_context!())
    .expect("TranslateIT app failed to build");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if commands::voice_lab_recording::voice_lab_recording_blocks_app_exit()
                || commands::voice_lab::current_voice_lab_build_snapshot().active
            {
                api.prevent_exit();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                return;
            }

            let runtime = engine::runtime_state::latest_runtime_session_state();
            let runtime_state_unavailable =
                runtime.has_active_session && runtime.snapshot.is_none();
            if runtime_state_unavailable {
                api.prevent_exit();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                return;
            }

            let active_owner = runtime
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.owner_id.as_str());

            if let Some(owner) = active_owner {
                if owner != APPLICATION_MEETING_OWNER_ID {
                    api.prevent_exit();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    return;
                }

                let result = commands::meeting_session::stop_meeting_translation();
                let meeting_still_owned = result.status.has_session
                    && result.status.owner_id.as_deref() == Some(APPLICATION_MEETING_OWNER_ID);

                if meeting_still_owned {
                    api.prevent_exit();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    return;
                }
            }

            if !commands::helper_bridge::shutdown_helper_bridge_for_app_exit() {
                api.prevent_exit();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    });
}

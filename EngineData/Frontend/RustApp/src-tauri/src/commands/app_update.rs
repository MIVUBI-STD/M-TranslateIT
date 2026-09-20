use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const UPDATE_ENDPOINT: &str =
    "https://github.com/MIVUBI-STD/M-TranslateIT/releases/latest/download/latest.json";

#[derive(Debug, Serialize)]
pub struct AppUpdateCheck {
    configured: bool,
    available: bool,
    current_version: String,
    version: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AppUpdateInstall {
    installed: bool,
    version: Option<String>,
    message: String,
}

fn updater_public_key() -> Option<&'static str> {
    option_env!("TRANSLATEIT_UPDATER_PUBLIC_KEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn update_blocker() -> Option<&'static str> {
    if crate::commands::voice_lab_recording::voice_lab_recording_blocks_app_exit() {
        return Some("Stop the current My Voice recording before updating TranslateIT.");
    }
    if crate::commands::voice_lab::current_voice_lab_build_snapshot().active {
        return Some("Wait for the current My Voice build to finish before updating TranslateIT.");
    }

    let runtime = crate::engine::runtime_state::latest_runtime_session_state();
    if runtime.has_active_session {
        return Some("Stop Meeting translation or Mic Test before updating TranslateIT.");
    }

    None
}

async fn check_update(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let Some(pubkey) = updater_public_key() else {
        return Ok(None);
    };

    let endpoint = UPDATE_ENDPOINT
        .parse()
        .map_err(|error| format!("invalid updater endpoint: {error}"))?;

    let updater = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint])
        .map_err(|error| format!("updater endpoint configuration failed: {error}"))?
        .build()
        .map_err(|error| format!("updater configuration failed: {error}"))?;

    updater
        .check()
        .await
        .map_err(|error| format!("update check failed: {error}"))
}

#[tauri::command]
pub async fn check_app_update_once(app: AppHandle) -> Result<AppUpdateCheck, String> {
    let current_version = app.package_info().version.to_string();

    if updater_public_key().is_none() {
        return Ok(AppUpdateCheck {
            configured: false,
            available: false,
            current_version,
            version: None,
            notes: None,
        });
    }

    let update = check_update(&app).await?;
    Ok(match update {
        Some(update) => AppUpdateCheck {
            configured: true,
            available: true,
            current_version,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
        },
        None => AppUpdateCheck {
            configured: true,
            available: false,
            current_version,
            version: None,
            notes: None,
        },
    })
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<AppUpdateInstall, String> {
    if updater_public_key().is_none() {
        return Ok(AppUpdateInstall {
            installed: false,
            version: None,
            message: "Automatic updates are not configured for this build.".to_string(),
        });
    }

    if let Some(blocker) = update_blocker() {
        return Ok(AppUpdateInstall {
            installed: false,
            version: None,
            message: blocker.to_string(),
        });
    }

    let Some(update) = check_update(&app).await? else {
        return Ok(AppUpdateInstall {
            installed: false,
            version: None,
            message: "TranslateIT is already up to date.".to_string(),
        });
    };

    let version = update.version.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("update installation failed: {error}"))?;

    Ok(AppUpdateInstall {
        installed: true,
        version: Some(version),
        message: "Update installation started.".to_string(),
    })
}

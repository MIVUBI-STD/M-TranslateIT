//! One on-demand Quick Access popup. Tray and frontend triggers share this
//! existing window instance; there is no second action dispatcher or audio owner.
use tauri::{AppHandle, Manager, PhysicalPosition};

const QUICK_ACCESS_LABEL: &str = "quick-access";

fn popup_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(QUICK_ACCESS_LABEL)
        .ok_or_else(|| "quick_access:window_unavailable".to_string())
}

fn position_popup(window: &tauri::WebviewWindow) -> Result<(), String> {
    // Put controls near the top-right of the monitor currently being used,
    // away from the floating caption's default bottom-center location.
    let cursor = window.cursor_position().ok();
    let monitor = cursor
        .and_then(|position| window.monitor_from_point(position.x, position.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return Ok(()) };
    let work = monitor.work_area();
    let size = window.outer_size().map_err(|_| "quick_access:window_size_unavailable")?;
    let margin = (14.0 * monitor.scale_factor()).round() as i64;
    let left = i64::from(work.position.x);
    let top = i64::from(work.position.y);
    let x = (left + i64::from(work.size.width) - i64::from(size.width) - margin)
        .max(left);
    let y = top + margin;
    window.set_position(PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )).map_err(|_| "quick_access:window_position_failed".to_string())
}

pub fn toggle_window(app: &AppHandle) -> Result<bool, String> {
    let window = popup_window(app)?;
    if window.is_visible().map_err(|_| "quick_access:visibility_unavailable")? {
        window.hide().map_err(|_| "quick_access:window_hide_failed")?;
        return Ok(false);
    }
    position_popup(&window)?;
    window.show().map_err(|_| "quick_access:window_show_failed")?;
    window.set_focus().map_err(|_| "quick_access:focus_failed")?;
    Ok(true)
}

#[tauri::command]
pub fn toggle_quick_access(app: AppHandle) -> Result<bool, String> {
    toggle_window(&app)
}

#[tauri::command]
pub fn open_translateit_main(app: AppHandle) -> Result<(), String> {
    let main = app.get_webview_window("main")
        .ok_or_else(|| "quick_access:main_window_unavailable".to_string())?;
    main.unminimize().map_err(|_| "quick_access:main_restore_failed")?;
    main.show().map_err(|_| "quick_access:main_show_failed")?;
    main.set_focus().map_err(|_| "quick_access:main_focus_failed")?;
    if let Some(quick) = app.get_webview_window(QUICK_ACCESS_LABEL) {
        let _ = quick.hide();
    }
    Ok(())
}

// Tray is only a *trigger*, not a parallel Quick Access menu or a separate
// source-switching route. Failure must not make TranslateIT fail to boot.
#[cfg(target_os = "windows")]
pub fn install_tray<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let icon = app.default_window_icon().cloned()
        .ok_or("quick_access:tray_icon_unavailable")?;
    let _tray = TrayIconBuilder::with_id("quick-access-tray")
        .icon(icon)
        .tooltip("TranslateIT Quick Access")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event {
                let _ = toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(target_os = "windows")]
use sysinfo::System;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MeetingAppDetection {
    pub supported: bool,
    pub detected: bool,
    pub provider: Option<String>,
    pub confidence: String,
    pub evidence: String,
    pub note: String,
    pub updated_unix_ms: u128,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(any(target_os = "windows", test))]
fn classify_window_title(title: &str) -> Option<&'static str> {
    let title = title.trim().to_ascii_lowercase();
    if title.contains("google meet") {
        Some("Google Meet")
    } else if title.contains("microsoft teams") {
        Some("Microsoft Teams")
    } else if title.contains("zoom meeting") || title.contains("zoom workplace") {
        Some("Zoom")
    } else if title.contains("webex") {
        Some("Webex")
    } else {
        None
    }
}

#[cfg(any(target_os = "windows", test))]
fn classify_process_name(name: &str) -> Option<&'static str> {
    let name = name.trim().to_ascii_lowercase();
    match name.as_str() {
        "ms-teams.exe" | "ms-teams" | "teams.exe" | "teams" => Some("Microsoft Teams"),
        "zoom.exe" | "zoom" => Some("Zoom"),
        "webex.exe" | "webex" | "ciscocollabhost.exe" | "ciscocollabhost" => Some("Webex"),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn visible_window_titles() -> Vec<String> {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    type Lparam = isize;
    type Bool = i32;
    type EnumProc = Option<unsafe extern "system" fn(Hwnd, Lparam) -> Bool>;

    #[link(name = "User32")]
    extern "system" {
        fn EnumWindows(callback: EnumProc, lparam: Lparam) -> Bool;
        fn IsWindowVisible(hwnd: Hwnd) -> Bool;
        fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, buffer: *mut u16, max_count: i32) -> i32;
    }

    unsafe extern "system" fn collect(hwnd: Hwnd, lparam: Lparam) -> Bool {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 || length > 512 {
            return 1;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if copied > 0 {
            let titles = unsafe { &mut *(lparam as *mut Vec<String>) };
            titles.push(String::from_utf16_lossy(&buffer[..copied as usize]));
        }
        1
    }

    let mut titles = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), &mut titles as *mut Vec<String> as Lparam);
    }
    titles
}


#[tauri::command]
pub fn detect_meeting_app() -> MeetingAppDetection {
    let updated_unix_ms = unix_ms();

    #[cfg(not(target_os = "windows"))]
    {
        return MeetingAppDetection {
            supported: false,
            detected: false,
            provider: None,
            confidence: "unsupported".to_string(),
            evidence: "platform".to_string(),
            note: "Meeting app detection is currently available on Windows.".to_string(),
            updated_unix_ms,
        };
    }

    #[cfg(target_os = "windows")]
    {
        for title in visible_window_titles() {
            if let Some(provider) = classify_window_title(&title) {
                return MeetingAppDetection {
                    supported: true,
                    detected: true,
                    provider: Some(provider.to_string()),
                    confidence: "high".to_string(),
                    evidence: "visible_window".to_string(),
                    note: format!("{provider} meeting window detected."),
                    updated_unix_ms,
                };
            }
        }

        let system = System::new_all();
        for process in system.processes().values() {
            let name = process.name().to_string_lossy();
            if let Some(provider) = classify_process_name(&name) {
                return MeetingAppDetection {
                    supported: true,
                    detected: true,
                    provider: Some(provider.to_string()),
                    confidence: "medium".to_string(),
                    evidence: "running_process".to_string(),
                    note: format!("{provider} is open. Start translation when your meeting is ready."),
                    updated_unix_ms,
                };
            }
        }

        MeetingAppDetection {
            supported: true,
            detected: false,
            provider: None,
            confidence: "none".to_string(),
            evidence: "scan_complete".to_string(),
            note: "No supported meeting app is currently detected.".to_string(),
            updated_unix_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_process_name, classify_window_title};

    #[test]
    fn classifies_supported_meeting_window_titles_without_browser_false_positive() {
        assert_eq!(classify_window_title("Weekly Sync - Google Meet"), Some("Google Meet"));
        assert_eq!(classify_window_title("Project | Microsoft Teams"), Some("Microsoft Teams"));
        assert_eq!(classify_window_title("Zoom Meeting"), Some("Zoom"));
        assert_eq!(classify_window_title("Docs - Google Chrome"), None);
        assert_eq!(classify_window_title("YouTube - Microsoft Edge"), None);
    }

    #[test]
    fn classifies_native_meeting_processes_only() {
        assert_eq!(classify_process_name("ms-teams.exe"), Some("Microsoft Teams"));
        assert_eq!(classify_process_name("Zoom.exe"), Some("Zoom"));
        assert_eq!(classify_process_name("CiscoCollabHost.exe"), Some("Webex"));
        assert_eq!(classify_process_name("chrome.exe"), None);
        assert_eq!(classify_process_name("msedge.exe"), None);
    }
}

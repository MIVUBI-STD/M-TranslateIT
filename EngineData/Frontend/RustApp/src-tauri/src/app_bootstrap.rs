use crate::engine::paths::{initialize_tauri_path_context, ProjectPaths};
use tauri::{LogicalSize, Manager, Runtime};

#[cfg(target_os = "windows")]
mod windows_power_lifecycle {
    use std::ffi::c_void;
    use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
    use std::sync::OnceLock;
    use std::thread;

    use tauri::{Runtime, WebviewWindow};

    use crate::engine::runtime_state::APPLICATION_MEETING_OWNER_ID;
    const WM_POWERBROADCAST: u32 = 0x0218;
    const PBT_APMSUSPEND: usize = 0x0004;
    const PBT_APMRESUMECRITICAL: usize = 0x0006;
    const PBT_APMRESUMESUSPEND: usize = 0x0007;
    const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
    const TRANSLATEIT_POWER_SUBCLASS_ID: usize = 0x5452_5057;

    type Hwnd = *mut c_void;
    type Lparam = isize;
    type Lresult = isize;
    type Wparam = usize;
    type SubclassProc =
        Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam, usize, usize) -> Lresult>;

    #[link(name = "Comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: Hwnd,
            subclass_proc: SubclassProc,
            subclass_id: usize,
            ref_data: usize,
        ) -> i32;
        fn DefSubclassProc(hwnd: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    }

    static POWER_CLEANUP_SENDER: OnceLock<SyncSender<()>> = OnceLock::new();

    fn application_meeting_owned() -> bool {
        crate::engine::runtime_state::latest_runtime_session_state()
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.owner_id == APPLICATION_MEETING_OWNER_ID)
            .unwrap_or(false)
    }

    fn invalidate_application_meeting_output_authority() {
        let report = crate::engine::runtime_state::latest_runtime_session_state();
        let Some(snapshot) = report.snapshot.as_ref() else {
            return;
        };
        if snapshot.owner_id != APPLICATION_MEETING_OWNER_ID {
            return;
        }

        let generation = snapshot.generation;
        if snapshot.authority_active {
            let revoked = crate::engine::runtime_state::revoke_runtime_session_authority(
                generation,
                "Windows power transition invalidated the active Meeting generation before suspend/resume cleanup.",
            );
            let authority_revoked = revoked
                .snapshot
                .as_ref()
                .map(|current| {
                    current.owner_id == APPLICATION_MEETING_OWNER_ID
                        && current.generation == generation
                        && !current.authority_active
                })
                .unwrap_or(false);
            if !authority_revoked {
                return;
            }
        }

        // This is intentionally only the immediate fail-closed output boundary.
        // Full capture/helper/consumer cleanup stays owned by canonical Meeting Stop
        // on the lifecycle worker below. The playback cancel is an atomic flag and
        // does not block the Windows power-broadcast callback.
        let _ = crate::engine::audio::meeting_output::cancel_meeting_output_for_generation(
            generation,
        );
    }

    fn converge_application_meeting_to_stopped() {
        if application_meeting_owned() {
            // Canonical Meeting Stop is idempotent after the synchronous power-event
            // revocation above. It owns provider/helper/consumer cancellation and all
            // audio-resource release; this lifecycle hook must not duplicate cleanup.
            let _ = crate::commands::meeting_session::stop_meeting_translation();
        }

        // A suspend/resume boundary invalidates diagnostic/setup functional proof even
        // when the helper process itself survives. The next readiness check must prove
        // the required outbound path again instead of reusing pre-suspend evidence.
        crate::commands::helper_bridge::invalidate_required_outbound_readiness_for_power_transition();
    }

    fn power_event_requires_cleanup(wparam: Wparam) -> bool {
        matches!(
            wparam,
            PBT_APMSUSPEND
                | PBT_APMRESUMECRITICAL
                | PBT_APMRESUMESUSPEND
                | PBT_APMRESUMEAUTOMATIC
        )
    }

    fn ensure_cleanup_handoff() -> Result<(), Box<dyn std::error::Error>> {
        if POWER_CLEANUP_SENDER.get().is_some() {
            return Ok(());
        }

        // One bounded signal may wait while the cleanup worker is already converging
        // a previous power event. This lets a resume notification request one
        // idempotent follow-up Stop without making the Windows callback wait.
        let (sender, receiver) = sync_channel::<()>(1);
        let cleanup_thread = thread::Builder::new()
            .name("translateit-power-cleanup".to_string())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    converge_application_meeting_to_stopped();
                }
            })?;

        if POWER_CLEANUP_SENDER.set(sender).is_err() {
            // Another installer won the once-only sender race. Dropping this detached
            // worker's only sender lets it exit without creating a second active owner.
            drop(cleanup_thread);
            return Ok(());
        }

        // Dropping a JoinHandle detaches the one process-lifetime lifecycle worker;
        // the static sender owns its useful lifetime until process exit.
        drop(cleanup_thread);
        Ok(())
    }

    fn request_meeting_cleanup() {
        let Some(sender) = POWER_CLEANUP_SENDER.get() else {
            return;
        };
        match sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {}
            Err(TrySendError::Disconnected(())) => {}
        }
    }

    unsafe extern "system" fn translateit_power_window_proc(
        hwnd: Hwnd,
        message: u32,
        wparam: Wparam,
        lparam: Lparam,
        _subclass_id: usize,
        _ref_data: usize,
    ) -> Lresult {
        if message == WM_POWERBROADCAST && power_event_requires_cleanup(wparam) {
            // Authority and any already-running translated playback are invalidated
            // before returning to Windows. Potentially blocking capture/helper/thread
            // cleanup is then handed off to canonical Meeting Stop.
            invalidate_application_meeting_output_authority();
            request_meeting_cleanup();
        }

        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    pub fn install<R: Runtime>(
        window: &WebviewWindow<R>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        ensure_cleanup_handoff()?;
        let hwnd = window.hwnd()?;
        let installed = unsafe {
            SetWindowSubclass(
                hwnd.0,
                Some(translateit_power_window_proc),
                TRANSLATEIT_POWER_SUBCLASS_ID,
                0,
            )
        };
        if installed == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "TranslateIT could not install the Windows power lifecycle hook.",
            )
            .into());
        }
        Ok(())
    }

    #[cfg(test)]
    mod b4_power_lifecycle_tests {
        use super::{
            power_event_requires_cleanup, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMECRITICAL,
            PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
        };

        #[test]
        fn b4_suspend_and_resume_events_request_cleanup_convergence() {
            assert!(power_event_requires_cleanup(PBT_APMSUSPEND));
            assert!(power_event_requires_cleanup(PBT_APMRESUMECRITICAL));
            assert!(power_event_requires_cleanup(PBT_APMRESUMESUSPEND));
            assert!(power_event_requires_cleanup(PBT_APMRESUMEAUTOMATIC));
            assert!(!power_event_requires_cleanup(0));
            assert!(!power_event_requires_cleanup(0xffff));
        }
    }
}

fn configure_runtime_paths<R: Runtime>(
    app: &tauri::App<R>,
) -> Result<ProjectPaths, Box<dyn std::error::Error>> {
    let current_paths = ProjectPaths::discover();
    let paths = if current_paths.is_repository_development() {
        // `tauri dev` and local repository execution keep the explicit repository
        // development layout. This fallback is intentionally distinct from installed
        // path proof.
        current_paths
    } else {
        let resource_dir = app.path().resource_dir()?;
        let app_local_data_dir = app.path().app_local_data_dir()?;
        initialize_tauri_path_context(resource_dir, app_local_data_dir)?
    };

    paths.ensure_user_data_dirs()?;

    // Child runtime processes consume these as transport values only. The values are
    // always overwritten from ProjectPaths so user environment state cannot become a
    // second installed-path authority.
    std::env::set_var("TRANSLATEIT_RUNTIME_ROOT", &paths.runtime_root);
    std::env::set_var("TRANSLATEIT_USER_DATA_ROOT", &paths.user_data_root);

    Ok(paths)
}

pub fn configure_main_window<R: Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = configure_runtime_paths(app)?;
    crate::commands::startup_recovery::begin_startup_recovery(&paths)?;

    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        windows_power_lifecycle::install(&window)?;

        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_size(LogicalSize::new(1600.0, 940.0));
        let _ = window.center();
        let _ = window.set_focus();
    }

    Ok(())
}

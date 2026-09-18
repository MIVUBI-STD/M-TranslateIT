use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const HELPER_SCHEDULER_TOTAL_ADMISSION_CAP: u32 = 8;
pub const HELPER_SCHEDULER_MEETING_OUTBOUND_WAIT_MS: u64 = 120_000;
pub const HELPER_SCHEDULER_MEETING_INCOMING_WAIT_MS: u64 = 30_000;
pub const HELPER_SCHEDULER_TEXT_WAIT_MS: u64 = 15_000;
pub const HELPER_SCHEDULER_DIAGNOSTIC_WAIT_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperTaskPriority {
    MeetingOutbound,
    MeetingIncoming,
    Text,
    Diagnostic,
}

impl HelperTaskPriority {
    pub fn label(self) -> &'static str {
        match self {
            Self::MeetingOutbound => "meeting_outbound",
            Self::MeetingIncoming => "meeting_incoming",
            Self::Text => "text",
            Self::Diagnostic => "diagnostic",
        }
    }

    fn scheduler_wait_deadline_ms(self) -> u64 {
        match self {
            Self::MeetingOutbound => HELPER_SCHEDULER_MEETING_OUTBOUND_WAIT_MS,
            Self::MeetingIncoming => HELPER_SCHEDULER_MEETING_INCOMING_WAIT_MS,
            Self::Text => HELPER_SCHEDULER_TEXT_WAIT_MS,
            Self::Diagnostic => HELPER_SCHEDULER_DIAGNOSTIC_WAIT_MS,
        }
    }

    fn scheduler_admission_limit(self) -> u32 {
        match self {
            Self::MeetingOutbound => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP,
            Self::MeetingIncoming => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(1),
            Self::Text => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(2),
            Self::Diagnostic => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(3),
        }
    }
}

#[derive(Debug, Default)]
struct HelperSchedulerState {
    active: bool,
    waiting_meeting_outbound: u32,
    waiting_meeting_incoming: u32,
    waiting_text: u32,
    waiting_diagnostic: u32,
    next_request_sequence: u64,
}

static HELPER_SCHEDULER: OnceLock<(Mutex<HelperSchedulerState>, Condvar)> = OnceLock::new();

fn scheduler() -> &'static (Mutex<HelperSchedulerState>, Condvar) {
    HELPER_SCHEDULER.get_or_init(|| (Mutex::new(HelperSchedulerState::default()), Condvar::new()))
}

pub struct HelperTaskPermit {
    request_id: String,
}

impl HelperTaskPermit {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl Drop for HelperTaskPermit {
    fn drop(&mut self) {
        let (lock, wake) = scheduler();
        if let Ok(mut state) = lock.lock() {
            state.active = false;
            wake.notify_all();
        }
    }
}

fn scheduler_waiting_increment(state: &mut HelperSchedulerState, priority: HelperTaskPriority) {
    match priority {
        HelperTaskPriority::MeetingOutbound => {
            state.waiting_meeting_outbound = state.waiting_meeting_outbound.saturating_add(1)
        }
        HelperTaskPriority::MeetingIncoming => {
            state.waiting_meeting_incoming = state.waiting_meeting_incoming.saturating_add(1)
        }
        HelperTaskPriority::Text => state.waiting_text = state.waiting_text.saturating_add(1),
        HelperTaskPriority::Diagnostic => {
            state.waiting_diagnostic = state.waiting_diagnostic.saturating_add(1)
        }
    }
}

fn scheduler_waiting_decrement(state: &mut HelperSchedulerState, priority: HelperTaskPriority) {
    match priority {
        HelperTaskPriority::MeetingOutbound => {
            state.waiting_meeting_outbound = state.waiting_meeting_outbound.saturating_sub(1)
        }
        HelperTaskPriority::MeetingIncoming => {
            state.waiting_meeting_incoming = state.waiting_meeting_incoming.saturating_sub(1)
        }
        HelperTaskPriority::Text => state.waiting_text = state.waiting_text.saturating_sub(1),
        HelperTaskPriority::Diagnostic => {
            state.waiting_diagnostic = state.waiting_diagnostic.saturating_sub(1)
        }
    }
}

fn scheduler_total_admitted(state: &HelperSchedulerState) -> u32 {
    let active = if state.active { 1_u32 } else { 0_u32 };
    active
        .saturating_add(state.waiting_meeting_outbound)
        .saturating_add(state.waiting_meeting_incoming)
        .saturating_add(state.waiting_text)
        .saturating_add(state.waiting_diagnostic)
}

fn scheduler_can_admit(state: &HelperSchedulerState, priority: HelperTaskPriority) -> bool {
    scheduler_total_admitted(state) < priority.scheduler_admission_limit()
}

fn scheduler_wait_deadline_error(priority: HelperTaskPriority, wait_deadline: Duration) -> String {
    format!(
        "helper_scheduler:wait_deadline_exceeded:{}:{}ms",
        priority.label(),
        wait_deadline.as_millis()
    )
}

fn scheduler_can_enter(state: &HelperSchedulerState, priority: HelperTaskPriority) -> bool {
    if state.active {
        return false;
    }
    match priority {
        HelperTaskPriority::MeetingOutbound => true,
        HelperTaskPriority::MeetingIncoming => state.waiting_meeting_outbound == 0,
        HelperTaskPriority::Text => {
            state.waiting_meeting_outbound == 0 && state.waiting_meeting_incoming == 0
        }
        HelperTaskPriority::Diagnostic => {
            state.waiting_meeting_outbound == 0
                && state.waiting_meeting_incoming == 0
                && state.waiting_text == 0
        }
    }
}

fn acquire_helper_task_permit_with_wait_deadline(
    priority: HelperTaskPriority,
    wait_deadline: Duration,
) -> Result<HelperTaskPermit, String> {
    let (lock, wake) = scheduler();
    let mut state = lock
        .lock()
        .map_err(|_| "helper_scheduler:lock_poisoned".to_string())?;

    if !scheduler_can_admit(&state, priority) {
        return Err(format!(
            "helper_scheduler:admission_capacity_exceeded:{}:total={}:limit={}",
            priority.label(),
            scheduler_total_admitted(&state),
            priority.scheduler_admission_limit()
        ));
    }

    scheduler_waiting_increment(&mut state, priority);
    let wait_started = Instant::now();

    while !scheduler_can_enter(&state, priority) {
        let remaining = wait_deadline.saturating_sub(wait_started.elapsed());
        if remaining.is_zero() {
            scheduler_waiting_decrement(&mut state, priority);
            wake.notify_all();
            return Err(scheduler_wait_deadline_error(priority, wait_deadline));
        }

        let (next_state, wait_result) = match wake.wait_timeout(state, remaining) {
            Ok(value) => value,
            Err(poisoned) => {
                let (mut poisoned_state, _) = poisoned.into_inner();
                scheduler_waiting_decrement(&mut poisoned_state, priority);
                wake.notify_all();
                return Err("helper_scheduler:wait_lock_poisoned".to_string());
            }
        };
        state = next_state;

        if wait_result.timed_out() && !scheduler_can_enter(&state, priority) {
            scheduler_waiting_decrement(&mut state, priority);
            wake.notify_all();
            return Err(scheduler_wait_deadline_error(priority, wait_deadline));
        }
    }

    scheduler_waiting_decrement(&mut state, priority);
    state.active = true;
    state.next_request_sequence = state.next_request_sequence.saturating_add(1);
    Ok(HelperTaskPermit {
        request_id: format!("helper-{}", state.next_request_sequence),
    })
}

pub fn acquire_helper_task_permit(
    priority: HelperTaskPriority,
) -> Result<HelperTaskPermit, String> {
    acquire_helper_task_permit_with_wait_deadline(
        priority,
        Duration::from_millis(priority.scheduler_wait_deadline_ms()),
    )
}

#[cfg(test)]
mod scheduler_policy_tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    static SCHEDULER_TEST_SERIAL: OnceLock<Mutex<()>> = OnceLock::new();

    fn scheduler_test_guard() -> std::sync::MutexGuard<'static, ()> {
        SCHEDULER_TEST_SERIAL
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reset_scheduler() {
        let (lock, wake) = scheduler();
        let mut state = lock.lock().expect("scheduler test lock");
        *state = HelperSchedulerState::default();
        wake.notify_all();
    }

    #[test]
    fn scheduler_policy_preserves_priority_and_reserved_admission_headroom() {
        let _serial = scheduler_test_guard();
        assert!(
            HelperTaskPriority::MeetingOutbound.scheduler_wait_deadline_ms()
                > HelperTaskPriority::MeetingIncoming.scheduler_wait_deadline_ms()
        );
        assert!(
            HelperTaskPriority::MeetingIncoming.scheduler_wait_deadline_ms()
                > HelperTaskPriority::Text.scheduler_wait_deadline_ms()
        );
        assert!(
            HelperTaskPriority::Text.scheduler_wait_deadline_ms()
                > HelperTaskPriority::Diagnostic.scheduler_wait_deadline_ms()
        );

        let mut state = HelperSchedulerState::default();
        state.waiting_meeting_outbound = 1;
        assert!(scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingOutbound
        ));
        assert!(!scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state.waiting_meeting_outbound = 0;
        state.waiting_meeting_incoming = 1;
        assert!(scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state.waiting_meeting_incoming = 0;
        state.waiting_text = 1;
        assert!(scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state = HelperSchedulerState::default();
        state.active = true;
        state.waiting_diagnostic = HelperTaskPriority::Diagnostic
            .scheduler_admission_limit()
            .saturating_sub(1);
        assert_eq!(
            scheduler_total_admitted(&state),
            HelperTaskPriority::Diagnostic.scheduler_admission_limit()
        );
        assert!(!scheduler_can_admit(&state, HelperTaskPriority::Diagnostic));
        assert!(scheduler_can_admit(&state, HelperTaskPriority::Text));
        assert!(scheduler_can_admit(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(scheduler_can_admit(
            &state,
            HelperTaskPriority::MeetingOutbound
        ));
        assert_eq!(
            HelperTaskPriority::MeetingOutbound.scheduler_admission_limit(),
            HELPER_SCHEDULER_TOTAL_ADMISSION_CAP
        );
    }

    #[test]
    fn scheduler_capacity_rejection_does_not_add_waiting_callers() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        {
            let (lock, _) = scheduler();
            let mut state = lock.lock().expect("scheduler test lock");
            state.active = true;
            state.waiting_diagnostic = HelperTaskPriority::Diagnostic
                .scheduler_admission_limit()
                .saturating_sub(1);
        }

        let error = match acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::Diagnostic,
            Duration::from_millis(1),
        ) {
            Ok(_) => panic!("diagnostic request should be rejected at its admission limit"),
            Err(error) => error,
        };
        assert!(error.starts_with("helper_scheduler:admission_capacity_exceeded:diagnostic:"));

        let (lock, _) = scheduler();
        let state = lock.lock().expect("scheduler test lock");
        assert_eq!(
            scheduler_total_admitted(&state),
            HelperTaskPriority::Diagnostic.scheduler_admission_limit()
        );
        drop(state);
        reset_scheduler();
    }

    #[test]
    fn scheduler_wait_deadline_cleans_counter_and_releases_lower_priority_blocking() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let active = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::MeetingOutbound,
            Duration::from_millis(50),
        )
        .expect("outbound scheduler permit");

        let waiter = thread::spawn(|| {
            acquire_helper_task_permit_with_wait_deadline(
                HelperTaskPriority::Text,
                Duration::from_millis(25),
            )
        });
        let error = match waiter.join().expect("scheduler waiter thread") {
            Ok(_) => panic!("text request should time out while worker remains active"),
            Err(error) => error,
        };
        assert_eq!(error, "helper_scheduler:wait_deadline_exceeded:text:25ms");

        {
            let (lock, _) = scheduler();
            let state = lock.lock().expect("scheduler test lock");
            assert!(state.active);
            assert_eq!(state.waiting_text, 0);
            assert_eq!(state.waiting_meeting_outbound, 0);
            assert_eq!(state.waiting_meeting_incoming, 0);
            assert_eq!(state.waiting_diagnostic, 0);
        }

        drop(active);
        {
            let (lock, _) = scheduler();
            let state = lock.lock().expect("scheduler test lock");
            assert!(!state.active);
        }
        reset_scheduler();
    }

    #[test]
    fn scheduler_waiters_enter_in_priority_order_after_active_permit_releases() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let active = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::Diagnostic,
            Duration::from_millis(100),
        )
        .expect("seed active scheduler permit");

        let (entered_tx, entered_rx) = mpsc::channel::<&'static str>();
        let mut handles = Vec::new();
        for (priority, label) in [
            (HelperTaskPriority::Diagnostic, "diagnostic"),
            (HelperTaskPriority::Text, "text"),
            (HelperTaskPriority::MeetingIncoming, "meeting_incoming"),
            (HelperTaskPriority::MeetingOutbound, "meeting_outbound"),
        ] {
            let tx = entered_tx.clone();
            handles.push(thread::spawn(move || {
                let permit = acquire_helper_task_permit_with_wait_deadline(
                    priority,
                    Duration::from_secs(2),
                )
                .expect("queued scheduler permit");
                tx.send(label).expect("record scheduler entry");
                drop(permit);
            }));
        }
        drop(entered_tx);

        let registration_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let all_registered = {
                let (lock, _) = scheduler();
                let state = lock.lock().expect("scheduler test lock");
                state.waiting_meeting_outbound == 1
                    && state.waiting_meeting_incoming == 1
                    && state.waiting_text == 1
                    && state.waiting_diagnostic == 1
            };
            if all_registered {
                break;
            }
            assert!(
                Instant::now() < registration_deadline,
                "all scheduler waiters must register before releasing the active permit"
            );
            thread::sleep(Duration::from_millis(2));
        }

        drop(active);
        let entered: Vec<&'static str> = (0..4)
            .map(|_| {
                entered_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("scheduler entry order")
            })
            .collect();
        assert_eq!(
            entered,
            vec![
                "meeting_outbound",
                "meeting_incoming",
                "text",
                "diagnostic"
            ]
        );

        for handle in handles {
            handle.join().expect("scheduler waiter thread");
        }
        reset_scheduler();
    }
}

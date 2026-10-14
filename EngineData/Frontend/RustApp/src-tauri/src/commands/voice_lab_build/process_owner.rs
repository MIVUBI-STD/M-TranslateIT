//! Windows Job Object lifetime for the top-level My Voice build worker.
 //! Kill-on-close prevents orphaned Python stage trees after desktop exit.
 //! Assignment follows spawn, so the brief initial race is not eliminated.
use std::process::Child;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
#[repr(C)]
struct BasicLimit {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}
#[cfg(windows)]
#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}
#[cfg(windows)]
#[repr(C)]
struct ExtendedLimit {
    basic_limit_information: BasicLimit,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[cfg(windows)]
#[repr(C)]
struct JobAccounting {
    total_user_time: i64,
    total_kernel_time: i64,
    this_period_total_user_time: i64,
    this_period_total_kernel_time: i64,
    total_page_fault_count: u32,
    total_processes: u32,
    active_processes: u32,
    total_terminated_processes: u32,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(security: *mut c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, kind: i32, info: *const c_void, length: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> *mut c_void;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn TerminateJobObject(job: *mut c_void, exit_code: u32) -> i32;
    fn QueryInformationJobObject(
        job: *mut c_void, info_kind: i32, info: *mut c_void,
        length: u32, returned_length: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_pid: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[cfg(windows)]
#[derive(Debug)]
pub(super) struct OwnedBuildJob {
    handle: isize,
}

#[cfg(windows)]
impl OwnedBuildJob {
    pub(super) fn new() -> Result<Self, String> {
        // SAFETY: OS call returns a kernel HANDLE owned by this Rust object.
        let raw = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
        if raw.is_null() {
            return Err("my_voice:build_job_create_failed".into());
        }
        let job = Self { handle: raw as isize };
        // SAFETY: fully initialized C-compatible structure and matching size.
        let mut limit: ExtendedLimit = unsafe { std::mem::zeroed() };
        limit.basic_limit_information.limit_flags = 0x0000_2000;
        let success = unsafe {
            SetInformationJobObject(
                job.ptr(), 9, &limit as *const _ as *const c_void,
                std::mem::size_of::<ExtendedLimit>() as u32,
            )
        };
        if success == 0 {
            return Err("my_voice:build_job_configure_failed".into());
        }
        Ok(job)
    }

    fn ptr(&self) -> *mut c_void {
        self.handle as *mut c_void
    }

    pub(super) fn assign(&self, child: &Child) -> Result<(), String> {
        // SAFETY: both HANDLEs refer to live process and Job objects. Windows
        // may reject nested or restricted Job membership; fail closed.
        let assigned = unsafe {
            AssignProcessToJobObject(self.ptr(), child.as_raw_handle() as *mut c_void)
        };
        if assigned == 0 {
            Err("my_voice:build_job_assignment_failed".into())
        } else {
            Ok(())
        }
    }

    pub(super) fn resume_suspended(&self, pid: u32) -> Result<(), String> {
        // CREATE_SUSPENDED ensures user code cannot run before assignment.
        // Thread handles are not exposed by std::process::Child, so find the
        // one initial thread of this newly suspended owned process.
        let snapshot = unsafe { CreateToolhelp32Snapshot(0x0000_0004, 0) };
        if snapshot.is_null() || snapshot as isize == -1 {
            return Err("my_voice:build_thread_snapshot_failed".into());
        }
        let mut entry = ThreadEntry32 {
            size: std::mem::size_of::<ThreadEntry32>() as u32,
            usage: 0, thread_id: 0, owner_pid: 0, base_priority: 0,
            delta_priority: 0, flags: 0,
        };
        let mut matched: Vec<u32> = Vec::new();
        let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while has_entry {
            if entry.owner_pid == pid {
                matched.push(entry.thread_id);
            }
            has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot); }
        if matched.len() != 1 {
            return Err("my_voice:build_suspended_thread_ambiguous".into());
        }
        let thread = unsafe { OpenThread(0x0002, 0, matched[0]) };
        if thread.is_null() {
            return Err("my_voice:build_thread_open_failed".into());
        }
        let prior = unsafe { ResumeThread(thread) };
        unsafe { CloseHandle(thread); }
        if prior != 1 {
            return Err("my_voice:build_thread_resume_failed".into());
        }
        Ok(())
    }

    fn active_processes(&self) -> Result<u32, String> {
        // JobObjectBasicAccountingInformation, documented class 1. Verify
        // the owned tree, never scan or terminate unrelated user processes.
        let mut record: JobAccounting = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            QueryInformationJobObject(
                self.ptr(), 1, &mut record as *mut _ as *mut c_void,
                std::mem::size_of::<JobAccounting>() as u32,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err("my_voice:build_job_accounting_unavailable".into());
        }
        Ok(record.active_processes)
    }

    pub(super) fn terminate_and_verify_quiescence(&self) -> Result<(), String> {
        // The Python coordinator may crash while a CUDA training child is
        // still running. Do NOT copy multi-GB checkpoints until *all* owned
        // descendants are gone; otherwise an archive may race a writer.
        if self.active_processes()? == 0 {
            return Ok(());
        }
        if !self.terminate() {
            return Err("my_voice:build_job_terminate_unconfirmed".into());
        }
        for _ in 0..10 {
            if self.active_processes()? == 0 {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("my_voice:build_job_descendants_not_stopped".into())
    }

    pub(super) fn terminate(&self) -> bool {
        // SAFETY: terminates only processes assigned to this Job handle.
        unsafe { TerminateJobObject(self.ptr(), 1) != 0 }
    }
}
#[cfg(windows)]
impl Drop for OwnedBuildJob {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE applies when this final handle is closed.
        unsafe { CloseHandle(self.ptr()); }
    }
}

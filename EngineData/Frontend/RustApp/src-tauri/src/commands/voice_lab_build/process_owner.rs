//! Windows Job Object lifetime for the top-level My Voice build worker.
 //! Kill-on-close prevents orphaned Python stage trees after desktop exit.
 //! Assignment follows spawn, so the brief initial race is not eliminated.
use std::process::Child;

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
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(security: *mut c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, kind: i32, info: *const c_void, length: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn TerminateJobObject(job: *mut c_void, exit_code: u32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
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

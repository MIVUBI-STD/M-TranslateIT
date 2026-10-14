//! My Voice native Windows RAM/disk admission and checkpoint-copy headroom.
 //! One scoped resource policy; no external service or user-data cleanup.
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

const GIB: u64 = 1024 * 1024 * 1024;
const CACHE_MIN_FREE: u64 = 12 * GIB;
const SAVED_MIN_FREE: u64 = 8 * GIB;
const RAM_MIN_FREE: u64 = 3 * GIB;
const RECOVERY_MARGIN: u64 = 2 * GIB;

#[cfg(windows)]
#[repr(C)]
struct MemoryStatus {
    length: u32,
    load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page_file: u64,
    avail_page_file: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended_virtual: u64,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        available_to_caller: *mut u64,
        total_bytes: *mut u64,
        total_free_bytes: *mut u64,
    ) -> i32;
}

fn existing_directory(path: &Path) -> Result<PathBuf, String> {
    let mut current = path;
    loop {
        if current.is_dir() {
            let meta = std::fs::symlink_metadata(current)
                .map_err(|_| "my_voice:resource_directory_metadata_unavailable")?;
            if meta.file_type().is_symlink() {
                return Err("my_voice:resource_directory_link_untrusted".into());
            }
            return Ok(current.to_path_buf());
        }
        current = current.parent().ok_or("my_voice:resource_volume_unknown")?;
    }
}

#[cfg(windows)]
pub(crate) fn available_disk_bytes(path: &Path) -> Result<u64, String> {
    let directory = existing_directory(path)?;
    let wide: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    let status = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(), &mut available, std::ptr::null_mut(), std::ptr::null_mut(),
        )
    };
    if status == 0 { return Err("my_voice:disk_telemetry_unavailable".into()); }
    Ok(available)
}

#[cfg(not(windows))]
pub(crate) fn available_disk_bytes(_path: &Path) -> Result<u64, String> {
    Err("my_voice:resource_admission_windows_only".into())
}

#[cfg(windows)]
pub(crate) fn available_ram_bytes() -> Result<u64, String> {
    let mut value = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        load: 0, total_phys: 0, avail_phys: 0, total_page_file: 0,
        avail_page_file: 0, total_virtual: 0, avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut value) };
    if ok == 0 || value.total_phys == 0 || value.avail_phys > value.total_phys {
        return Err("my_voice:ram_telemetry_unavailable".into());
    }
    Ok(value.avail_phys)
}

#[cfg(not(windows))]
pub(crate) fn available_ram_bytes() -> Result<u64, String> {
    Err("my_voice:resource_admission_windows_only".into())
}

pub(crate) fn preflight(cache: &Path, saved: &Path) -> Result<(), String> {
    if available_ram_bytes()? < RAM_MIN_FREE {
        return Err("my_voice:insufficient_system_ram".into());
    }
    if available_disk_bytes(cache)? < CACHE_MIN_FREE {
        return Err("my_voice:insufficient_temporary_disk_space".into());
    }
    if available_disk_bytes(saved)? < SAVED_MIN_FREE {
        return Err("my_voice:insufficient_saved_disk_space".into());
    }
    Ok(())
}

/// Check the *actual* amount being archived plus margin, not a guessed
/// checkpoint size. Re-check immediately before allocating the archive copy.
pub(crate) fn ensure_archive_capacity(saved: &Path, copy_bytes: u64) -> Result<(), String> {
    let need = copy_bytes.checked_add(RECOVERY_MARGIN)
        .ok_or("my_voice:checkpoint_size_overflow")?;
    if available_disk_bytes(saved)? < need {
        return Err("my_voice:checkpoint_archive_space_insufficient".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_space_overflow_is_rejected() {
        assert!(ensure_archive_capacity(Path::new("."), u64::MAX).is_err());
    }
}

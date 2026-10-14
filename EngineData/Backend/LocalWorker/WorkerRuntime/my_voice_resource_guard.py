"""Bounded, read-only runtime RAM and storage checks for one My Voice run.

Do not delete files, checkpoint evidence or any user data on pressure.
All thresholds are provisional and must be native-validated before release.
"""
from __future__ import annotations

import ctypes
import os
import shutil
from pathlib import Path
from my_voice_adaptive_safety import CRITICAL_RAM_MIB, CRITICAL_DISK_MIB

MIB = 1024 * 1024
START_RAM_MIB = 3072
START_CACHE_MIB = 12 * 1024
START_SAVED_MIB = 8 * 1024

class ResourceGuardError(RuntimeError):
    pass

if os.name == "nt":
    class _MemoryStatus(ctypes.Structure):
        _fields_ = [
            ("dwLength", ctypes.c_uint32),
            ("dwMemoryLoad", ctypes.c_uint32),
            ("ullTotalPhys", ctypes.c_uint64),
            ("ullAvailPhys", ctypes.c_uint64),
            ("ullTotalPageFile", ctypes.c_uint64),
            ("ullAvailPageFile", ctypes.c_uint64),
            ("ullTotalVirtual", ctypes.c_uint64),
            ("ullAvailVirtual", ctypes.c_uint64),
            ("ullAvailExtendedVirtual", ctypes.c_uint64),
        ]
    _kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    _kernel.GlobalMemoryStatusEx.argtypes = [ctypes.POINTER(_MemoryStatus)]
    _kernel.GlobalMemoryStatusEx.restype = ctypes.c_int

def ram_available_mib() -> int:
    if os.name == "nt":
        memory = _MemoryStatus()
        memory.dwLength = ctypes.sizeof(_MemoryStatus)
        if not _kernel.GlobalMemoryStatusEx(ctypes.byref(memory)):
            raise ResourceGuardError("my_voice:ram_read_unavailable")
        total, available = int(memory.ullTotalPhys), int(memory.ullAvailPhys)
        if not total or not 0 <= available <= total:
            raise ResourceGuardError("my_voice:ram_read_invalid")
        return available // MIB
    try:
        content = Path("/proc/meminfo").read_text(encoding="ascii")
        line = next(line for line in content.splitlines() if line.startswith("MemAvailable:"))
        value = int(line.split()[1])
        if value <= 0:
            raise ValueError("empty")
        return value // 1024
    except (OSError, ValueError, StopIteration, IndexError) as exc:
        raise ResourceGuardError("my_voice:ram_read_unavailable") from exc

def _existing_volume(path_value: str) -> Path:
    if not path_value:
        raise ResourceGuardError("my_voice:resource_root_missing")
    original = Path(path_value)
    if not original.is_absolute():
        raise ResourceGuardError("my_voice:resource_root_not_absolute")
    current = original
    while not current.is_dir():
        if current.is_symlink():
            raise ResourceGuardError("my_voice:resource_root_symlink")
        if current.parent == current:
            raise ResourceGuardError("my_voice:resource_root_unavailable")
        current = current.parent
    if current.is_symlink():
        raise ResourceGuardError("my_voice:resource_root_symlink")
    return current

def disk_free_mib(path_value: str) -> int:
    try:
        disk = shutil.disk_usage(_existing_volume(path_value))
    except OSError as exc:
        raise ResourceGuardError("my_voice:disk_read_unavailable") from exc
    if disk.total <= 0 or disk.free < 0 or disk.free > disk.total:
        raise ResourceGuardError("my_voice:disk_read_invalid")
    return disk.free // MIB

def snapshot() -> tuple[int, int, int]:
    cache_root = os.environ.get("TRANSLATEIT_MYVOICE_CACHE_ROOT", "")
    saved_root = os.environ.get("TRANSLATEIT_MYVOICE_SAVED_ROOT", "")
    return (
        ram_available_mib(),
        disk_free_mib(cache_root),
        disk_free_mib(saved_root),
    )

def check_start() -> None:
    ram, cache, saved = snapshot()
    if ram < START_RAM_MIB:
        raise ResourceGuardError("my_voice:low_ram_before_stage")
    if cache < START_CACHE_MIB:
        raise ResourceGuardError("my_voice:low_temporary_disk_before_stage")
    if saved < START_SAVED_MIB:
        raise ResourceGuardError("my_voice:low_saved_disk_before_stage")

def critical() -> str | None:
    ram, cache, saved = snapshot()
    if ram < CRITICAL_RAM_MIB:
        return "my_voice:critical_ram"
    if cache < CRITICAL_DISK_MIB or saved < CRITICAL_DISK_MIB:
        return "my_voice:critical_disk"
    return None

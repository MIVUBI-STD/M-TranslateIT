"""Conservative per-stage process-tree supervisor, not a GPU safety guarantee.

Windows Job Object KILL_ON_JOB_CLOSE covers descendants after assignment.
Windows children start CREATE_SUSPENDED and only resume after Job admission.
A native worker/driver crash can still happen after resume.
GPU pressure observations use optional installed nvidia-smi and fail closed.
"""
from __future__ import annotations

import ctypes
import os
import shutil
import signal
import subprocess
from pathlib import Path
from typing import Any

SENSOR_INTERVAL_SECONDS = 5
SENSOR_TIMEOUT_SECONDS = 4
CRITICAL_FREE_MIB = 768
MAX_BAD_SAMPLES = 2

class StageGuardError(RuntimeError):
    pass

if os.name == "nt":
    from ctypes import wintypes

    class _BasicLimit(ctypes.Structure):
        _fields_ = [
            ("PerProcessUserTimeLimit", ctypes.c_int64),
            ("PerJobUserTimeLimit", ctypes.c_int64),
            ("LimitFlags", wintypes.DWORD),
            ("MinimumWorkingSetSize", ctypes.c_size_t),
            ("MaximumWorkingSetSize", ctypes.c_size_t),
            ("ActiveProcessLimit", wintypes.DWORD),
            ("Affinity", ctypes.c_size_t),
            ("PriorityClass", wintypes.DWORD),
            ("SchedulingClass", wintypes.DWORD),
        ]

    class _IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_uint64) for name in (
            "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
            "ReadTransferCount", "WriteTransferCount", "OtherTransferCount"
        )]

    class _ExtendedLimit(ctypes.Structure):
        _fields_ = [
            ("BasicLimitInformation", _BasicLimit),
            ("IoInfo", _IoCounters),
            ("ProcessMemoryLimit", ctypes.c_size_t),
            ("JobMemoryLimit", ctypes.c_size_t),
            ("PeakProcessMemoryUsed", ctypes.c_size_t),
            ("PeakJobMemoryUsed", ctypes.c_size_t),
        ]

    class _Accounting(ctypes.Structure):
        _fields_ = [
            ("TotalUserTime", ctypes.c_int64),
            ("TotalKernelTime", ctypes.c_int64),
            ("ThisPeriodTotalUserTime", ctypes.c_int64),
            ("ThisPeriodTotalKernelTime", ctypes.c_int64),
            ("TotalPageFaultCount", wintypes.DWORD),
            ("TotalProcesses", wintypes.DWORD),
            ("ActiveProcesses", wintypes.DWORD),
            ("TotalTerminatedProcesses", wintypes.DWORD),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
    kernel.CreateJobObjectW.restype = ctypes.c_void_p
    kernel.SetInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel.SetInformationJobObject.restype = wintypes.BOOL
    kernel.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
    kernel.Thread32First.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.Thread32First.restype = wintypes.BOOL
    kernel.Thread32Next.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    kernel.Thread32Next.restype = wintypes.BOOL
    kernel.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenThread.restype = ctypes.c_void_p
    kernel.ResumeThread.argtypes = [ctypes.c_void_p]
    kernel.ResumeThread.restype = wintypes.DWORD
    kernel.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel.QueryInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
    kernel.QueryInformationJobObject.restype = wintypes.BOOL
    kernel.TerminateJobObject.argtypes = [ctypes.c_void_p, wintypes.UINT]
    kernel.TerminateJobObject.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel.CloseHandle.restype = wintypes.BOOL

    class _ThreadEntry32(ctypes.Structure):
        _fields_ = [
            ("dwSize", wintypes.DWORD),
            ("cntUsage", wintypes.DWORD),
            ("th32ThreadID", wintypes.DWORD),
            ("th32OwnerProcessID", wintypes.DWORD),
            ("tpBasePri", wintypes.LONG),
            ("tpDeltaPri", wintypes.LONG),
            ("dwFlags", wintypes.DWORD),
        ]

    def _resume_owned_suspended_process(pid: int) -> None:
        # Popen closes its original main-thread handle, so locate exactly
        # one suspended initial thread through the Windows Toolhelp API.
        snap = kernel.CreateToolhelp32Snapshot(0x00000004, 0)
        if not snap or snap == ctypes.c_void_p(-1).value:
            raise StageGuardError("my_voice:suspended_snapshot_unavailable")
        matches: list[int] = []
        entry = _ThreadEntry32()
        entry.dwSize = ctypes.sizeof(_ThreadEntry32)
        try:
            success = kernel.Thread32First(snap, ctypes.byref(entry))
            while success:
                if entry.th32OwnerProcessID == pid:
                    matches.append(int(entry.th32ThreadID))
                success = kernel.Thread32Next(snap, ctypes.byref(entry))
        finally:
            kernel.CloseHandle(snap)
        if len(matches) != 1:
            raise StageGuardError("my_voice:suspended_thread_ambiguous")
        thread = kernel.OpenThread(0x0002, False, matches[0])
        if not thread:
            raise StageGuardError("my_voice:suspended_thread_unavailable")
        try:
            prior = kernel.ResumeThread(thread)
        finally:
            kernel.CloseHandle(thread)
        if prior != 1:
            raise StageGuardError("my_voice:suspended_thread_resume_failed")

class OwnedStageTree:
    def __init__(self) -> None:
        self.handle: int | None = None
        self.process: subprocess.Popen[Any] | None = None
        if os.name == "nt":
            handle = kernel.CreateJobObjectW(None, None)
            if not handle:
                raise StageGuardError("my_voice:job_create_failed")
            self.handle = int(handle)
            info = _ExtendedLimit()
            info.BasicLimitInformation.LimitFlags = 0x2000
            if not kernel.SetInformationJobObject(self.handle, 9, ctypes.byref(info), ctypes.sizeof(info)):
                self.close()
                raise StageGuardError("my_voice:job_limit_failed")

    def attach(self, proc: subprocess.Popen[Any]) -> None:
        self.process = proc
        if os.name == "nt":
            handle = getattr(proc, "_handle", None)
            if not handle or not self.handle or not kernel.AssignProcessToJobObject(self.handle, int(handle)):
                self.terminate()
                raise StageGuardError("my_voice:job_assign_failed")

    def remaining(self) -> int:
        if os.name == "nt":
            info = _Accounting()
            if not self.handle or not kernel.QueryInformationJobObject(
                self.handle, 1, ctypes.byref(info), ctypes.sizeof(info), None
            ):
                raise StageGuardError("my_voice:job_accounting_unavailable")
            return int(info.ActiveProcesses)
        return int(self.process is not None and self.process.poll() is None)

    def terminate(self) -> None:
        if os.name == "nt" and self.handle:
            if not kernel.TerminateJobObject(self.handle, 1):
                raise StageGuardError("my_voice:job_terminate_failed")
        elif self.process is not None and self.process.poll() is None:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def close(self) -> None:
        if os.name == "nt" and self.handle:
            kernel.CloseHandle(self.handle)
            self.handle = None

    def __enter__(self) -> "OwnedStageTree":
        return self

    def __exit__(self, _kind: Any, _exc: Any, _tb: Any) -> None:
        try:
            if self.process is not None and self.process.poll() is None:
                self.terminate()
                try:
                    self.process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    pass
        finally:
            self.close()

def locate_sensor() -> str:
    found = shutil.which("nvidia-smi")
    if found:
        return found
    if os.name == "nt":
        candidate = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "NVIDIA Corporation/NVSMI/nvidia-smi.exe"
        if candidate.is_file():
            return str(candidate)
    raise StageGuardError("my_voice:gpu_sensor_unavailable")

def gpu_free_mib(sensor: str) -> int:
    try:
        result = subprocess.run(
            [sensor, "--id=0", "--query-gpu=memory.free", "--format=csv,noheader,nounits"],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, check=False, timeout=SENSOR_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise StageGuardError("my_voice:gpu_sensor_timeout") from exc
    lines = result.stdout.strip().splitlines()
    if result.returncode != 0 or len(lines) != 1 or not lines[0].strip().isdecimal():
        raise StageGuardError("my_voice:gpu_sensor_unreadable")
    value = int(lines[0].strip())
    if value < 0 or value > 1048576:
        raise StageGuardError("my_voice:gpu_sensor_invalid")
    return value

def run_supervised_stage(
    argv: list[str], *, cwd: str, env: dict[str, str], gpu_stage: bool
) -> None:
    sensor = locate_sensor() if gpu_stage else None
    if sensor and gpu_free_mib(sensor) < CRITICAL_FREE_MIB:
        raise StageGuardError("my_voice:gpu_pressure_before_stage")
    with OwnedStageTree() as tree:
        try:
            proc = subprocess.Popen(
                argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                creationflags=(subprocess.CREATE_NO_WINDOW | 0x00000004) if os.name == "nt" else 0,
                start_new_session=os.name != "nt",
            )
        except OSError as exc:
            raise StageGuardError("my_voice:stage_spawn_failed") from exc
        tree.attach(proc)
        if os.name == "nt":
            _resume_owned_suspended_process(proc.pid)
        consecutive_bad = 0
        while proc.poll() is None:
            if sensor:
                try:
                    free = gpu_free_mib(sensor)
                    consecutive_bad = consecutive_bad + 1 if free < CRITICAL_FREE_MIB else 0
                except StageGuardError:
                    consecutive_bad += 1
                if consecutive_bad >= MAX_BAD_SAMPLES:
                    tree.terminate()
                    proc.wait(timeout=8)
                    raise StageGuardError("my_voice:gpu_pressure_or_telemetry_lost")
            try:
                proc.wait(timeout=SENSOR_INTERVAL_SECONDS)
            except subprocess.TimeoutExpired:
                pass
        if tree.remaining() != 0:
            tree.terminate()
            raise StageGuardError("my_voice:stage_descendants_still_running")
        if proc.returncode != 0:
            if proc.returncode in (3221225477, -1073741819):
                raise StageGuardError("my_voice:native_worker_crash")
            raise StageGuardError(f"my_voice:stage_exit_nonzero:{proc.returncode}")

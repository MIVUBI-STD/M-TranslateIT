"""Conservative per-stage process-tree supervisor, not a GPU safety guarantee.

Windows Job Object KILL_ON_JOB_CLOSE covers descendants after assignment.
Windows children start CREATE_SUSPENDED and only resume after Job admission.
A native worker/driver crash can still happen after resume.
GPU pressure observations use optional installed nvidia-smi and fail closed.
"""
from __future__ import annotations

import ctypes
import json
import os
import shutil
import signal
import subprocess
import tempfile
from pathlib import Path
from my_voice_adaptive_safety import PressureObserver, SafetySample
from my_voice_resource_guard import snapshot as resource_snapshot

from my_voice_resource_guard import (
    ResourceGuardError, check_start as check_system_resources, critical as critical_resources,
)
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

def _create_cooperative_request(directory: Path, token: str, stage: str, reason: str) -> None:
    # New temporary stage-scoped file; a stale signal from a previous run
    # cannot affect a new build. This is not an authentication boundary.
    data = json.dumps({
        "run_token": token, "stage": stage, "reason": reason,
    }, sort_keys=True).encode("utf-8")
    with (directory / "request.json").open("xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def _acknowledged(directory: Path, token: str, stage: str) -> bool:
    path = directory / "ack.json"
    if not path.exists():
        return False
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 4096:
        raise StageGuardError("my_voice:cooperative_ack_invalid")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise StageGuardError("my_voice:cooperative_ack_unreadable") from exc
    if (value.get("run_token") != token
        or value.get("stage") != stage
        or value.get("kind") != "checkpoint_candidate_not_resume_verified"
        or type(value.get("epoch")) is not int or value["epoch"] < 1):
        raise StageGuardError("my_voice:cooperative_ack_mismatch")
    return True


def run_supervised_stage(
    argv: list[str], *, cwd: str, env: dict[str, str], gpu_stage: bool,
    cooperative_stage: str | None = None,
) -> None:
    """One supervised stage, no throttling fiction and no automatic resume.

    High *stable* VRAM => WATCH, continue. Sustained decline => request yield
    at checkpoint epoch. Critical pressure/unknown telemetry => terminate
    owned process after repeated readings. A yield is always NOT success.
    """
    if cooperative_stage not in (None, "sovits", "gpt"):
        raise StageGuardError("my_voice:cooperative_stage_invalid")
    if cooperative_stage and not gpu_stage:
        raise StageGuardError("my_voice:cooperative_requires_cuda")
    try:
        check_system_resources()
    except ResourceGuardError as exc:
        raise StageGuardError(str(exc)) from exc
    sensor = locate_sensor() if gpu_stage else None
    if sensor and gpu_free_mib(sensor) < CRITICAL_FREE_MIB:
        raise StageGuardError("my_voice:gpu_pressure_before_stage")

    temporary: str | None = None
    token = env.get("TRANSLATEIT_MYVOICE_RUN_TOKEN", "")
    if cooperative_stage is not None:
        if len(token) != 64 or any(x not in "0123456789abcdef" for x in token):
            raise StageGuardError("my_voice:cooperative_run_token_invalid")
        root = Path(env.get("TRANSLATEIT_MYVOICE_CACHE_ROOT", "")) / "Build"
        if not root.is_dir() or root.is_symlink():
            raise StageGuardError("my_voice:cooperative_workspace_unavailable")
        temporary = tempfile.mkdtemp(prefix="safe-yield-", dir=str(root))
        env = dict(env)
        env["TRANSLATEIT_MYVOICE_STOP_REQUEST"] = str(Path(temporary) / "request.json")
        env["TRANSLATEIT_MYVOICE_STOP_ACK"] = str(Path(temporary) / "ack.json")
        env["TRANSLATEIT_MYVOICE_STOP_STAGE"] = cooperative_stage
    try:
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
            observer = PressureObserver(require_gpu=gpu_stage)
            emergency_readings = 0
            request_sent = False
            while proc.poll() is None:
                # Read once per cycle; missing RAM/disk data is unknown and
                # therefore never silently converted to NORMAL.
                try:
                    ram, cache, saved = resource_snapshot()
                    free_gpu = gpu_free_mib(sensor) if sensor else None
                    result = observer.observe(SafetySample(free_gpu, ram, cache, saved))
                except (ResourceGuardError, StageGuardError):
                    result = observer.observe(SafetySample(None, -1, -1, -1))
                if result.state in ("critical", "unknown"):
                    emergency_readings += 1
                else:
                    emergency_readings = 0
                if emergency_readings >= MAX_BAD_SAMPLES:
                    tree.terminate()
                    proc.wait(timeout=8)
                    raise StageGuardError("my_voice:emergency_resource_or_telemetry_failure")
                if result.state == "pressure" and temporary and not request_sent:
                    _create_cooperative_request(
                        Path(temporary), token, cooperative_stage, result.reason
                    )
                    request_sent = True
                try:
                    proc.wait(timeout=SENSOR_INTERVAL_SECONDS)
                except subprocess.TimeoutExpired:
                    pass
            if tree.remaining() != 0:
                tree.terminate()
                raise StageGuardError("my_voice:stage_descendants_still_running")
            # Never call an acknowledged yield safe when the worker crashed
            # after emitting its ack. Successful process exit is mandatory.
            if proc.returncode != 0:
                if proc.returncode in (3221225477, -1073741819):
                    raise StageGuardError("my_voice:native_worker_crash")
                raise StageGuardError(f"my_voice:stage_exit_nonzero:{proc.returncode}")
            if temporary and _acknowledged(Path(temporary), token, cooperative_stage):
                if not request_sent:
                    raise StageGuardError("my_voice:cooperative_unsolicited_ack")
                # This is a completed stage-process exit at a checkpoint
                # boundary, NOT completion of the full training run.
                raise StageGuardError("my_voice:cooperative_safe_stop_checkpoint_candidate")
    finally:
        if temporary:
            shutil.rmtree(temporary, ignore_errors=True)

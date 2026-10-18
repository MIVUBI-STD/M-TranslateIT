"""Pure SAFE_8GB pressure policy. No GPU imports, I/O, sleep or mutation.

WATCH tolerates high *stable* VRAM use. PRESSURE requires a downward trend.
A critical reading is an emergency signal, not permission to change weights.
Thresholds are initial design values, not experimentally validated limits.
"""
from __future__ import annotations

from collections import deque
from dataclasses import dataclass

WATCH_GPU_MIB = 1536
CRITICAL_GPU_MIB = 768
WATCH_RAM_MIB = 2048
CRITICAL_RAM_MIB = 1024
WATCH_DISK_MIB = 4096
CRITICAL_DISK_MIB = 2048
TREND_GPU_MIB = 256
TREND_RAM_MIB = 256
TREND_DISK_MIB = 512

# Ignore ordinary CUDA allocator ramp-up when deciding a *soft* yield.
# CRITICAL or UNKNOWN decisions must never be postponed by these controls.
STAGE_WARMUP_SECONDS = 90
SOFT_PRESSURE_SAMPLES = 3


def soft_stop_ready(state: str, elapsed_seconds: float, consecutive_pressure: int) -> bool:
    return (state == "pressure" and elapsed_seconds >= STAGE_WARMUP_SECONDS
            and consecutive_pressure >= SOFT_PRESSURE_SAMPLES)

@dataclass(frozen=True)
class SafetySample:
    gpu_free_mib: int | None
    ram_free_mib: int
    cache_free_mib: int
    saved_free_mib: int

    def valid(self, require_gpu: bool) -> bool:
        values = (self.ram_free_mib, self.cache_free_mib, self.saved_free_mib)
        return (
            all(type(v) is int and 0 <= v < 1048576 for v in values)
            and (not require_gpu or type(self.gpu_free_mib) is int)
            and (self.gpu_free_mib is None or (
                type(self.gpu_free_mib) is int
                and 0 <= self.gpu_free_mib < 1048576
            ))
        )

@dataclass(frozen=True)
class SafetyDecision:
    state: str
    reason: str

class PressureObserver:
    def __init__(self, *, require_gpu: bool) -> None:
        self.require_gpu = require_gpu
        self.history: deque[SafetySample] = deque(maxlen=3)

    def observe(self, sample: SafetySample) -> SafetyDecision:
        if not sample.valid(self.require_gpu):
            self.history.clear()
            return SafetyDecision("unknown", "telemetry_unavailable")
        self.history.append(sample)
        g = sample.gpu_free_mib
        if g is not None and g < CRITICAL_GPU_MIB:
            return SafetyDecision("critical", "gpu_free_critical")
        if sample.ram_free_mib < CRITICAL_RAM_MIB:
            return SafetyDecision("critical", "system_ram_critical")
        if min(sample.cache_free_mib, sample.saved_free_mib) < CRITICAL_DISK_MIB:
            return SafetyDecision("critical", "disk_space_critical")
        watching = (
            (g is not None and g < WATCH_GPU_MIB)
            or sample.ram_free_mib < WATCH_RAM_MIB
            or min(sample.cache_free_mib, sample.saved_free_mib) < WATCH_DISK_MIB
        )
        if len(self.history) == 3:
            start = self.history[0]
            if (g is not None and start.gpu_free_mib is not None
                    and g < WATCH_GPU_MIB and start.gpu_free_mib - g >= TREND_GPU_MIB):
                return SafetyDecision("pressure", "gpu_free_declining")
            if (sample.ram_free_mib < WATCH_RAM_MIB
                    and start.ram_free_mib - sample.ram_free_mib >= TREND_RAM_MIB):
                return SafetyDecision("pressure", "system_ram_declining")
            if (min(sample.cache_free_mib, sample.saved_free_mib) < WATCH_DISK_MIB
                    and (start.cache_free_mib - sample.cache_free_mib >= TREND_DISK_MIB
                         or start.saved_free_mib - sample.saved_free_mib >= TREND_DISK_MIB)):
                return SafetyDecision("pressure", "disk_space_declining")
        return SafetyDecision("watch" if watching else "normal", "stable_or_sufficient")

"""Conservative, device-adaptive epoch-stall observer, with NO auto-kill.

Training epoch duration depends on dataset, GPU, I/O and checkpoint writes.
A missing epoch alone cannot prove a hang, especially before learning timings.
Warnings only. Resource emergencies and explicit Stop remain separately owned.
"""

from __future__ import annotations

import math
from collections import deque

EARLY_OBSERVATION_SECONDS = 45 * 60
MIN_ADAPTIVE_STALL_SECONDS = 20 * 60
ADAPTIVE_SLOWDOWN_FACTOR = 6.0
MIN_MEASURED_INTERVALS = 2


class EpochStallObserver:
    def __init__(self) -> None:
        self.started_at: float | None = None
        self.last_at: float | None = None
        self.last_epoch = 0
        self.intervals: deque[float] = deque(maxlen=4)
        self.warned_for_epoch: int | None = None

    def begin(self, when: float) -> None:
        if not math.isfinite(when):
            raise ValueError("my_voice:epoch_stall_clock_invalid")
        if self.started_at is None:
            self.started_at = when

    def observe_epoch(self, epoch: int, when: float) -> None:
        if type(epoch) is not int or epoch <= self.last_epoch or not math.isfinite(when):
            raise ValueError("my_voice:epoch_stall_observation_invalid")
        if self.last_at is not None:
            elapsed = when - self.last_at
            if elapsed <= 0:
                raise ValueError("my_voice:epoch_stall_clock_invalid")
            # Coalesced short epoch completions count as several epochs,
            # not one unusually slow interval for this GPU.
            self.intervals.append(elapsed / (epoch - self.last_epoch))
        if self.started_at is None:
            self.started_at = when
        self.last_at = when
        self.last_epoch = epoch
        self.warned_for_epoch = None

    def possible_stall(self, when: float) -> str | None:
        if not math.isfinite(when):
            return None
        # Before at least two complete epoch intervals, the local hardware
        # performance baseline is unknown. No automatic timeout is justified.
        if self.last_at is None:
            if (
                self.started_at is not None
                and when - self.started_at >= EARLY_OBSERVATION_SECONDS
                and self.warned_for_epoch != 0
            ):
                self.warned_for_epoch = 0
                return "first_epoch_not_observed_no_baseline"
            return None
        if self.warned_for_epoch == self.last_epoch:
            return None
        since = when - self.last_at
        if since <= 0:
            return None
        if len(self.intervals) < MIN_MEASURED_INTERVALS:
            if since < EARLY_OBSERVATION_SECONDS:
                return None
            kind = "epoch_not_seen_for_45_minutes_no_baseline"
        else:
            limit = max(MIN_ADAPTIVE_STALL_SECONDS, ADAPTIVE_SLOWDOWN_FACTOR * max(self.intervals))
            if since < limit:
                return None
            kind = "epoch_duration_exceeds_measured_device_baseline"
        self.warned_for_epoch = self.last_epoch
        return kind

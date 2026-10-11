"""Non-destructive, source-only diagnostics for canonical My Voice recordings.

These frame-energy features are NOT measured SNR, speech recognition,
dereverberation, or a perceptual voice-quality score. They never modify a WAV.
Do not turn their advisory results into a training blocker without real audio
evidence. The approved GPT-SoVITS engine and frozen input bytes stay unchanged.
"""

from __future__ import annotations

import math
import statistics
import struct
import wave
from pathlib import Path

SAMPLE_RATE = 32_000
FRAME_SAMPLES = 640  # 20 ms at the canonical rate.
QUIET_CONTEXT_MIN_FRAMES = 8  # At least 160 ms of quiet leading/trailing audio.
MAX_TAKE_FRAMES = SAMPLE_RATE * 60


def inspect_take_frames(path: Path) -> dict[str, float | int | None]:
    """Describe temporal energy without classifying speech or estimating SNR."""
    try:
        with wave.open(str(path), "rb") as reader:
            if (
                reader.getnchannels() != 1
                or reader.getsampwidth() != 2
                or reader.getframerate() != SAMPLE_RATE
                or reader.getcomptype() != "NONE"
            ):
                raise ValueError("noncanonical_audio")
            total = reader.getnframes()
            if not 0 < total <= MAX_TAKE_FRAMES:
                raise ValueError("unsupported_take_duration")

            energies: list[float] = []
            remaining = total
            while remaining:
                n = min(FRAME_SAMPLES, remaining)
                raw = reader.readframes(n)
                if len(raw) != n * 2:
                    raise ValueError("incomplete_audio")
                square_sum = sum(
                    value * value
                    for (value,) in struct.iter_unpack("<h", raw)
                )
                energies.append(math.sqrt(square_sum / n))
                remaining -= n
    except (OSError, EOFError, wave.Error) as exc:
        raise ValueError("invalid_audio") from exc

    peak_frame = max(energies)
    quiet_limit = peak_frame * 0.35
    leading: list[float] = []
    for energy in energies:
        if energy >= quiet_limit or peak_frame == 0:
            break
        leading.append(energy)

    trailing: list[float] = []
    for energy in reversed(energies):
        if energy >= quiet_limit or peak_frame == 0:
            break
        trailing.append(energy)

    quiet_regions = [
        region for region in (leading, trailing)
        if len(region) >= QUIET_CONTEXT_MIN_FRAMES
    ]
    # A quiet boundary can reveal a comparative noise proxy. It cannot
    # identify whether the energy is fan noise, room tone, or quiet speech.
    edge_floor = (
        round(statistics.median(energy for region in quiet_regions for energy in region), 3)
        if quiet_regions else None
    )

    longest_low = current_low = 0
    active_frames = 0
    for energy in energies:
        if energy > 256.0:
            active_frames += 1
        if energy <= 128.0:
            current_low += 1
            longest_low = max(longest_low, current_low)
        else:
            current_low = 0

    return {
        "frame_ms": 20,
        "activity_frame_ratio": round(active_frames / len(energies), 4),
        "longest_low_energy_run_ms": longest_low * 20,
        "leading_quiet_ms": len(leading) * 20,
        "trailing_quiet_ms": len(trailing) * 20,
        "qualified_quiet_context_ms": sum(len(region) for region in quiet_regions) * 20,
        "edge_floor_rms_pcm16": edge_floor,
    }


def reference_quiet_context_available(reports: list[dict[str, float | int | None]]) -> bool:
    """Compare the quiet-edge proxy only if ALL candidates have that evidence."""
    return bool(reports) and all(
        report["edge_floor_rms_pcm16"] is not None for report in reports
    )

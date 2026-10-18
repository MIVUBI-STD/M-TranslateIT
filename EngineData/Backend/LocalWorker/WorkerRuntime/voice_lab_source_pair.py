"""Inspect a frozen native-rate VoiceLab WAV against its canonical 32 kHz take.

A waveform match is an advisory signal-consistency check, NOT proof of audio
provenance, speaker identity or better voice quality. This does not modify WAVs,
invoke neural enhancement, or change the frozen GPT-SoVITS training source.
"""

from __future__ import annotations

import hashlib
import io
import math
import wave
from pathlib import Path
from typing import Any

MAX_CANONICAL_BYTES = 4 * 1024 * 1024
MAX_NATIVE_BYTES = 25 * 1024 * 1024
TARGET_RATE = 32_000


def _read_pcm16(path: Path, size_limit: int) -> tuple[Any, int, str]:
    import numpy as np

    if path.is_symlink() or not path.is_file():
        raise ValueError("not_regular_file")
    size = path.stat().st_size
    if not 44 <= size <= size_limit:
        raise ValueError("out_of_bounds")
    raw = path.read_bytes()
    if len(raw) != size:
        raise ValueError("changed_during_read")
    try:
        with wave.open(io.BytesIO(raw), "rb") as stream:
            rate = stream.getframerate()
            if (
                stream.getnchannels() != 1
                or stream.getsampwidth() != 2
                or stream.getcomptype() != "NONE"
                or not 8_000 <= rate <= 192_000
            ):
                raise ValueError("unsupported_format")
            count = stream.getnframes()
            if not 0 < count <= rate * 60:
                raise ValueError("unsupported_duration")
            payload = stream.readframes(count)
            if len(payload) != count * 2:
                raise ValueError("truncated_data")
    except (EOFError, wave.Error) as exc:
        raise ValueError("invalid_wav") from exc
    return (
        np.frombuffer(payload, dtype="<i2").astype(np.float64) / 32768.0,
        rate,
        hashlib.sha256(raw).hexdigest(),
    )


def inspect_frozen_source_pair(canonical: Path, native: Path) -> dict[str, Any]:
    result: dict[str, Any] = {
        "method": "polyphase_waveform_consistency_v1",
        "status": "unavailable",
        "eligible_for_offline_enhancement": False,
    }
    if not native.exists() and not native.is_symlink():
        return result
    try:
        target, target_rate, target_hash = _read_pcm16(canonical, MAX_CANONICAL_BYTES)
        source, source_rate, source_hash = _read_pcm16(native, MAX_NATIVE_BYTES)
    except (OSError, ValueError):
        result["status"] = "invalid_audio"
        return result

    result.update(
        {
            "canonical_sha256": target_hash,
            "native_sha256": source_hash,
            "native_sample_rate_hz": source_rate,
        }
    )
    if target_rate != TARGET_RATE:
        result["status"] = "invalid_canonical_rate"
        return result
    if abs(len(target) / target_rate - len(source) / source_rate) > 0.003:
        result["status"] = "duration_mismatch"
        return result

    import numpy as np
    from scipy.signal import resample_poly

    divisor = math.gcd(target_rate, source_rate)
    adjusted = resample_poly(source, target_rate // divisor, source_rate // divisor)
    if abs(len(adjusted) - len(target)) > 4:
        result["status"] = "resample_length_mismatch"
        return result
    length = min(len(adjusted), len(target))
    trim = min(640, length // 8)
    if length - trim * 2 < 1_600:
        result["status"] = "too_short_to_compare"
        return result
    target_slice = target[trim : length - trim]
    native_slice = adjusted[trim : length - trim]
    target_rms = float(np.sqrt(np.mean(target_slice**2)))
    native_rms = float(np.sqrt(np.mean(native_slice**2)))
    if min(target_rms, native_rms) < 0.005:
        result["status"] = "insufficient_signal"
        return result
    similarity = float(
        np.dot(target_slice, native_slice)
        / (np.linalg.norm(target_slice) * np.linalg.norm(native_slice))
    )
    gain_difference_db = abs(20.0 * math.log10(native_rms / target_rms))
    if not math.isfinite(similarity) or not math.isfinite(gain_difference_db):
        result["status"] = "invalid_comparison"
        return result
    result["waveform_correlation"] = round(similarity, 6)
    result["level_difference_db"] = round(gain_difference_db, 3)
    if similarity >= 0.97 and gain_difference_db <= 2.0:
        result["status"] = "signal_consistent"
        result["eligible_for_offline_enhancement"] = True
    else:
        result["status"] = "signal_mismatch"
    return result

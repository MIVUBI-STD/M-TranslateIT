"""Contract fixtures for source-pair comparison; not microphone quality proof."""

import math
import tempfile
import unittest
import wave
from pathlib import Path

import numpy as np
from scipy.signal import resample_poly

from voice_lab_source_pair import inspect_frozen_source_pair


def tone(rate: int, freq: int = 440) -> np.ndarray:
    t = np.arange(rate, dtype=np.float64)
    fade = np.minimum(1.0, t / 2_000) * np.minimum(1.0, (rate - 1 - t) / 2_000)
    s = (
        fade
        * 0.18
        * (np.sin(2 * math.pi * freq * t / rate) + 0.3 * np.sin(2 * math.pi * 1_300 * t / rate))
    )
    return np.rint(s * 32767).astype(np.int16)


def wav(path: Path, pcm: np.ndarray, rate: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as stream:
        stream.setnchannels(1)
        stream.setsampwidth(2)
        stream.setframerate(rate)
        stream.writeframes(pcm.astype("<i2").tobytes())


class VoiceLabSourcePairTests(unittest.TestCase):
    def test_same_recording_at_48k_and_32k_is_consistent(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            original = tone(48_000)
            canonical = np.rint(
                resample_poly(original.astype(np.float64) / 32768, 2, 3) * 32767
            ).astype(np.int16)
            native_path = root / "NativeRate" / "take_0001.wav"
            canonical_path = root / "take_0001.wav"
            wav(native_path, original, 48_000)
            wav(canonical_path, canonical, 32_000)
            original_bytes = (native_path.read_bytes(), canonical_path.read_bytes())
            info = inspect_frozen_source_pair(canonical_path, native_path)
            self.assertEqual(info["status"], "signal_consistent")
            self.assertTrue(info["eligible_for_offline_enhancement"])
            self.assertEqual(info["native_sample_rate_hz"], 48_000)
            self.assertEqual(len(info["native_sha256"]), 64)
            self.assertEqual(
                (native_path.read_bytes(), canonical_path.read_bytes()), original_bytes
            )

    def test_legacy_missing_sidecar_is_not_blocking_training(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            original = root / "take_0001.wav"
            wav(original, tone(32_000), 32_000)
            info = inspect_frozen_source_pair(original, root / "NativeRate" / original.name)
            self.assertEqual(info["status"], "unavailable")
            self.assertFalse(info["eligible_for_offline_enhancement"])

    def test_unrelated_waveforms_are_not_paired(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            canonical_path = root / "take_0001.wav"
            native_path = root / "NativeRate" / "take_0001.wav"
            wav(canonical_path, tone(32_000, 440), 32_000)
            wav(native_path, tone(48_000, 790), 48_000)
            info = inspect_frozen_source_pair(canonical_path, native_path)
            self.assertEqual(info["status"], "signal_mismatch")
            self.assertFalse(info["eligible_for_offline_enhancement"])

    def test_invalid_sidecar_is_ineligible(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            canonical_path = root / "take_0001.wav"
            native_path = root / "NativeRate" / "take_0001.wav"
            wav(canonical_path, tone(32_000), 32_000)
            native_path.parent.mkdir()
            native_path.write_bytes(b"RIFF" + b"\x00" * 40)
            info = inspect_frozen_source_pair(canonical_path, native_path)
            self.assertEqual(info["status"], "invalid_audio")

    def test_duration_mismatch_is_ineligible(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            canonical_path = root / "take_0001.wav"
            native_path = root / "NativeRate" / "take_0001.wav"
            wav(canonical_path, tone(32_000), 32_000)
            wav(native_path, tone(48_000)[:24_000], 48_000)
            info = inspect_frozen_source_pair(canonical_path, native_path)
            self.assertEqual(info["status"], "duration_mismatch")

    def test_same_rate_exact_copy_is_consistent(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            canonical_path = root / "take_0001.wav"
            native_path = root / "NativeRate" / "take_0001.wav"
            wav(canonical_path, tone(32_000), 32_000)
            native_path.parent.mkdir()
            native_path.write_bytes(canonical_path.read_bytes())
            info = inspect_frozen_source_pair(canonical_path, native_path)
            self.assertEqual(info["status"], "signal_consistent")


if __name__ == "__main__":
    unittest.main()

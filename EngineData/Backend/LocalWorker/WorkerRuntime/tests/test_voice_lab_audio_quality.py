"""Audio-quality source regression cases; no neural model or device required."""

from __future__ import annotations

import json
import struct
import tempfile
import unittest
import wave
from pathlib import Path

from voice_lab_audio_quality import (
    inspect_take_frames,
    reference_quiet_context_available,
)
from voice_lab_build import validate_dataset_signal
from voice_lab_gpt_sovits_build import select_reference, training_takes


def write_take(path: Path, *, edge_level: int, edge_ms: int = 300) -> None:
    count = 32_000 * 5
    edge = 32_000 * edge_ms // 1_000
    samples = [
        (edge_level if index % 2 else -edge_level)
        if index < edge or index >= count - edge
        else (8_000 if index % 2 else -8_000)
        for index in range(count)
    ]
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack(f"<{count}h", *samples))


class VoiceLabAudioQualityTests(unittest.TestCase):
    def test_quiet_edge_energy_is_comparative_not_claimed_snr(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            noisy = Path(tmp) / "take_0001.wav"
            quiet = Path(tmp) / "take_0002.wav"
            write_take(noisy, edge_level=1_200)
            write_take(quiet, edge_level=400)
            a = inspect_take_frames(noisy)
            b = inspect_take_frames(quiet)
            self.assertGreater(a["edge_floor_rms_pcm16"], b["edge_floor_rms_pcm16"])
            self.assertGreaterEqual(a["qualified_quiet_context_ms"], 320)
            self.assertTrue(reference_quiet_context_available([a, b]))
            self.assertNotIn("snr", a)
            self.assertNotIn("noise_reduction_score", a)

    def test_no_quiet_edge_does_not_invent_noise_measurement(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take_0001.wav"
            write_take(path, edge_level=8_000)
            report = inspect_take_frames(path)
            self.assertIsNone(report["edge_floor_rms_pcm16"])
            self.assertFalse(reference_quiet_context_available([report]))

    def test_reference_prefers_quieter_edge_when_comparable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_take(root / "take_0001.wav", edge_level=1_200)
            write_take(root / "take_0002.wav", edge_level=400)
            manifest = {
                "takes": [
                    {
                        "line_id": line_id,
                        "exact_text": f"spoken line {line_id}",
                        "wav_file": f"take_{line_id:04}.wav",
                    }
                    for line_id in (1, 2)
                ],
                "held_out_lines": [
                    {"line_id": 1001, "exact_text": "unseen reference text"}
                ],
            }
            selected = select_reference(training_takes(root, manifest))
            self.assertEqual(selected["line_id"], 2)

    def test_frozen_dataset_report_does_not_modify_training_wav(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first = root / "take_0001.wav"
            second = root / "take_0002.wav"
            write_take(first, edge_level=1_200)
            write_take(second, edge_level=400)
            old_bytes = (first.read_bytes(), second.read_bytes())
            manifest = {
                "takes": [
                    {"wav_file": first.name},
                    {"wav_file": second.name},
                ]
            }
            validate_dataset_signal(root, manifest)
            report = json.loads((root / "audio_quality.json").read_text("utf-8"))
            self.assertEqual(report["schema"], "voice_lab.audio_quality.v1")
            self.assertEqual(len(report["takes"]), 2)
            self.assertEqual((first.read_bytes(), second.read_bytes()), old_bytes)
            self.assertEqual(report["method"], "non_destructive_frame_energy")
            self.assertFalse(report["provides_perceptual_quality_score"])

    def test_invalid_format_fails_explicitly(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take_0001.wav"
            with wave.open(str(path), "wb") as writer:
                writer.setnchannels(2)
                writer.setsampwidth(2)
                writer.setframerate(48_000)
                writer.writeframes(b"\0" * 4_800)
            with self.assertRaisesRegex(ValueError, "noncanonical_audio"):
                inspect_take_frames(path)


if __name__ == "__main__":
    unittest.main()

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


def write_take(
    path: Path, *, edge_level: int, edge_ms: int = 300, speech_level: int = 8_000
) -> None:
    count = 32_000 * 5
    edge = 32_000 * edge_ms // 1_000
    samples = [
        (edge_level if index % 2 else -edge_level)
        if index < edge or index >= count - edge
        else (speech_level if index % 2 else -speech_level)
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
                "held_out_lines": [{"line_id": 1001, "exact_text": "unseen reference text"}],
            }
            selected = select_reference(training_takes(root, manifest))
            self.assertEqual(selected["line_id"], 2)

    def test_reference_ranks_relative_edge_energy_not_microphone_gain(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            # A has quieter absolute boundaries, but much weaker speech.
            # B has a better boundary-to-active-energy ratio at higher gain.
            quiet_but_weak = root / "take_0001.wav"
            stronger_speech = root / "take_0002.wav"
            write_take(quiet_but_weak, edge_level=300, speech_level=3_000)
            write_take(stronger_speech, edge_level=600, speech_level=12_000)
            original = (quiet_but_weak.read_bytes(), stronger_speech.read_bytes())
            manifest = {
                "takes": [
                    {"line_id": 1, "exact_text": "first example", "wav_file": quiet_but_weak.name},
                    {
                        "line_id": 2,
                        "exact_text": "second example",
                        "wav_file": stronger_speech.name,
                    },
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "unseen words"}],
            }
            metrics_a = inspect_take_frames(quiet_but_weak)
            metrics_b = inspect_take_frames(stronger_speech)
            self.assertLess(
                metrics_a["edge_floor_rms_pcm16"],
                metrics_b["edge_floor_rms_pcm16"],
            )
            selected = select_reference(training_takes(root, manifest))
            self.assertEqual(selected["line_id"], 2)
            self.assertEqual((quiet_but_weak.read_bytes(), stronger_speech.read_bytes()), original)

    def test_reference_selection_keeps_existing_clipping_priority(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            clean = root / "take_0001.wav"
            lightly_clipped = root / "take_0002.wav"
            write_take(clean, edge_level=900, speech_level=8_000)
            write_take(lightly_clipped, edge_level=100, speech_level=12_000)
            with wave.open(str(lightly_clipped), "rb") as reader:
                payload = bytearray(reader.readframes(reader.getnframes()))
            for index in range(32_000, 33_600):
                struct.pack_into("<h", payload, index * 2, 32_767 if index % 2 else -32_767)
            with wave.open(str(lightly_clipped), "wb") as writer:
                writer.setnchannels(1)
                writer.setsampwidth(2)
                writer.setframerate(32_000)
                writer.writeframes(payload)
            manifest = {
                "takes": [
                    {"line_id": 1, "exact_text": "clean example", "wav_file": clean.name},
                    {
                        "line_id": 2,
                        "exact_text": "clipped example",
                        "wav_file": lightly_clipped.name,
                    },
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "unseen words"}],
            }
            self.assertEqual(select_reference(training_takes(root, manifest))["line_id"], 1)

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

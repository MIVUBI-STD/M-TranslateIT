from __future__ import annotations

import struct
import tempfile
import unittest
import wave
from pathlib import Path

from voice_lab_build import BuildError, validate_take_signal
from voice_lab_gpt_sovits import GPT_EPOCHS, SOVITS_EPOCHS, VoiceLabProviderError
from voice_lab_gpt_sovits_build import select_reference, training_takes


class VoiceLabBuildContractTests(unittest.TestCase):
    @staticmethod
    def write_canonical_wav(path: Path, duration_ms: int) -> None:
        frames = 32_000 * duration_ms // 1_000
        with wave.open(str(path), "wb") as writer:
            writer.setnchannels(1)
            writer.setsampwidth(2)
            writer.setframerate(32_000)
            writer.writeframes(b"\x00\x00" * frames)

    @staticmethod
    def write_signal_wav(path: Path, samples: list[int]) -> None:
        with wave.open(str(path), "wb") as writer:
            writer.setnchannels(1)
            writer.setsampwidth(2)
            writer.setframerate(32_000)
            writer.writeframes(struct.pack(f"<{len(samples)}h", *samples))

    def test_training_epoch_contract_is_explicit_and_importable(self) -> None:
        self.assertEqual(SOVITS_EPOCHS, 8)
        self.assertEqual(GPT_EPOCHS, 15)

    def test_reference_selection_prefers_take_closest_to_five_seconds(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            durations = [(1, 3_200), (2, 5_050), (3, 7_400)]
            takes = []
            for line_id, duration_ms in durations:
                name = f"take_{line_id:04}.wav"
                self.write_canonical_wav(root / name, duration_ms)
                takes.append(
                    {"line_id": line_id, "exact_text": f"line {line_id}", "wav_file": name}
                )
            manifest = {
                "takes": takes,
                "held_out_lines": [{"line_id": 1001, "exact_text": "held out sentence"}],
            }
            normalized = training_takes(root, manifest)
            selected = select_reference(normalized)
            self.assertEqual(selected["line_id"], 2)
            self.assertEqual(selected["duration_ms"], 5_050)

    def test_held_out_text_cannot_overlap_training_text(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self.write_canonical_wav(root / "take_0001.wav", 4_000)
            manifest = {
                "takes": [
                    {
                        "line_id": 1,
                        "exact_text": "same sentence",
                        "wav_file": "take_0001.wav",
                    }
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "same sentence"}],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "invalid_held_out_line"):
                training_takes(root, manifest)

    def test_noncanonical_take_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            path = root / "take_0001.wav"
            with wave.open(str(path), "wb") as writer:
                writer.setnchannels(2)
                writer.setsampwidth(2)
                writer.setframerate(48_000)
                writer.writeframes(b"\x00\x00" * 48_000 * 2)
            manifest = {
                "takes": [
                    {
                        "line_id": 1,
                        "exact_text": "training sentence",
                        "wav_file": path.name,
                    }
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "held out sentence"}],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "noncanonical_take"):
                training_takes(root, manifest)

    def test_build_gate_rejects_excessive_silence(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "take_0001.wav"
            samples = [0] * 30_000 + [4_000] * 2_000
            self.write_signal_wav(path, samples)
            with self.assertRaisesRegex(BuildError, "take_excessive_silence"):
                validate_take_signal(path)

    def test_build_gate_rejects_severe_clipping(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "take_0001.wav"
            samples = [8_000] * 30_000 + [32_767] * 2_000
            self.write_signal_wav(path, samples)
            with self.assertRaisesRegex(BuildError, "take_severe_clipping"):
                validate_take_signal(path)

    def test_build_gate_accepts_normal_signal(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "take_0001.wav"
            samples = [3_000 if index % 2 == 0 else -3_000 for index in range(32_000)]
            self.write_signal_wav(path, samples)
            validate_take_signal(path)


if __name__ == "__main__":
    unittest.main()

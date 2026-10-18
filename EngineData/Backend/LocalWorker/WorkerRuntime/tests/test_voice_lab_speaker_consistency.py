"""Source contracts for the existing GPT-SoVITS speaker embeddings."""

from __future__ import annotations

import json
import struct
import tempfile
import unittest
import wave
from pathlib import Path

import torch

from voice_lab_audio_quality import attach_speaker_evidence
from voice_lab_gpt_sovits import VoiceLabProviderError
from voice_lab_gpt_sovits_build import select_reference, training_takes
from voice_lab_speaker_consistency import inspect_speaker_embeddings


def write_take(path: Path) -> None:
    samples = [500, -500] * 4_800 + [8_000, -8_000] * 70_400 + [500, -500] * 4_800
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack(f"<{len(samples)}h", *samples))


class VoiceLabSpeakerConsistencyTests(unittest.TestCase):
    def make_fixture(self, root: Path) -> tuple[list[dict], Path]:
        takes = []
        folder = root / "7-sv_cn"
        folder.mkdir()
        for line_id in (1, 2, 3):
            name = f"take_{line_id:04}.wav"
            write_take(root / name)
            takes.append(
                {
                    "line_id": line_id,
                    "exact_text": f"unique sentence {line_id}",
                    "wav_file": name,
                }
            )
        return takes, folder

    @staticmethod
    def save_vectors(folder: Path, vectors: list[list[float]]) -> None:
        for line_id, values in enumerate(vectors, start=1):
            torch.save(torch.tensor([values]), folder / f"take_{line_id:04}.wav.pt")

    def test_consistency_is_advisory_and_uses_existing_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            takes, folder = self.make_fixture(Path(temp))
            self.save_vectors(folder, [[1.0, 0.0], [0.9, 0.1], [-1.0, 0.0]])
            result = inspect_speaker_embeddings(folder, takes)
            self.assertTrue(result["advisory_only"])
            self.assertFalse(result["automated_take_rejection"])
            self.assertEqual(len(result["per_take"]), 3)
            self.assertEqual(result["per_take"][0]["line_id"], 1)

    def test_missing_embedding_does_not_silently_pass(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            takes, folder = self.make_fixture(Path(temp))
            self.save_vectors(folder, [[1.0, 0.0], [0.9, 0.1], [-1.0, 0.0]])
            (folder / "take_0002.wav.pt").unlink()
            with self.assertRaisesRegex(VoiceLabProviderError, "speaker_embedding_missing:2"):
                inspect_speaker_embeddings(folder, takes)

    def test_invalid_embedding_is_rejected_before_training(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            takes, folder = self.make_fixture(Path(temp))
            self.save_vectors(folder, [[1.0, 0.0], [0.9, float("nan")], [-1.0, 0.0]])
            with self.assertRaisesRegex(VoiceLabProviderError, "speaker_embedding_nonfinite:2"):
                inspect_speaker_embeddings(folder, takes)
            self.save_vectors(folder, [[1.0, 0.0], [0.9, 0.1], [-1.0, 0.0]])
            torch.save(torch.tensor([[1.0, 0.0, 0.0]]), folder / "take_0003.wav.pt")
            with self.assertRaisesRegex(
                VoiceLabProviderError, "speaker_embedding_dimension_mismatch:3"
            ):
                inspect_speaker_embeddings(folder, takes)

    def test_reference_identity_breaks_only_equal_signal_ties(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            takes, _ = self.make_fixture(root)
            normalized = training_takes(
                root,
                {
                    "takes": takes,
                    "held_out_lines": [{"line_id": 1001, "exact_text": "unseen held out"}],
                },
            )
            self.assertEqual(select_reference(normalized)["line_id"], 1)
            selected = select_reference(normalized, peer_cosines={1: 0.1, 2: 0.95, 3: 0.2})
            self.assertEqual(selected["line_id"], 2)

    def test_report_stays_in_existing_frozen_dataset_and_does_not_touch_audio(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            takes, _ = self.make_fixture(root)
            original = [(root / take["wav_file"]).read_bytes() for take in takes]
            report = root / "audio_quality.json"
            report.write_text(
                json.dumps(
                    {
                        "schema": "voice_lab.audio_quality.v1",
                        "method": "non_destructive_frame_energy",
                        "takes": [{"wav_file": take["wav_file"]} for take in takes],
                    }
                )
            )
            evidence = {
                "per_take": [
                    {"line_id": take["line_id"], "median_peer_cosine": 0.9} for take in takes
                ]
            }
            attach_speaker_evidence(report, takes, evidence)
            result = json.loads(report.read_text("utf-8"))
            self.assertEqual(result["speaker_consistency"], evidence)
            self.assertEqual(
                [(root / take["wav_file"]).read_bytes() for take in takes],
                original,
            )

    def test_report_rejects_wrong_frozen_dataset(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            report = Path(temp) / "audio_quality.json"
            report.write_text(
                json.dumps(
                    {
                        "schema": "voice_lab.audio_quality.v1",
                        "takes": [{"wav_file": "take_0010.wav"}],
                    }
                )
            )
            with self.assertRaisesRegex(ValueError, "audio_quality_report_dataset_mismatch"):
                attach_speaker_evidence(
                    report,
                    [{"line_id": 1, "wav_file": "take_0001.wav"}],
                    {"per_take": [{"line_id": 1}]},
                )


if __name__ == "__main__":
    unittest.main()

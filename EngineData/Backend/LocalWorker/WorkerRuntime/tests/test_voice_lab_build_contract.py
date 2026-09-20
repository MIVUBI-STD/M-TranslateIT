from __future__ import annotations

import struct
import tempfile
import unittest
import wave
from pathlib import Path

from voice_lab_build import BuildError, validate_dataset_signal, validate_take_signal
from voice_lab_gpt_sovits import GPT_EPOCHS, SOVITS_EPOCHS, VoiceLabProviderError
from voice_lab_gpt_sovits_build import (
    select_best_candidate,
    select_reference,
    synthesis_artifact_flags,
    training_takes,
    word_error_rate,
)


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
            metrics = validate_take_signal(path)
            self.assertGreater(metrics["active_rms"], 0.0)
            self.assertEqual(metrics["dc_offset"], 0.0)

    def test_build_gate_rejects_signal_that_is_too_low(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "take_0001.wav"
            samples = [200 if index % 2 == 0 else -200 for index in range(32_000)]
            self.write_signal_wav(path, samples)
            with self.assertRaisesRegex(BuildError, "take_signal_too_low"):
                validate_take_signal(path)

    def test_build_gate_rejects_large_dc_offset(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "take_0001.wav"
            samples = [4_000 if index % 2 == 0 else 1_000 for index in range(32_000)]
            self.write_signal_wav(path, samples)
            with self.assertRaisesRegex(BuildError, "take_dc_offset_too_high"):
                validate_take_signal(path)

    def test_dataset_gate_rejects_extreme_recording_level_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            loud = root / "take_0001.wav"
            quiet = root / "take_0002.wav"
            self.write_signal_wav(
                loud, [12_000 if index % 2 == 0 else -12_000 for index in range(32_000)]
            )
            self.write_signal_wav(
                quiet, [1_000 if index % 2 == 0 else -1_000 for index in range(32_000)]
            )
            manifest = {
                "takes": [
                    {"line_id": 1, "exact_text": "one", "wav_file": loud.name},
                    {"line_id": 2, "exact_text": "two", "wav_file": quiet.name},
                ]
            }
            with self.assertRaisesRegex(BuildError, "dataset_recording_level_inconsistent"):
                validate_dataset_signal(root, manifest)

    def test_dataset_gate_accepts_reasonably_consistent_levels(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            first = root / "take_0001.wav"
            second = root / "take_0002.wav"
            self.write_signal_wav(
                first, [3_000 if index % 2 == 0 else -3_000 for index in range(32_000)]
            )
            self.write_signal_wav(
                second, [5_000 if index % 2 == 0 else -5_000 for index in range(32_000)]
            )
            manifest = {
                "takes": [
                    {"line_id": 1, "exact_text": "one", "wav_file": first.name},
                    {"line_id": 2, "exact_text": "two", "wav_file": second.name},
                ]
            }
            validate_dataset_signal(root, manifest)

    def test_word_error_rate_detects_omission_and_substitution(self) -> None:
        self.assertEqual(
            word_error_rate("please confirm the schedule", "please confirm the schedule"), 0.0
        )
        self.assertEqual(
            word_error_rate("please confirm the schedule", "please confirm schedule"), 0.25
        )
        self.assertEqual(word_error_rate("fifteen not fifty", "fifty not fifteen"), 2 / 3)

    def test_candidate_selection_prioritizes_intelligibility_before_similarity(self) -> None:
        candidates = [
            {
                "candidate_id": "similar-but-wrong",
                "candidate_order": 0,
                "mean_speaker_similarity": 0.99,
                "minimum_speaker_similarity": 0.98,
                "mean_intelligibility_wer": 0.25,
                "maximum_intelligibility_wer": 0.5,
                "artifact_case_count": 0,
                "samples": [{"line_id": 1001}],
            },
            {
                "candidate_id": "clearer",
                "candidate_order": 1,
                "mean_speaker_similarity": 0.90,
                "minimum_speaker_similarity": 0.88,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 0,
                "samples": [{"line_id": 1001}],
            },
        ]
        self.assertEqual(select_best_candidate(candidates)["candidate_id"], "clearer")

    def test_candidate_selection_uses_similarity_only_after_intelligibility_tie(self) -> None:
        candidates = [
            {
                "candidate_id": "lower-similarity",
                "candidate_order": 0,
                "mean_speaker_similarity": 0.90,
                "minimum_speaker_similarity": 0.87,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 0,
                "samples": [{"line_id": 1001}],
            },
            {
                "candidate_id": "higher-similarity",
                "candidate_order": 1,
                "mean_speaker_similarity": 0.94,
                "minimum_speaker_similarity": 0.91,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 0,
                "samples": [{"line_id": 1001}],
            },
        ]
        self.assertEqual(select_best_candidate(candidates)["candidate_id"], "higher-similarity")

    def test_candidate_selection_rejects_artifacted_candidate_before_score_tiebreaks(self) -> None:
        candidates = [
            {
                "candidate_id": "artifacted",
                "candidate_order": 0,
                "mean_speaker_similarity": 0.99,
                "minimum_speaker_similarity": 0.98,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 1,
                "samples": [{"line_id": 1001}],
            },
            {
                "candidate_id": "clean",
                "candidate_order": 1,
                "mean_speaker_similarity": 0.90,
                "minimum_speaker_similarity": 0.88,
                "mean_intelligibility_wer": 0.05,
                "maximum_intelligibility_wer": 0.10,
                "artifact_case_count": 0,
                "samples": [{"line_id": 1001}],
            },
        ]
        self.assertEqual(select_best_candidate(candidates)["candidate_id"], "clean")

    def test_synthesis_artifact_flags_detect_only_gross_signal_failures(self) -> None:
        clean = [0.1 if index % 2 == 0 else -0.1 for index in range(32_000)]
        self.assertEqual(synthesis_artifact_flags(clean), [])
        clipped = [1.0] * 2_000 + [0.1] * 30_000
        self.assertEqual(synthesis_artifact_flags(clipped), ["clipping"])
        silent = [0.0] * 30_000 + [0.1] * 2_000
        self.assertEqual(synthesis_artifact_flags(silent), ["unexpected_silence"])

    def test_training_takes_reject_duplicate_training_identity(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            for name in ("take_0001.wav", "take_0002.wav"):
                self.write_canonical_wav(root / name, 4_000)
            duplicate_id = {
                "takes": [
                    {"line_id": 1, "exact_text": "first", "wav_file": "take_0001.wav"},
                    {"line_id": 1, "exact_text": "second", "wav_file": "take_0002.wav"},
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "held out"}],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "invalid_training_take"):
                training_takes(root, duplicate_id)

            duplicate_text = {
                "takes": [
                    {"line_id": 1, "exact_text": "same", "wav_file": "take_0001.wav"},
                    {"line_id": 2, "exact_text": "same", "wav_file": "take_0002.wav"},
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "held out"}],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "invalid_training_take"):
                training_takes(root, duplicate_text)

            duplicate_file = {
                "takes": [
                    {"line_id": 1, "exact_text": "first", "wav_file": "take_0001.wav"},
                    {"line_id": 2, "exact_text": "second", "wav_file": "take_0001.wav"},
                ],
                "held_out_lines": [{"line_id": 1001, "exact_text": "held out"}],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "invalid_training_take"):
                training_takes(root, duplicate_file)

    def test_training_takes_reject_duplicate_held_out_text(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self.write_canonical_wav(root / "take_0001.wav", 4_000)
            manifest = {
                "takes": [
                    {"line_id": 1, "exact_text": "training", "wav_file": "take_0001.wav"}
                ],
                "held_out_lines": [
                    {"line_id": 1001, "exact_text": "same held out"},
                    {"line_id": 1002, "exact_text": "same held out"},
                ],
            }
            with self.assertRaisesRegex(VoiceLabProviderError, "invalid_held_out_line"):
                training_takes(root, manifest)

    def test_candidate_selection_fails_if_every_checkpoint_has_gross_artifacts(self) -> None:
        candidates = [
            {
                "candidate_id": "artifact-a",
                "candidate_order": 0,
                "mean_speaker_similarity": 0.95,
                "minimum_speaker_similarity": 0.92,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 1,
                "samples": [{"line_id": 1001}],
            },
            {
                "candidate_id": "artifact-b",
                "candidate_order": 1,
                "mean_speaker_similarity": 0.96,
                "minimum_speaker_similarity": 0.93,
                "mean_intelligibility_wer": 0.0,
                "maximum_intelligibility_wer": 0.0,
                "artifact_case_count": 2,
                "samples": [{"line_id": 1001}],
            },
        ]
        with self.assertRaisesRegex(
            VoiceLabProviderError, "candidate_artifact_free_checkpoint_missing"
        ):
            select_best_candidate(candidates)


if __name__ == "__main__":
    unittest.main()

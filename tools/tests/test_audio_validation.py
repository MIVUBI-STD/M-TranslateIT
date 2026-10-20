"""Test only the deterministic, non-model audio fixture and proof contracts."""
from __future__ import annotations

import copy
import sys
import tempfile
import unittest
import json
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parents[1] / "audio_validation"
sys.path.insert(0, str(HERE))
import harness  # noqa: E402


class AudioFixtureContractTests(unittest.TestCase):
    def test_scenarios_bound_to_canonical_corpus(self):
        cases, fingerprint = harness.specs()
        self.assertEqual(len(cases), 48)
        self.assertEqual(len({r["id"] for r in cases}), len(cases))
        self.assertEqual(cases[-1]["id"], "A48")
        self.assertEqual(len(fingerprint), 64)
        self.assertEqual(next(x for x in cases if x["id"] == "A03")["reference"],
                         "Saya tidak setuju dengan perubahan itu.")

    def test_pcm16_roundtrip_and_bounds(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "sample.wav"
            harness.write_wave(path, [0, 123, -123] * 1000)
            data, rate = harness.decode_wave(path)
            self.assertEqual(rate, harness.RATE)
            self.assertEqual(data, [0, 123, -123] * 1000)
            self.assertEqual(harness.metrics(path)["frames"], 3000)
            with self.assertRaises(ValueError):
                harness.write_wave(path, [])

    def test_silence_and_noise_are_reproducible_without_tts(self):
        with tempfile.TemporaryDirectory() as folder:
            for kind in ("silence", "noise", "tone"):
                case = {"id": "A13", "kind": kind, "duration_ms": 100,
                        "noise_amplitude": 0.1, "tone_hz": 220}
                first = harness.generate_samples(case, "", Path(folder), 42)
                second = harness.generate_samples(case, "", Path(folder), 42)
                self.assertEqual(first, second)
                self.assertEqual(len(first), 1600)

    def test_tts_proof_requires_real_preflight_and_non_silent_wav_metrics(self):
        scenario = {
            "id": "A01", "kind": "speech", "expect": "asr_mt_tts",
            "language": "id", "reference": "Halo.", "max_wer": 0.5,
        }
        fixture = {
            "scenario_fingerprint": "a"*64,
            "audio_fingerprint": "b"*64,
            "fixtures": [{"id": "A01", "sha256": "c"*64}],
        }
        stage = {
            "asr": {"ok": True, "transcript_text": "Halo."},
            "translation": {
                "ok": True, "direction_pair": "id->en", "complete": True,
                "finished_with_eos": True, "translated_text": "Hello.",
            },
            "tts": {
                "ok": True, "wav_sha256": "d"*64,
                "duration_ms": 1200, "rms": 0.11,
                "peak": 0.5, "rate_hz": 32000,
            },
        }
        with tempfile.TemporaryDirectory() as folder:
            observations = Path(folder)/"observed.json"
            def graded():
                observations.write_text(json.dumps({
                    "schema": harness.RESULT_SCHEMA,
                    "scenario_fingerprint": "a"*64,
                    "audio_fingerprint": "b"*64,
                    "source_identity": {"local_sha": "1"*40},
                    "results": [{"scenario_id": "A01", "audio_sha256": "c"*64,
                                 "stages": stage}],
                }), encoding="utf-8")
                with patch.object(harness, "inspect", return_value=fixture), \
                     patch.object(harness, "specs", return_value=([scenario], "a"*64)), \
                     patch.object(harness, "load_corpus", return_value={"cases": []}):
                    return harness.score(Path(folder), observations)["results"][0]
            self.assertEqual(graded()["status"], "FAIL")
            stage["voice_preflight"] = {"ok": True}
            self.assertEqual(graded()["status"], "PASS")
            stage["tts"]["rms"] = 0.0
            self.assertEqual(graded()["status"], "FAIL")

    def test_vad_without_canonical_provenance_is_not_accepted(self):
        case = {"id": "A13", "kind": "silence", "expect": "no_speech",
                "language": "id", "reference": ""}
        fixture = {
            "scenario_fingerprint": "a"*64,
            "audio_fingerprint": "b"*64,
            "fixtures": [{"id": "A13", "sha256": "c"*64}],
        }
        with tempfile.TemporaryDirectory() as folder:
            obs = Path(folder)/"observation.json"
            obs.write_text(json.dumps({
                "schema": harness.RESULT_SCHEMA,
                "scenario_fingerprint": "a"*64,
                "audio_fingerprint": "b"*64,
                "source_identity": {"local_sha": "1"*40},
                "results": [{"scenario_id": "A13", "audio_sha256": "c"*64,
                             "stages": {"vad": {"ok": True, "finalized_count": 0}}}],
            }), encoding="utf-8")
            with patch.object(harness, "inspect", return_value=fixture), \
                 patch.object(harness, "specs", return_value=([case], "a"*64)), \
                 patch.object(harness, "load_corpus", return_value={"cases": []}):
                report = harness.score(Path(folder), obs)
            self.assertEqual(report["results"][0]["status"], "UNVERIFIED")

    def test_compare_refuses_audio_substitution(self):
        base = {"schema": harness.SCORE_SCHEMA,
                "scenario_fingerprint": "A"*64,
                "audio_fingerprint": "b"*64,
                "source_identity": {"local_sha": "1"*40},
                "results": [{"id": "A01", "status": "PASS"}]}
        candidate = copy.deepcopy(base)
        candidate["audio_fingerprint"] = "c"*64
        with self.assertRaisesRegex(ValueError, "identical audio bytes"):
            harness.compare(base, candidate)
        candidate["audio_fingerprint"] = base["audio_fingerprint"]
        self.assertFalse(harness.compare(base, candidate)["regressions"])


if __name__ == "__main__":
    unittest.main()

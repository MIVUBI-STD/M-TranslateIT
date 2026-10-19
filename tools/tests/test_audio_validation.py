"""Test only the deterministic, non-model audio fixture and proof contracts."""
from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parents[1] / "audio_validation"
sys.path.insert(0, str(HERE))
import harness  # noqa: E402


class AudioFixtureContractTests(unittest.TestCase):
    def test_scenarios_bound_to_canonical_corpus(self):
        cases, fingerprint = harness.specs()
        self.assertEqual(len(cases), 16)
        self.assertEqual(len({r["id"] for r in cases}), len(cases))
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

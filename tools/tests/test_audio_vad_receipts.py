"""Prevent false VAD proof, swapped WAV identities and incomplete case receipts."""
from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parents[1] / "audio_validation"
sys.path.insert(0, str(HERE))
import harness  # noqa: E402
import vad_runner  # noqa: E402


class AudioVadReceiptTests(unittest.TestCase):
    def setUp(self):
        self.scenarios, self.scenario_hash = harness.specs()
        self.record = {row["id"]: "a" * 64 for row in self.scenarios}
        self.manifest = {
            "schema": harness.FIXTURE_SCHEMA,
            "scenario_fingerprint": self.scenario_hash,
            "audio_fingerprint": "b" * 64,
            "fixtures": [{"id": key, "sha256": value} for key, value in self.record.items()],
        }

    def receipts(self, changed=None):
        lines = []
        for key in self.record:
            value = {
                "schema": vad_runner.RECEIPT_SCHEMA, "scenario_id": key,
                "audio_fingerprint": self.manifest["audio_fingerprint"],
                "scenario_fingerprint": self.scenario_hash,
                "audio_sha256": self.record[key],
                "stage": {
                    "ok": True, "finalized_count": 1, "overflow_count": 0,
                    "writer_ok": True, "claim": vad_runner.CLAIM, "test_only": True,
                },
            }
            if key == "A01" and changed:
                changed(value)
            lines.append(vad_runner.PREFIX + __import__("json").dumps(value))
        return "\n".join(lines)

    def test_complete_replay_receipts_are_bound_to_original_wavs(self):
        result = vad_runner.parse_receipts(
            self.receipts(), self.manifest, self.scenario_hash
        )
        self.assertEqual(len(result), len(self.scenarios))
        self.assertEqual(result["A01"]["finalized_count"], 1)

    def test_modified_audio_hash_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "unbound"):
            vad_runner.parse_receipts(
                self.receipts(lambda r: r.__setitem__("audio_sha256", "c"*64)),
                self.manifest, self.scenario_hash
            )

    def test_incomplete_receipt_set_is_not_a_pass(self):
        one = self.receipts().split("\n")[:-1]
        with self.assertRaisesRegex(ValueError, "incomplete"):
            vad_runner.parse_receipts("\n".join(one), self.manifest, self.scenario_hash)

    def test_false_proof_flag_is_not_trusted(self):
        with self.assertRaisesRegex(ValueError, "invalid Rust VAD"):
            vad_runner.parse_receipts(
                self.receipts(lambda r: r["stage"].__setitem__("test_only", False)),
                self.manifest, self.scenario_hash
            )

    def test_merge_requires_exact_worker_identity_and_no_duplicate_vad(self):
        source_sha = "1"*40
        stage = vad_runner.parse_receipts(
            self.receipts(), self.manifest, self.scenario_hash
        )
        output = vad_runner.make_observations(
            source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, None
        )
        self.assertEqual(output["results"][0]["stages"]["vad"]["claim"], vad_runner.CLAIM)
        bad = copy.deepcopy(output)
        bad["source_identity"]["local_sha"] = "2"*40
        with self.assertRaisesRegex(ValueError, "different source"):
            vad_runner.make_observations(
                source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, bad
            )
        good = copy.deepcopy(output)
        good["source_identity"]["local_sha"] = source_sha
        with self.assertRaisesRegex(ValueError, "duplicate VAD"):
            vad_runner.make_observations(
                source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, good
            )


if __name__ == "__main__":
    unittest.main()

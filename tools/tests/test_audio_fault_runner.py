"""Offline audio protocol and negative-proof smoke; no model or CI needed."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "audio_validation"))
from fault_runner import RUNNERS, execute, matrix_cases  # noqa: E402
from worker_runner import WorkerTransportError, validate_response_frame  # noqa: E402


class OfflineAudioFaultContracts(unittest.TestCase):
    def test_every_planned_fault_has_explicit_execution_state(self):
        matrix = matrix_cases()
        self.assertEqual(len(matrix), 28)
        result = execute(set())
        self.assertEqual(result["counts"]["PASS"], 0)
        self.assertEqual(result["counts"]["NOT_EXECUTED"], 28)

    def test_actual_offline_injectors_run_and_record_partial_scope(self):
        selected = {row["id"] for row in matrix_cases() if row["slug"] in RUNNERS}
        self.assertEqual(len(selected), 3)
        report = execute(selected)
        self.assertEqual(report["counts"]["PASS"], 3)
        self.assertEqual(report["counts"]["FAIL"], 0)
        self.assertEqual(report["counts"]["NOT_EXECUTED"], 25)
        self.assertTrue(all(r["proof"] == "OFFLINE_CONTRACT_ONLY_NOT_NATIVE"
                            for r in report["cases"] if r["status"] == "PASS"))

    def test_refuse_invalid_worker_reply(self):
        for raw in (None, b"", b"bad\n", b'{"ok":"true"}\n',
                    b'{"ok":true,"stage":"ping","request_id":"old"}\n'):
            with self.assertRaises(WorkerTransportError):
                validate_response_frame(raw, request_id="fixture-1", command="ping")

    def test_unknown_fault_id_is_rejected(self):
        with self.assertRaises(ValueError):
            execute({"F99"})


if __name__ == "__main__":
    unittest.main()

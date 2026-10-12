"""Source-contract tests for opt-in REA CLI boundary (no REA installed)."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_rea import (ReaBridgeError, _analyze, _command, _doctor,
                            _target_and_output, main, plan)


class ReaBoundaryTests(unittest.TestCase):
    def test_planning_never_executes(self):
        self.assertEqual(plan("doctor", "6.4.0", None)["status"], "PLANNED_NOT_EXECUTED")
        with patch("repository_rea.subprocess.run") as called:
            with patch.object(sys, "argv", ["repository_rea.py", "doctor"]):
                self.assertEqual(main(), 0)
            called.assert_not_called()

    def test_windows_batch_launcher_is_not_executed(self):
        with patch("repository_rea.shutil.which", return_value=r"C:\\npm\\rea.cmd"):
            with self.assertRaises(ReaBridgeError):
                _command(None)

    def test_version_and_report_shape_must_match(self):
        result = {
            "healthy": True,
            "scope_checks": [{"name": "node", "ok": True}],
            "identity": {"cli_package_version": "6.4.0"},
        }
        ok = subprocess.CompletedProcess(["rea"], 0, json.dumps(result).encode(), b"")
        with patch("repository_rea.subprocess.run", return_value=ok):
            self.assertTrue(_doctor(["rea"], "6.4.0", None, None)["healthy"])
            with self.assertRaises(ReaBridgeError):
                _doctor(["rea"], "6.3.0", None, None)
        bad = subprocess.CompletedProcess(["rea"], 0, b"not JSON", b"")
        with patch("repository_rea.subprocess.run", return_value=bad):
            with self.assertRaises(ReaBridgeError):
                _doctor(["rea"], "6.4.0", None, None)

    def test_private_output_and_explicit_target_requirements(self):
        with tempfile.TemporaryDirectory() as private:
            target = Path(private) / "application.asar"
            target.write_bytes(b"fake")
            output = Path(private) / "evidence.json"
            self.assertEqual(_target_and_output(str(target), str(output))[1], output)
            with self.assertRaises(ReaBridgeError):
                _target_and_output(str(target), str(ROOT / "evidence.json"))
            output.write_text("do not overwrite", encoding="utf-8")
            with self.assertRaises(ReaBridgeError):
                _target_and_output(str(target), str(output))
        with patch("repository_rea.subprocess.run") as called:
            with patch.object(sys, "argv",
                              ["repository_rea.py", "analyze-js", "--target", "/missing",
                               "--output", "/missing/evidence.json", "--execute"]):
                self.assertEqual(main(), 2)
            called.assert_not_called()

    def test_analysis_retains_only_private_digest_and_removes_failed_output(self):
        with tempfile.TemporaryDirectory() as private:
            target = Path(private) / "app.asar"
            target.write_bytes(b"fake")
            output = Path(private) / "result.json"
            def write_evidence(*args, **kwargs):
                kwargs["stdout"].write(b'{"evidence":"private value"}')
                return subprocess.CompletedProcess(args[0], 0)
            with patch("repository_rea.subprocess.run", side_effect=write_evidence):
                report = _analyze(["rea"], target, output)
            self.assertEqual(report["status"], "REA_ARTIFACT_CAPTURED_UNREVIEWED")
            self.assertNotIn("private value", str(report))
            self.assertTrue(output.exists())
            output.unlink()
            with patch("repository_rea.subprocess.run",
                       return_value=subprocess.CompletedProcess(["rea"], 1)):
                with self.assertRaises(ReaBridgeError):
                    _analyze(["rea"], target, output)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()

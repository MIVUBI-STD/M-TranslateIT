"""Source-contract tests for opt-in REA CLI boundary (no REA installed)."""
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_rea import (ReaBridgeError, _analyze, _bounded_stdout, _command,
                            _doctor, _target_and_output, main, plan)


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
        def fake_doctor(argv, output, limit, seconds):
            output.write(json.dumps(result).encode("utf-8"))
            return 0
        with patch("repository_rea._bounded_stdout", side_effect=fake_doctor):
            self.assertTrue(_doctor(["rea"], "6.4.0", None, None)["healthy"])
            with self.assertRaises(ReaBridgeError):
                _doctor(["rea"], "6.3.0", None, None)
        def malformed(argv, output, limit, seconds):
            output.write(b"invalid JSON")
            return 0
        with patch("repository_rea._bounded_stdout", side_effect=malformed):
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

    def test_optional_provider_doctor_failure_does_not_block_static_js(self):
        with tempfile.TemporaryDirectory() as private:
            target = Path(private) / "app.asar"
            target.write_bytes(b"fake")
            output = Path(private) / "result.json"
            unhealthy = {"healthy": False, "version": "6.4.0"}
            with patch("repository_rea._doctor", return_value=unhealthy):
                with patch("repository_rea._command", return_value=["rea"]):
                    with patch("repository_rea._analyze", return_value={
                        "status": "REA_ARTIFACT_CAPTURED_UNREVIEWED"
                    }) as analyze:
                        with patch.object(sys, "argv", [
                            "repository_rea.py", "analyze-js",
                            "--target", str(target), "--output", str(output),
                            "--target-authorized", "--execute",
                        ]):
                            self.assertEqual(main(), 0)
                        analyze.assert_called_once()
                        actual = analyze.call_args.args
                        self.assertEqual(actual[0], ["rea"])
                        self.assertEqual(actual[1].resolve(), target.resolve())
                        self.assertEqual(actual[2].resolve(), output.resolve())

    def test_invalid_success_json_is_rejected_and_cleaned_up(self):
        with tempfile.TemporaryDirectory() as private:
            target = Path(private) / "app.asar"
            target.write_bytes(b"fake")
            output = Path(private) / "result.json"
            def write_invalid(argv, output, limit, seconds):
                output.write(b'{"broken":')
                return 0
            with patch("repository_rea._bounded_stdout", side_effect=write_invalid):
                with self.assertRaises(ReaBridgeError):
                    _analyze(["rea"], target, output)
            self.assertFalse(output.exists())
            def write_error(argv, output, limit, seconds):
                output.write(b'{"error":"no provider"}')
                return 0
            with patch("repository_rea._bounded_stdout", side_effect=write_error):
                with self.assertRaises(ReaBridgeError):
                    _analyze(["rea"], target, output)
            self.assertFalse(output.exists())


    def test_rea_script_identity_requires_installed_package(self):
        with tempfile.TemporaryDirectory() as private:
            base = Path(private)
            scripts = base / "scripts"
            scripts.mkdir()
            entry = scripts / "rea.mjs"
            entry.write_text("", encoding="utf-8")
            with self.assertRaises(ReaBridgeError):
                _command(str(entry))
            (base / "package.json").write_text(
                json.dumps({"name": "not-rea-agents"}), encoding="utf-8"
            )
            with self.assertRaises(ReaBridgeError):
                _command(str(entry))
            (base / "package.json").write_text(
                json.dumps({"name": "rea-agents", "version": "6.4.0"}),
                encoding="utf-8"
            )
            with patch("repository_rea.shutil.which", return_value="/approved/node"):
                self.assertEqual(Path(_command(str(entry))[1]).resolve(), entry.resolve())

    def test_analysis_retains_only_private_digest_and_removes_failed_output(self):
        with tempfile.TemporaryDirectory() as private:
            target = Path(private) / "app.asar"
            target.write_bytes(b"fake")
            output = Path(private) / "result.json"
            def write_evidence(argv, output, limit, seconds):
                output.write(b'{"evidence":"private value"}')
                return 0
            with patch("repository_rea._bounded_stdout", side_effect=write_evidence):
                report = _analyze(["rea"], target, output)
            self.assertEqual(report["status"], "REA_ARTIFACT_CAPTURED_UNREVIEWED")
            self.assertNotIn("private value", str(report))
            self.assertNotIn(str(output), str(report))
            self.assertEqual(report["jsonSyntax"], "VALID_JSON_DOCUMENT")
            self.assertTrue(output.exists())
            output.unlink()
            with patch("repository_rea._bounded_stdout", return_value=1):
                with self.assertRaises(ReaBridgeError):
                    _analyze(["rea"], target, output)
            self.assertFalse(output.exists())

    def test_stdout_limit_enforced_during_execution(self):
        output = io.BytesIO()
        command = [sys.executable, "-c", "import sys;sys.stdout.buffer.write(b'X'*131072)"]
        with self.assertRaises(ReaBridgeError):
            _bounded_stdout(command, output, 1024, 10)
        self.assertLessEqual(len(output.getvalue()), 1024)

    def test_deadline_terminates_silent_child(self):
        output = io.BytesIO()
        with self.assertRaises(ReaBridgeError):
            _bounded_stdout([sys.executable, "-c", "import time;time.sleep(10)"],
                            output, 1024, 0.3)

    def test_mcp_probe_is_not_started_without_execute(self):
        with patch("repository_rea.subprocess.Popen") as launch:
            with patch.object(sys, "argv", ["repository_rea.py", "mcp-probe"]):
                self.assertEqual(main(), 0)
            launch.assert_not_called()


if __name__ == "__main__":
    unittest.main()

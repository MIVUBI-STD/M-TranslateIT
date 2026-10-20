"""Read-only synthesis preflight contracts; never require local voices in CI."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "audio_validation"))
from generation_doctor import doctor  # noqa: E402


class AudioGenerationDoctorTests(unittest.TestCase):
    def test_missing_executable_fails_closed(self):
        with patch("generation_doctor.shutil.which", return_value=None):
            result = doctor("missing-offline-tts")
        self.assertFalse(result["ok"])
        self.assertEqual(result["case_count"], 48)
        self.assertEqual(result["blocker"], "offline_tts_executable_missing")

    def test_generator_refuses_to_write_when_language_is_missing(self):
        import tempfile
        from pathlib import Path
        import harness
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "official-fixtures"
            with patch.object(harness.shutil, "which", return_value="/local/tts"), \
                 patch("generation_doctor.doctor", return_value={
                     "ok": False, "generator": {"version": "unavailable",
                                                "executable": "/local/tts"}
                 }):
                with self.assertRaisesRegex(ValueError, "required language"):
                    harness.generate(output, "eSpeak", 42)
            self.assertFalse(output.exists())

    def test_missing_language_voice_fails_closed(self):
        class Result:
            returncode = 0
            stdout = "Pty Language Age/Gender VoiceName File Other Languages\n 5 en M english en"
        with patch("generation_doctor.shutil.which", return_value="/local/tts"), \
             patch("generation_doctor.subprocess.run", return_value=Result()):
            result = doctor("eSpeak")
        self.assertFalse(result["ok"])
        self.assertEqual(result["blocker"], "required_offline_tts_voice_missing")


if __name__ == "__main__":
    unittest.main()

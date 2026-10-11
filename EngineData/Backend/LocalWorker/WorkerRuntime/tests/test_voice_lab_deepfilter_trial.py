"""Source-only regression cases; these do not execute the DeepFilterNet binary."""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from scipy.signal import resample_poly

from test_voice_lab_source_pair import tone, wav
from voice_lab_deepfilter_trial import (
    ATTENUATION_DB, EXECUTABLE_NAME, MODEL_NAME, deepfilter_command,
    frozen_sources, inspect_enhanced, prepare_48k, verify_asset,
)
from voice_lab_source_pair import inspect_frozen_source_pair


class DeepFilterTrialContracts(unittest.TestCase):
    def test_assets_require_exact_filename_and_sha(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / EXECUTABLE_NAME
            exe.write_bytes(b"fixture executable bytes")
            digest = hashlib.sha256(exe.read_bytes()).hexdigest()
            self.assertEqual(verify_asset(exe, EXECUTABLE_NAME, digest, 256), digest)
            with self.assertRaisesRegex(ValueError, "asset_checksum_mismatch"):
                verify_asset(exe, EXECUTABLE_NAME, "0" * 64, 256)
            with self.assertRaisesRegex(ValueError, "asset_identity_invalid"):
                verify_asset(exe, MODEL_NAME, digest, 256)

    def test_prepares_48k_without_changing_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "take_0001.wav"
            out_file = root / "trial" / "take_0001.wav"
            wav(source, tone(32_000), 32_000)
            original = source.read_bytes()
            metadata = prepare_48k(source, out_file)
            self.assertEqual(metadata["source_rate_hz"], 32_000)
            self.assertEqual(original, source.read_bytes())
            import wave
            with wave.open(str(out_file), "rb") as reader:
                self.assertEqual(reader.getframerate(), 48_000)
                self.assertEqual(reader.getsampwidth(), 2)

    def _frozen(self, root):
        native = root / "NativeRate" / "take_0001.wav"
        canonical = root / "take_0001.wav"
        s = tone(48_000)
        wav(native, s, 48_000)
        target = np.rint(resample_poly(s.astype(np.float64) / 32768, 2, 3) * 32767)
        wav(canonical, target.astype(np.int16), 32_000)
        evidence = inspect_frozen_source_pair(canonical, native)
        (root / "dataset.json").write_text(json.dumps({
            "schema_version": 1, "authorized_voice_confirmed": True,
            "takes": [{"line_id": 1, "wav_file": canonical.name, "exact_text": "private"}],
        }), encoding="utf-8")
        (root / "audio_quality.json").write_text(json.dumps({
            "schema": "voice_lab.audio_quality.v1",
            "takes": [{"wav_file": canonical.name, "source_pair": evidence}],
        }), encoding="utf-8")
        return native

    def test_frozen_pair_is_required_and_hash_change_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            native = self._frozen(root)
            self.assertEqual(frozen_sources(root, [1])[0]["line_id"], 1)
            native.write_bytes(native.read_bytes() + b"changed")
            with self.assertRaisesRegex(ValueError, "source_pair_not_verified"):
                frozen_sources(root, [1])
            with self.assertRaisesRegex(ValueError, "trial_selection_invalid"):
                frozen_sources(root, [1, 1])

    def test_command_uses_explicit_v3_model_and_no_postfilter(self):
        model = Path("controlled") / MODEL_NAME
        binary = Path("controlled") / EXECUTABLE_NAME
        cmd = deepfilter_command(binary, model, Path("out"), [Path("take_0001.wav")])
        self.assertEqual(cmd[1:3], ["--model", str(model)])
        self.assertEqual(cmd[3:5], ["--atten-lim-db", str(ATTENUATION_DB)])
        self.assertNotIn("--pf", cmd)
        self.assertNotIn("--compensate-delay", cmd)

    def test_output_is_review_only_and_always_separate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            raw = root / "input.wav"
            enhanced = root / "enhanced.wav"
            candidate = root / "candidate.wav"
            wav(raw, tone(48_000), 48_000)
            wav(enhanced, tone(48_000), 48_000)
            info = inspect_enhanced(raw, enhanced, candidate)
            self.assertEqual(info["status"], "requires_human_voice_review")
            self.assertFalse(info["training_approved"])
            self.assertEqual(len(info["derived_32k_sha256"]), 64)
            self.assertTrue(candidate.exists())

    def test_silent_or_short_enhanced_audio_does_not_create_candidate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            raw = root / "input.wav"
            enhanced = root / "enhanced.wav"
            candidate = root / "candidate.wav"
            wav(raw, tone(48_000), 48_000)
            wav(enhanced, np.zeros(48_000, dtype=np.int16), 48_000)
            with self.assertRaisesRegex(ValueError, "enhanced_signal_unusable"):
                inspect_enhanced(raw, enhanced, candidate)
            wav(enhanced, tone(48_000)[:24_000], 48_000)
            with self.assertRaisesRegex(ValueError, "enhanced_signal_unusable"):
                inspect_enhanced(raw, enhanced, candidate)
            self.assertFalse(candidate.exists())


if __name__ == "__main__":
    unittest.main()

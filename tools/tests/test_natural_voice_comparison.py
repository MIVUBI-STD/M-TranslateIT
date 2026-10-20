"""No-network contracts for the opt-in natural Indonesian comparison."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "audio_validation"))
from natural_voice_comparison import CASES, VOICES, plan, ssml, generate  # noqa: E402


class NaturalVoiceComparisonContracts(unittest.TestCase):
    def test_plan_only_picks_five_canonical_public_sentences(self):
        p = plan()
        self.assertEqual(p["requests"], 10)
        self.assertEqual([x["id"] for x in p["samples"]], list(CASES))
        self.assertEqual(len(p["voices"]), 2)

    def test_escapes_ssml_and_refuses_unlisted_voices(self):
        self.assertIn(b"A &amp; B", ssml("A & B", VOICES[0]))
        with self.assertRaises(ValueError):
            ssml("A", "en-US-Unknown")

    def test_missing_credentials_block_without_network(self):
        with patch("natural_voice_comparison.synthesize") as synth:
            with self.assertRaises(ValueError):
                generate(Path("unused-public-audio"), region="invalid region", key="")
            synth.assert_not_called()


if __name__ == "__main__":
    unittest.main()

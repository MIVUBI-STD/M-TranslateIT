"""Regression checks for TranslateIT ChatGPT/cloud-only development policy."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import verify_repository


class CloudOnlyGovernanceTests(unittest.TestCase):
    def test_current_candidate_and_skill_contracts(self):
        errors: list[str] = []
        verify_repository.check_chatgpt_execution_policy(errors)
        self.assertEqual(errors, [])

    def test_removing_no_user_pc_rule_is_rejected(self):
        real_text = verify_repository.text

        def invalid(rel: str) -> str:
            value = real_text(rel)
            if rel == "GITHUB_RULES.md":
                return value.replace("Never ask the user to run", "Ask the user to run", 1)
            return value

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=invalid):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("Never ask the user to run" in error for error in errors))

    def test_removing_candidate_admission_is_rejected(self):
        real_text = verify_repository.text

        def invalid(rel: str) -> str:
            value = real_text(rel)
            if rel == "docs/knowledge/operations/target-windows-performance.md":
                return value.replace("## Candidate admission", "## Premature manual testing", 1)
            return value

        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=invalid):
            verify_repository.check_chatgpt_execution_policy(errors)
        self.assertTrue(any("## Candidate admission" in error for error in errors))


if __name__ == "__main__":
    unittest.main()

"""Regression corpus for repository agent preflight."""
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_permissions import evaluate_permission, load_policy  # noqa: E402


class PermissionTests(unittest.TestCase):
    def test_golden_policy_cases(self):
        corpus = json.loads((ROOT / ".agents/evals/permission-cases.json").read_text(encoding="utf-8"))
        policy = load_policy()
        for case in corpus["cases"]:
            with self.subTest(case=case["id"]):
                self.assertEqual(evaluate_permission(case["request"], policy)["decision"], case["expected"])

    def test_policy_is_closed_for_unknown_actions(self):
        self.assertEqual(
            evaluate_permission({"mode": "standard-development", "action": "delete", "scope": "governance", "path": "README.md"})["decision"],
            "deny",
        )

    def test_user_data_is_not_published_through_governance(self):
        self.assertEqual(
            evaluate_permission({"mode": "complex-development", "action": "write", "scope": "governance", "path": "UserData/CacheData/speech.wav"})["decision"],
            "deny",
        )

    def test_hosted_execution_is_denied_even_in_development_mode(self):
        for action in ("ci-dispatch", "hosted-compute", "cloud-inference"):
            with self.subTest(action=action):
                outcome = evaluate_permission(
                    {"mode": "complex-development", "action": action, "scope": "governance"}
                )
                self.assertEqual(outcome["decision"], "deny")

    def test_work_mode_cannot_escalate_with_scope(self):
        self.assertEqual(
            evaluate_permission({"mode": "plan", "action": "ci-dispatch", "scope": "governance"})["decision"],
            "deny",
        )


if __name__ == "__main__":
    unittest.main()

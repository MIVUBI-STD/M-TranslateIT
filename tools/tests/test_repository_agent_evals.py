"""Agent routing corpus: identity, negative cases and honest observed-result scoring."""
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_agent_evals import score_agent_runs, validate_agent_evals  # noqa: E402


class AgentRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = json.loads((ROOT / ".agents/evals/skill-routing.json").read_text(encoding="utf-8"))["cases"]

    def test_routing_corpus_matches_canonical_inventory_and_modes(self):
        self.assertEqual(validate_agent_evals(), [])

    def test_missing_model_observations_cannot_be_called_a_pass(self):
        self.assertFalse(score_agent_runs({"schemaVersion": 1, "runs": []})["passed"])

    def test_correct_synthetic_receipt_validates_scorer_only(self):
        # These are synthetic scorer fixtures, not observed model decisions.
        receipts = [{"id": item["id"], "mode": item["expectedMode"],
                     "specialist": item["expectedSpecialist"],
                     "scope": item["expectedScope"],
                     "requiresBrief": item["requiresBrief"]} for item in self.cases]
        result = score_agent_runs({"schemaVersion": 1, "runs": receipts})
        self.assertTrue(result["passed"])
        self.assertTrue(result["complete"])

    def test_forbidden_specialist_fails_closed(self):
        case = next(item for item in self.cases if item["expectedSpecialist"] is None)
        result = score_agent_runs({"schemaVersion": 1, "runs": [{
            "id": case["id"], "mode": case["expectedMode"],
            "specialist": case["forbiddenSpecialists"][0],
            "scope": case["expectedScope"],
            "requiresBrief": case["requiresBrief"],
        }]})
        self.assertFalse(result["passed"])
        self.assertFalse(result["failures"][0]["forbidden"])

    def test_duplicate_observation_rejected(self):
        case = self.cases[0]
        duplicate = {"id": case["id"], "mode": case["expectedMode"],
                     "specialist": case["expectedSpecialist"],
                     "scope": case["expectedScope"],
                     "requiresBrief": case["requiresBrief"]}
        with self.assertRaises(ValueError):
            score_agent_runs({"schemaVersion": 1, "runs": [duplicate, duplicate]})


if __name__ == "__main__":
    unittest.main()

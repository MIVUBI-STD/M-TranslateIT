"""Agent routing corpus: identity, negative cases and honest observed-result scoring."""
import json
import copy
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_agent_evals import (  # noqa: E402
    score_agent_runs, score_procedure_runs, validate_agent_evals, validate_agent_procedures,
)


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


    @staticmethod
    def procedure_fixture(case):
        return {
            "id": case["id"],
            "selectedSkill": case["skill"],
            "observedMilestones": case["requiredMilestones"][:],
            "observedActions": [],
            "disposition": case["expectedDisposition"],
            "handoffTarget": case["handoffTarget"],
            "evidenceRef": "fixture://synthetic-agent-run/" + case["id"],
        }

    def test_procedure_corpus_has_correct_owner_links(self):
        self.assertEqual(validate_agent_procedures(), [])
        corpus = json.loads((ROOT / ".agents/evals/skill-procedure.json").read_text(
            encoding="utf-8",
        ))
        self.assertGreaterEqual(len(corpus["cases"]), 16)
        self.assertTrue(any(c["skill"] is None for c in corpus["cases"]))
        self.assertTrue(any(c["expectedDisposition"] == "PROPOSE_HANDOFF"
                            for c in corpus["cases"]))

    def test_procedure_no_observed_runs_is_not_pass(self):
        score = score_procedure_runs({"schemaVersion": 1, "runs": []})
        self.assertFalse(score["passed"])
        self.assertEqual(score["proof"], "EXTERNAL_RECEIPT_CONSISTENCY_ONLY")
        self.assertFalse(score["observedAgentBehaviorVerified"])

    def test_synthetic_procedure_receipts_only_test_the_scorer(self):
        corpus = json.loads((ROOT / ".agents/evals/skill-procedure.json").read_text(
            encoding="utf-8",
        ))["cases"]
        receipts = [self.procedure_fixture(case) for case in corpus]
        outcome = score_procedure_runs({"schemaVersion": 1, "runs": receipts})
        self.assertTrue(outcome["passed"])
        self.assertTrue(outcome["complete"])
        self.assertEqual(outcome["coverage"], 1.0)
        self.assertEqual(outcome["receiptConsistency"], 1.0)
        self.assertFalse(outcome["observedAgentBehaviorVerified"])
        self.assertFalse(outcome["ciTriggered"])

    def test_procedure_missing_milestone_fails(self):
        case = self.procedure_case("desktop-readiness")
        receipt = self.procedure_fixture(case)
        receipt["observedMilestones"] = []
        result = score_procedure_runs({"schemaVersion": 1, "runs": [receipt]})
        self.assertFalse(result["failures"][0]["required"])
        self.assertFalse(result["passed"])

    def test_procedure_forbidden_action_fails(self):
        case = self.procedure_case("release-controlled")
        receipt = self.procedure_fixture(case)
        receipt["observedActions"] = [case["forbiddenActions"][0]]
        result = score_procedure_runs({"schemaVersion": 1, "runs": [receipt]})
        self.assertFalse(result["failures"][0]["forbidden"])

    def test_procedure_wrong_skill_or_handoff_fails(self):
        case = self.procedure_case("desktop-audio-handoff")
        receipt = self.procedure_fixture(case)
        receipt["selectedSkill"] = "windows-audio-runtime-development"
        receipt["handoffTarget"] = "desktop-runtime-development"
        result = score_procedure_runs({"schemaVersion": 1, "runs": [receipt]})
        self.assertFalse(result["failures"][0]["skill"])
        self.assertFalse(result["failures"][0]["handoff"])

    def test_procedure_receipt_cannot_invent_evidence(self):
        case = self.procedure_case("brief-cross-owner")
        receipt = self.procedure_fixture(case)
        receipt["evidenceRef"] = ""
        result = score_procedure_runs({"schemaVersion": 1, "runs": [receipt]})
        self.assertFalse(result["failures"][0]["evidence"])

    def test_duplicate_procedure_receipt_fails_closed(self):
        case = self.procedure_case("brief-cross-owner")
        receipt = self.procedure_fixture(case)
        with self.assertRaises(ValueError):
            score_procedure_runs({"schemaVersion": 1, "runs": [receipt, receipt]})

    def test_invalid_procedure_owner_or_route_is_detected(self):
        corpus = json.loads((ROOT / ".agents/evals/skill-procedure.json").read_text(
            encoding="utf-8",
        ))
        wrong = copy.deepcopy(corpus)
        wrong["cases"][0]["skill"] = "unregistered-qa-expert"
        self.assertTrue(any("not registered" in message for message in
                            validate_agent_procedures(corpus=wrong)))
        wrong = copy.deepcopy(corpus)
        wrong["cases"][0]["routingCaseId"] = "invented-route"
        self.assertTrue(any("routing case" in message for message in
                            validate_agent_procedures(corpus=wrong)))

    def test_procedure_handoff_requires_different_existing_specialist(self):
        corpus = json.loads((ROOT / ".agents/evals/skill-procedure.json").read_text(
            encoding="utf-8",
        ))
        wrong = copy.deepcopy(corpus)
        case = next(c for c in wrong["cases"] if c["id"] == "desktop-audio-handoff")
        case["handoffTarget"] = case["skill"]
        self.assertTrue(any("different specialist" in msg for msg in
                            validate_agent_procedures(corpus=wrong)))

    @staticmethod
    def procedure_case(key):
        cases = json.loads((ROOT / ".agents/evals/skill-procedure.json").read_text(
            encoding="utf-8",
        ))["cases"]
        return next(case for case in cases if case["id"] == key)



if __name__ == "__main__":
    unittest.main()

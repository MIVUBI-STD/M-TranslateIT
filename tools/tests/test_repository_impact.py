"""Conservative affected-proof planning: no unsafe skip, no CI dispatch."""
import unittest
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_impact import plan_impacted, verify_impact  # noqa: E402

FRONT = "EngineData/Frontend/RustApp/"
WORKER = "EngineData/Backend/LocalWorker/WorkerRuntime/"


class ImpactPlanningTests(unittest.TestCase):
    def test_current_impact_contract(self):
        self.assertEqual(verify_impact(), [])

    def test_governance_only_does_not_require_runtime_proof(self):
        plan = plan_impacted(["docs/knowledge/flow.md"])
        self.assertEqual(plan["domains"], ["repository"])
        self.assertEqual(plan["manualWorkflowCandidates"], ["repository-verify.yml"])
        self.assertFalse(plan["ciTriggered"])

    def test_registered_ipc_boundary_routes_to_both_sides_and_existing_tests(self):
        plan = plan_impacted([FRONT + "src-tauri/src/commands/registry.rs"])
        self.assertIn("tauri-command-bridge", plan["paths"][0]["interopContracts"])
        self.assertTrue({"frontend", "rust"}.issubset(plan["domains"]))
        self.assertIn("validate:bridge-contract", plan["existingFrontendValidators"])
        self.assertIn("code-health.yml", plan["manualWorkflowCandidates"])

    def test_worker_wire_protocol_does_not_skip_frontend_or_rust(self):
        plan = plan_impacted([WORKER + "worker_io_runtime.py"])
        self.assertIn("local-worker-wire-framing", plan["paths"][0]["interopContracts"])
        self.assertTrue({"frontend", "rust", "python"}.issubset(plan["domains"]))
        self.assertTrue(any(path.endswith("test_worker_protocol_framing.py") for path in plan["knownRegressionTests"]))
        self.assertEqual(plan["proof"], "PLANNING_ONLY_NOT_EXECUTED")

    def test_unregistered_source_cannot_claim_complete_dependency_closure(self):
        plan = plan_impacted([FRONT + "src/app/runtime/textTranslationCache.ts"])
        self.assertTrue(plan["reviewRequired"])
        self.assertEqual(plan["dependencyCoverage"], "CONSERVATIVE")
        self.assertIn("frontend", plan["domains"])

    def test_unknown_source_escalates_to_full_review(self):
        plan = plan_impacted(["NewProductionDomain/kernel.rs"])
        self.assertTrue(plan["reviewRequired"])
        self.assertTrue({"frontend", "rust", "python", "release", "repository"}.issubset(plan["domains"]))

    def test_input_does_not_allow_path_escape_or_aliases(self):
        for bad in ("../secret", "/etc/passwd", "docs/../../AGENTS.md", r"docs\README.md", "", "docs/a//b"):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                plan_impacted([bad])

    def test_duplicate_paths_and_order_are_deterministic(self):
        a = plan_impacted(["docs/knowledge/flow.md", "AGENTS.md"])
        b = plan_impacted(["AGENTS.md", "docs/knowledge/flow.md", "AGENTS.md"])
        self.assertEqual(a, b)


if __name__ == "__main__":
    unittest.main()

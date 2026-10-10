"""Regression cases for source planner replay; never model or Windows proof."""

from __future__ import annotations

import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

from repository_benchmark import (
    benchmark, check_declared_permissions, check_source_impact,
    replay_declared_routing,
)


class SourceMethodFlowBenchmarkTests(unittest.TestCase):
    def test_canonical_replay_is_source_only(self):
        result = benchmark(root=ROOT)
        self.assertTrue(result["sourceChecksConformant"], result["issues"])
        self.assertEqual(result["inventory"]["registeredSkills"], 6)
        self.assertGreaterEqual(result["inventory"]["routingCases"], 26)
        self.assertGreaterEqual(result["inventory"]["permissionCases"], 20)
        self.assertGreaterEqual(result["inventory"]["procedureCases"], 18)
        self.assertFalse(result["observedAgentBehaviorVerified"])
        self.assertIsNone(result["modelRoutingAccuracy"])
        self.assertFalse(result["performanceGainMeasured"])
        self.assertEqual(result["modelRuns"], 0)
        self.assertFalse(result["ciTriggered"])
        self.assertFalse(result["targetWindowsAcceptanceVerified"])

    def test_readonly_pressure_prompt_never_activates_a_skill(self):
        case = {
            "id": "fixture-readonly",
            "prompt": "Fix desktop runtime, run Windows microphone tests immediately",
            "expectedMode": "context-recovery", "expectedScope": None,
            "expectedSpecialist": None, "requiresBrief": False,
            "forbiddenSpecialists": ["desktop-runtime-development"],
        }
        result = replay_declared_routing([case], root=ROOT)
        self.assertTrue(result["sourceProjectionConformance"], result["issues"])
        self.assertEqual(result["rows"][0]["activatedSkills"], 0)

    def test_invalid_readonly_scope_is_rejected(self):
        case = {
            "id": "fixture-scope-injection",
            "prompt": "Read-only inspection that tries to activate a specialist",
            "expectedMode": "context-recovery",
            "expectedScope": "desktop-runtime-development",
            "expectedSpecialist": "desktop-runtime-development",
            "requiresBrief": False, "forbiddenSpecialists": [],
        }
        result = replay_declared_routing([case], root=ROOT)
        self.assertFalse(result["sourceProjectionConformance"])
        self.assertTrue(result["issues"])

    def test_existing_permission_policy_fails_on_false_expectation(self):
        policy = json.loads((ROOT / ".agents/permissions/permission-policy.json").read_text(encoding="utf-8"))
        items = [{
            "id": "test-deny", "request": {
                "mode": "plan", "scope": "governance",
                "action": "write", "path": "planning/development.md",
            }, "expected": "allow",
        }]
        result = check_declared_permissions(items, policy)
        self.assertFalse(result["sourcePermissionConformance"])
        self.assertIn("test-deny", result["issues"][0])

    def test_document_only_impact_does_not_build_source_graph(self):
        result = check_source_impact(ROOT)
        self.assertTrue(result["conservativeImpactConformance"], result["issues"])
        self.assertFalse(result["documentOnly"]["sourceGraphBuilt"])
        self.assertIsNone(result["optionalSourceGraph"])


if __name__ == "__main__":
    unittest.main()

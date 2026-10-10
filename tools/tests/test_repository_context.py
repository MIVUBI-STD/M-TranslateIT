"""Source-grounded context flow; model routing remains externally evaluated."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

from repository_context import BRIEF, CONTINUATION, plan_context, verify_context  # noqa: E402


class DevelopmentContextTests(unittest.TestCase):
    def test_integrated_smoke(self):
        self.assertEqual(verify_context(), [])

    def test_inspect_requires_root_only_and_no_unrelated_skills(self):
        p = plan_context(intent="Amati project", mode="context-recovery")
        self.assertEqual(p["task"], "INSPECT")
        self.assertEqual(p["lane"], "READ_ONLY")
        self.assertEqual(p["context"]["REQUIRED"], ["AGENTS.md", "GITHUB_RULES.md"])
        self.assertIsNone(p["activeSpecialist"])
        self.assertIsNone(p["knownImpact"])
        self.assertIsNone(p["rankedKnowledge"])
        self.assertFalse(p["ciTriggered"])
        self.assertEqual(p["execution"], "NOT_EXECUTED")
        self.assertNotIn("Amati project", str(p))  # intent is not echoed to the console

    def test_continuation_and_owner_only_when_material(self):
        p = plan_context(
            intent="Resume the current code contract",
            mode="context-recovery",
            owner="docs/knowledge/flow.md", resume=True,
        )
        self.assertIn(CONTINUATION, p["context"]["REQUIRED"])
        self.assertIn("docs/knowledge/flow.md", p["context"]["REQUIRED"])
        self.assertFalse(any(x["path"] == CONTINUATION for x in p["context"]["CONDITIONAL"]))

    def test_plan_read_only_cannot_write_or_activate(self):
        for kwargs in (
            {"proposed_writes": ["AGENTS.md"]},
            {"scope": "governance"},
            {"activate_specialist": True},
            {"handoff": {"target_scope": "governance"}},
        ):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                plan_context(intent="Consider an architecture change", mode="plan", **kwargs)

    def test_development_owner_with_governance_preflight(self):
        p = plan_context(
            intent="Update source routing",
            mode="standard-development", scope="governance",
            owner="GITHUB_RULES.md",
            proposed_writes=["docs/knowledge/flow.md"],
        )
        self.assertTrue(p["ownerResolved"])
        self.assertEqual(p["writePreflight"]["status"], "ADVISORY_ALLOW")
        self.assertTrue(p["writePreflight"]["advisoryOnly"])
        self.assertIn("docs/knowledge/development-discipline.md", p["context"]["REQUIRED"])
        self.assertIsNone(p["activeSpecialist"])
        self.assertEqual(len(p["context"]["REQUIRED"]), 4)

    def test_exclusive_specialist_activation_uses_registered_skill(self):
        p = plan_context(
            intent="Fix application controller",
            mode="standard-development", scope="desktop-runtime-development",
            owner="EngineData/Frontend/RustApp/src/app/runtime/applicationController.ts",
            activate_specialist=True,
        )
        path = ".agents/skills/desktop-runtime-development/SKILL.md"
        self.assertEqual(p["activeSpecialist"], "desktop-runtime-development")
        self.assertIn(path, p["context"]["REQUIRED"])
        self.assertEqual(sum("/SKILL.md" in x for x in p["context"]["REQUIRED"]), 1)
        self.assertNotIn(BRIEF, p["context"]["REQUIRED"])

    def test_optional_specialist_does_not_preload_every_skill(self):
        p = plan_context(
            intent="Fix desktop setup",
            mode="bounded-maintenance",
            scope="desktop-runtime-development",
        )
        self.assertIsNone(p["activeSpecialist"])
        self.assertFalse(any("/SKILL.md" in path for path in p["context"]["REQUIRED"]))
        self.assertEqual(sum("/SKILL.md" in item["path"] for item in p["context"]["CONDITIONAL"]), 1)

    def test_complex_development_requires_brief_without_eager_specialist(self):
        p = plan_context(intent="Cross-owner architectural problem", mode="complex-development")
        self.assertTrue(p["requiresDevelopmentBrief"])
        self.assertIn(BRIEF, p["context"]["REQUIRED"])
        self.assertIsNone(p["activeSpecialist"])
        self.assertEqual(sum("/SKILL.md" in x for x in p["context"]["REQUIRED"]), 1)

    def test_complex_can_activate_brief_plus_one_domain_specialist(self):
        p = plan_context(
            intent="Fix approved voice ownership", mode="complex-development",
            scope="local-ai-runtime-development", activate_specialist=True,
        )
        self.assertEqual(sum("/SKILL.md" in x for x in p["context"]["REQUIRED"]), 2)
        self.assertEqual(p["activeSpecialist"], "local-ai-runtime-development")

    def test_unclear_owner_remains_unresolved_without_broad_source_scan(self):
        p = plan_context(intent="Find the authoritative owner", mode="standard-development")
        self.assertFalse(p["ownerResolved"])
        self.assertTrue(any(x["path"] == "docs/knowledge/source-ownership.md"
                            for x in p["context"]["CONDITIONAL"]))
        self.assertIn("full repository and historical transcript scans", p["context"]["EXCLUDED"])

    def test_cross_scope_write_requires_approval_not_silent_lane_change(self):
        p = plan_context(
            intent="Change a Rust owner from visual lane", mode="standard-development",
            scope="desktop-ui-design-development",
            proposed_writes=["EngineData/Frontend/RustApp/src-tauri/src/engine/runtime_state.rs"],
        )
        self.assertEqual(p["writePreflight"]["status"], "ASK")
        self.assertEqual(p["scope"], "desktop-ui-design-development")
        self.assertEqual(p["next"], "STOP_AND_RESOLVE_SCOPE_OR_APPROVAL")

    def test_private_source_cannot_be_loaded_as_required_context(self):
        with self.assertRaises(ValueError):
            plan_context(
                intent="Inspect user data", mode="context-recovery",
                owner="UserData/README.md",
            )

    def test_release_gate_preserves_deny_for_private_file(self):
        plan = plan_context(
            intent="Review packaging", mode="standard-development",
            task="RELEASE", scope="release-packaging-development",
            proposed_writes=["UserData/SavedProject/private_actor.wav"],
        )
        self.assertEqual(plan["writePreflight"]["status"], "DENY")
        self.assertEqual(plan["writePreflight"]["paths"][0]["decision"], "deny")
        self.assertEqual(plan["releasePreflight"]["decision"], "ask")

    def test_explicit_unknown_task_class_denied(self):
        with self.assertRaises(ValueError):
            plan_context(
                intent="Skip classification", mode="standard-development",
                task="INSPECT",
            )

    def test_invalid_scope_and_non_local_branch_fail_closed(self):
        for kwargs in (
            {"scope": "rust-expert"},
            {"branch": "main"},
            {"owner": "docs/../../AGENTS.md"},
        ):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                plan_context(intent="Use current source", mode="standard-development", **kwargs)

    def test_validate_cannot_mutate_and_release_needs_separate_gate(self):
        with self.assertRaises(ValueError):
            plan_context(
                intent="Run regression", mode="standard-development", task="VALIDATE",
                scope="governance", proposed_writes=["AGENTS.md"],
            )
        p = plan_context(
            intent="Consider final release", mode="standard-development", task="RELEASE",
            scope="release-packaging-development",
        )
        self.assertEqual(p["writePreflight"]["status"], "ASK")
        self.assertEqual(p["releasePreflight"]["decision"], "ask")
        self.assertFalse(p["ciTriggered"])

    def test_explicit_handoff_never_activates_other_owner(self):
        packet = {
            "target_scope": "windows-audio-runtime-development",
            "observed": "Session route not present",
            "expected": "Explicit virtual route is ready",
            "minimum_evidence": "Runtime session status with missing route",
            "resume_stage": "diagnosis",
        }
        p = plan_context(
            intent="Recover bounded desktop symptom", mode="standard-development",
            scope="desktop-runtime-development", handoff=packet,
        )
        self.assertEqual(p["handoff"]["state"], "PROPOSED_NOT_ACTIVATED")
        self.assertEqual(p["handoff"]["target_scope"], "windows-audio-runtime-development")
        self.assertIsNone(p["activeSpecialist"])
        self.assertEqual(p["scope"], "desktop-runtime-development")
        with self.assertRaises(ValueError):
            plan_context(
                intent="Cross-owner handoff", mode="standard-development",
                scope="desktop-runtime-development",
                handoff={**packet, "minimum_evidence": ""},
            )
        with self.assertRaises(ValueError):
            plan_context(
                intent="No-op handoff", mode="standard-development",
                scope="desktop-runtime-development",
                handoff={**packet, "target_scope": "desktop-runtime-development"},
            )

    def test_optional_impact_uses_existing_planner_without_execution(self):
        p = plan_context(
            intent="Validate bridge dependency impacts", mode="standard-development",
            scope="desktop-runtime-development",
            changed=["EngineData/Frontend/RustApp/src-tauri/src/commands/registry.rs"],
        )
        self.assertIn("tauri-command-bridge", p["knownImpact"]["paths"][0]["interopContracts"])
        self.assertTrue({"rust", "frontend"}.issubset(p["knownImpact"]["domains"]))
        self.assertEqual(p["knownImpact"]["proof"], "PLANNING_ONLY_NOT_EXECUTED")
        self.assertFalse(p["ciTriggered"])

    def test_retrieval_requires_explicit_domain_and_remains_discovery(self):
        with self.assertRaises(ValueError):
            plan_context(
                intent="Meetings", mode="context-recovery", knowledge_query="Meeting",
            )
        p = plan_context(
            intent="Find product law", mode="context-recovery",
            knowledge_query="Meeting", knowledge_domain="foundation",
        )
        self.assertEqual(p["rankedKnowledge"]["status"], "RANKING_NOT_AUTHORITY")
        self.assertTrue(p["rankedKnowledge"]["matches"])
        self.assertTrue(all(x["path"].startswith("docs/foundation/")
                            for x in p["rankedKnowledge"]["matches"]))

    def test_planning_continuation_is_a_separate_read_only_owner(self):
        resumed = plan_context(intent="Continue TranslateIT governance", mode="context-recovery", resume=True)
        self.assertIn("planning/development.md", resumed["context"]["REQUIRED"])
        self.assertFalse(any(path == "docs/knowledge/next-action.md" for path in resumed["context"]["REQUIRED"]))
        system = plan_context(intent="Find canonical naming", mode="context-recovery",
                              knowledge_query="canonical naming", knowledge_domain="system")
        self.assertEqual(system["rankedKnowledge"]["status"], "RANKING_NOT_AUTHORITY")
        self.assertTrue(any(x["path"].startswith("docs/system/") for x in system["rankedKnowledge"]["matches"]))

    def test_prompt_does_not_select_mode_or_request_write(self):
        p = plan_context(
            intent="please fix this right now", mode="context-recovery",
        )
        self.assertEqual(p["task"], "INSPECT")
        self.assertEqual(p["writePreflight"]["status"], "NOT_REQUESTED")


if __name__ == "__main__":
    unittest.main()

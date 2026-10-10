"""Safeguards for opt-in execution of affected, existing repository checks."""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

from repository_verification import (  # noqa: E402
    FRONT, WORKER, _checkout_identity, _node_check, _safe_known_tests,
    execute_verification, plan_verification, verify_verification,
)


class AffectedVerificationTests(unittest.TestCase):
    def test_current_repository_planning_smoke(self):
        self.assertEqual(verify_verification(), [])

    def test_governance_checks_are_bounded_read_only_until_execute(self):
        with patch("repository_verification.subprocess.run") as called:
            plan = plan_verification(
                ["docs/knowledge/flow.md"], mode="standard-development",
                scope="governance",
            )
            called.assert_not_called()
        self.assertEqual([check["id"] for check in plan["checks"]],
                         ["repository:policy"])
        self.assertTrue(plan["permission"]["decision"] == "allow")
        self.assertFalse(plan["selectedChecksExecuted"])
        self.assertFalse(plan["ciTriggered"])
        self.assertEqual(plan["proof"], "PLAN_ONLY_NO_COMMANDS_EXECUTED")

    def test_agent_and_tooling_changes_include_current_unit_suite(self):
        plan = plan_verification(
            ["tools/repository_verification.py"], mode="standard-development",
            scope="governance",
        )
        self.assertIn("repository:policy", [x["id"] for x in plan["checks"]])
        self.assertIn("repository:unit-contracts", [x["id"] for x in plan["checks"]])
        self.assertFalse(plan["reviewRequired"])

    def test_frontend_changes_reuse_existing_validators_and_typecheck(self):
        plan = plan_verification(
            [FRONT + "src/app/bridge/applicationRuntimeApi.ts"],
            mode="standard-development", scope="desktop-runtime-development",
        )
        ids = {check["id"] for check in plan["checks"]}
        self.assertIn("node:typecheck", ids)
        self.assertIn("node:validate:application-runtime", ids)
        self.assertIn("node:validate:reachability", ids)
        self.assertIn("node:validate:dependency-usage", ids)
        self.assertIn("code-health.yml", plan["manualWorkflowCandidates"])
        self.assertTrue(plan["reviewRequired"])
        self.assertIn("PARTIAL_IMPORT_GRAPH_NOT_COMPILER_PROOF", plan["residualProof"])
        self.assertFalse(plan["ciTriggered"])

    def test_worker_changes_validate_ast_and_existing_python_tests(self):
        plan = plan_verification(
            [WORKER + "worker_io_runtime.py"], mode="bounded-maintenance",
            scope="local-ai-runtime-development",
        )
        ids = {check["id"] for check in plan["checks"]}
        self.assertIn("worker:ast", ids)
        self.assertTrue(any(x.startswith("worker-test:") for x in ids))
        self.assertTrue(any(x.startswith("frontend-test:") for x in ids))
        self.assertFalse(plan["selectedChecksExecuted"])

    def test_rust_compilation_is_separately_opted_in_and_offline(self):
        changed = [FRONT + "src-tauri/src/commands/application_runtime/mod.rs"]
        default = plan_verification(
            changed, mode="standard-development",
            scope="desktop-runtime-development",
        )
        enabled = plan_verification(
            changed, mode="standard-development",
            scope="desktop-runtime-development", include_offline_rust=True,
        )
        self.assertNotIn("rust:offline-check", [x["id"] for x in default["checks"]])
        check = next(x for x in enabled["checks"] if x["id"] == "rust:offline-check")
        self.assertEqual(check["argv"], ["cargo", "check", "--locked", "--offline"])
        self.assertIn("RUST_COMPILER_NOT_SELECTED", default["residualProof"])
        self.assertNotIn("RUST_COMPILER_NOT_SELECTED", enabled["residualProof"])

    def test_read_only_work_modes_cannot_execute(self):
        for mode in ("context-recovery", "plan"):
            with self.subTest(mode=mode):
                plan = plan_verification(["docs/knowledge/flow.md"], mode=mode)
                self.assertEqual(plan["permission"]["decision"], "deny")
                with self.assertRaises(ValueError):
                    execute_verification(plan, authorized=True, expected_sha="a" * 40)

    def test_no_unknown_scope_or_mode_escalation(self):
        for kwargs in (
            {"mode": "invented", "scope": "governance"},
            {"mode": "standard-development", "scope": "rust-expert"},
        ):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                plan_verification(["docs/knowledge/flow.md"], **kwargs)

    def test_missing_scope_requires_further_authorization(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development",
        )
        self.assertEqual(plan["permission"]["decision"], "ask")
        with self.assertRaises(ValueError):
            execute_verification(plan, authorized=True, expected_sha="a" * 40)

    def test_requires_explicit_execute_and_pinned_sha(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development", scope="governance",
        )
        with self.assertRaises(ValueError):
            execute_verification(plan, expected_sha="a" * 40)
        with self.assertRaises(ValueError):
            _checkout_identity(ROOT, "not-a-full-sha")

    def test_source_execution_is_not_a_shell_and_only_selected_checks_pass(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development", scope="governance",
        )
        expected = "f" * 40
        with patch("repository_verification._checkout_identity") as pinned, \
             patch("repository_verification.subprocess.run") as called:
            called.return_value = subprocess.CompletedProcess(
                args=["python"], returncode=0, stdout="validator passed", stderr=""
            )
            result = execute_verification(plan, expected_sha=expected, authorized=True)
            pinned.assert_called()
            self.assertEqual(called.call_count, 1)
            kwargs = called.call_args.kwargs
            self.assertIs(kwargs["shell"], False)
            self.assertEqual(kwargs["env"]["npm_config_offline"], "true")
            self.assertEqual(kwargs["env"]["PIP_NO_INDEX"], "1")
        self.assertEqual(result["status"], "SELECTED_CHECKS_PASS")
        self.assertEqual(result["sha"], expected)
        self.assertTrue(result["selectedChecksPassed"])
        self.assertFalse(result["fullDependencyClosureProven"])
        self.assertFalse(result["ciTriggered"])

    def test_failure_stops_without_retry_or_fabricated_pass(self):
        plan = plan_verification(
            ["tools/repository_verification.py"], mode="standard-development",
            scope="governance",
        )
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.subprocess.run") as called:
            called.return_value = subprocess.CompletedProcess(
                args=["python"], returncode=2, stdout="", stderr="real failure"
            )
            result = execute_verification(
                plan, expected_sha="e" * 40, authorized=True,
            )
            self.assertEqual(called.call_count, 1)
        self.assertEqual(result["status"], "FAILED_OR_UNAVAILABLE")
        self.assertFalse(result["selectedChecksPassed"])
        self.assertEqual(result["executed"], 1)
        self.assertEqual(result["checks"][0]["status"], "FAIL")

    def test_passing_selected_checks_do_not_close_partial_dependency_proof(self):
        plan = plan_verification(
            [FRONT + "src/app/bridge/applicationRuntimeApi.ts"],
            mode="standard-development", scope="desktop-runtime-development",
        )
        self.assertTrue(plan["reviewRequired"])
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.shutil.which", return_value="/usr/bin/node"), \
             patch("repository_verification.subprocess.run") as called:
            called.return_value = subprocess.CompletedProcess(
                args=["node"], returncode=0, stdout="passed", stderr="",
            )
            receipt = execute_verification(
                plan, expected_sha="c" * 40, authorized=True,
            )
        self.assertEqual(receipt["status"], "SELECTED_CHECKS_PASS_PARTIAL_PROOF")
        self.assertTrue(receipt["selectedChecksPassed"])
        self.assertTrue(receipt["reviewRequired"])
        self.assertFalse(receipt["fullDependencyClosureProven"])
        self.assertFalse(receipt["ciTriggered"])

    def test_timeout_is_not_pass_and_does_not_retry(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development", scope="governance",
        )
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.subprocess.run") as called:
            called.side_effect = subprocess.TimeoutExpired(cmd="python", timeout=120)
            receipt = execute_verification(
                plan, expected_sha="c" * 40, authorized=True,
            )
            self.assertEqual(called.call_count, 1)
        self.assertFalse(receipt["selectedChecksPassed"])
        self.assertEqual(receipt["checks"][0]["status"], "TIMEOUT")

    def test_execution_output_is_bounded(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development", scope="governance",
        )
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.subprocess.run") as called:
            called.return_value = subprocess.CompletedProcess(
                args=["python"], returncode=0,
                stdout="a" * 6000, stderr="b" * 6000,
            )
            receipt = execute_verification(
                plan, expected_sha="c" * 40, authorized=True,
            )
        self.assertEqual(len(receipt["checks"][0]["stdoutTail"]), 2400)
        self.assertEqual(len(receipt["checks"][0]["stderrTail"]), 2400)

    def test_tampered_plan_rejected_before_any_test_exec(self):
        plan = plan_verification(
            ["docs/knowledge/flow.md"], mode="standard-development", scope="governance",
        )
        plan["checks"][0]["argv"] = ["git", "push", "--force"]
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.subprocess.run") as called:
            with self.assertRaises(ValueError):
                execute_verification(plan, expected_sha="d" * 40, authorized=True)
            called.assert_not_called()

    def test_missing_tool_is_unavailable_not_pass(self):
        plan = plan_verification(
            [FRONT + "src/app/bridge/applicationRuntimeApi.ts"],
            mode="standard-development", scope="desktop-runtime-development",
        )
        with patch("repository_verification._checkout_identity"), \
             patch("repository_verification.shutil.which", return_value=None), \
             patch("repository_verification.subprocess.run") as called:
            result = execute_verification(plan, expected_sha="b" * 40, authorized=True)
            called.assert_not_called()
        self.assertFalse(result["selectedChecksPassed"])
        self.assertEqual(result["checks"][0]["status"], "UNAVAILABLE_TOOL")

    def test_known_tests_reject_path_traversal_and_linked_sources(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = root / FRONT / "scripts/tests"
            base.mkdir(parents=True, exist_ok=True)
            valid = base / "safe.test.ts"
            valid.write_text("pass", encoding="utf-8")
            self.assertEqual(_safe_known_tests(
                [FRONT + "scripts/tests/safe.test.ts"],
                FRONT + "scripts/tests/", ".test.ts", root,
            ), [FRONT + "scripts/tests/safe.test.ts"])
            with self.assertRaises(ValueError):
                _safe_known_tests(
                    [FRONT + "scripts/tests/../../bad.test.ts"],
                    FRONT + "scripts/tests/", ".test.ts", root,
                )
            linked = base / "external.test.ts"
            try:
                linked.symlink_to(valid)
            except (OSError, NotImplementedError):
                return
            with self.assertRaises(ValueError):
                _safe_known_tests(
                    [FRONT + "scripts/tests/external.test.ts"],
                    FRONT + "scripts/tests/", ".test.ts", root,
                )

    def test_unknown_verifier_is_not_executed(self):
        with patch("repository_verification.plan_impacted") as impact:
            impact.return_value = {
                "changed": ["docs/knowledge/flow.md"], "domains": ["repository"],
                "knownRegressionTests": [],
                "existingFrontendValidators": ["postinstall", "shell:rm -rf /"],
                "manualWorkflowCandidates": ["repository-verify.yml"],
                "reviewRequired": False,
                "dependencyCoverage": "REGISTERED_BOUNDARY_ONLY",
                "sourceDependencyGraph": None,
            }
            plan = plan_verification(
                ["docs/knowledge/flow.md"], mode="standard-development",
                scope="governance",
            )
        self.assertEqual([x["id"] for x in plan["checks"]], ["repository:policy"])

    def test_checkout_rejects_main_dirty_state_and_different_sha(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            expected = "a" * 40

            def response(argv, **kwargs):
                kind = argv[1:]
                if kind == ["rev-parse", "--show-toplevel"]:
                    value = str(root.resolve())
                elif kind == ["branch", "--show-current"]:
                    value = self.branch
                elif kind == ["rev-parse", "HEAD"]:
                    value = self.head
                elif kind == ["status", "--porcelain", "--untracked-files=normal"]:
                    value = self.dirty
                else:
                    raise AssertionError("unexpected Git call")
                return subprocess.CompletedProcess(argv, 0, value, "")

            self.branch, self.head, self.dirty = "main", expected, ""
            with patch("repository_verification.subprocess.run", side_effect=response):
                with self.assertRaises(ValueError):
                    _checkout_identity(root, expected)
            self.branch, self.head, self.dirty = "Local", "b" * 40, ""
            with patch("repository_verification.subprocess.run", side_effect=response):
                with self.assertRaises(ValueError):
                    _checkout_identity(root, expected)
            self.branch, self.head, self.dirty = "Local", expected, " M AGENTS.md"
            with patch("repository_verification.subprocess.run", side_effect=response):
                with self.assertRaises(ValueError):
                    _checkout_identity(root, expected)
            self.branch, self.head, self.dirty = "Local", expected, ""
            with patch("repository_verification.subprocess.run", side_effect=response):
                _checkout_identity(root, expected)


if __name__ == "__main__":
    unittest.main()

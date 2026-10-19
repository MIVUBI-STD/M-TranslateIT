"""CI routing regression tests: precise changed domains, no accidental full claims."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ci_local_scope import changed_paths, route


class LocalScopeTests(unittest.TestCase):
    def test_documentation_only_skips_ai_and_compilers(self):
        self.assertEqual(route(["docs/knowledge/current-validation.md"]),
                         {"repository": True, "frontend": False, "rust": False,
                          "worker": False, "review": False})

    def test_rust_does_not_require_frontend_typecheck_or_python(self):
        self.assertEqual(route(["EngineData/Frontend/RustApp/src-tauri/src/commands/application_runtime/mod.rs"]),
                         {"repository": False, "frontend": False, "rust": True,
                          "worker": False, "review": False})

    def test_frontend_changes_are_local_to_frontend(self):
        result = route(["EngineData/Frontend/RustApp/src/components/settings/SettingsDiagnostics.svelte"])
        self.assertTrue(result["frontend"])
        self.assertFalse(result["worker"])
        self.assertFalse(result["rust"])

    def test_worker_python_does_not_require_rust(self):
        result = route(["EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py"])
        self.assertTrue(result["worker"])
        self.assertFalse(result["rust"])

    def test_unknown_paths_require_review_not_silent_green(self):
        result = route(["EngineData/UnknownRuntime/new_code.rs"])
        self.assertTrue(result["repository"])
        self.assertTrue(result["review"])

    def test_model_assets_require_worker_and_review(self):
        result = route(["EngineData/Backend/RuntimeAssets/Translation/README.md"])
        self.assertTrue(result["worker"])
        self.assertTrue(result["review"])

    def test_manual_targeted_dispatch_checks_only_selected_commit(self):
        head = "a" * 40
        parent = "b" * 40

        def fake_git(*args):
            if args == ("rev-parse", "HEAD"):
                return head.encode()
            if args == ("rev-parse", "HEAD^"):
                return parent.encode()
            if args == ("diff", "--name-only", "-z", "--no-renames", parent, head):
                return b"docs/knowledge/current-validation.md\\0"
            raise AssertionError(f"Unexpected git command: {args}")

        with patch.dict("os.environ", {"GITHUB_EVENT_NAME": "workflow_dispatch"}):
            with patch("ci_local_scope.git", side_effect=fake_git):
                paths, scope = changed_paths(None, head)
        self.assertEqual(paths, ["docs/knowledge/current-validation.md"])
        self.assertEqual(scope, "manual selected commit")

    def test_push_without_before_does_not_assume_selected_commit(self):
        head = "a" * 40

        def fake_git(*args):
            if args == ("rev-parse", "HEAD"):
                return head.encode()
            if args == ("ls-files", "-z"):
                return b"docs/a.md\\0EngineData/Frontend/RustApp/src/app/a.ts\\0"
            raise AssertionError(f"Unexpected git command: {args}")

        with patch.dict("os.environ", {"GITHUB_EVENT_NAME": "push"}):
            with patch("ci_local_scope.git", side_effect=fake_git):
                paths, scope = changed_paths(None, head)
        self.assertEqual(len(paths), 2)
        self.assertEqual(scope, "all tracked fallback")

    def test_targeted_workflow_never_starts_full_on_push_or_hides_unknown_owner(self):
        workflow = (Path(__file__).resolve().parents[2] / ".github/workflows/translateit-windows-local-validation.yml").read_text(encoding="utf-8")
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.validation_level == 'full'", workflow)
        self.assertIn('if [[ "$REVIEW" == true ]]; then', workflow)
        self.assertIn("'full' || 'targeted'", workflow)

    def test_mixed_changes_union_scopes(self):
        result = route(["AGENTS.md",
                        "EngineData/Frontend/RustApp/src-tauri/src/commands/helper_bridge.rs",
                        "EngineData/Frontend/RustApp/src/app/bridge/applicationRuntimeApi.ts"])
        self.assertTrue(result["repository"])
        self.assertTrue(result["frontend"])
        self.assertTrue(result["rust"])
        self.assertFalse(result["worker"])


if __name__ == "__main__":
    unittest.main()

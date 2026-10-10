"""Regression proof for derived TS/Svelte, Python, and Rust source dependency edges."""
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_dependencies import (  # noqa: E402
    FRONT, RUST, WORKER, affected_consumers, build_source_graph, relevant_unknown_imports,
)


def fixture(root: Path) -> None:
    files = {
        FRONT + "src/main.ts": 'import "./app/runtime/controller";',
        FRONT + "src/app/runtime/controller.ts": 'import {value} from "../bridge/api";\nexport const x = value;',
        FRONT + "src/app/bridge/api.ts": "export const value = 1;",
        FRONT + "src/app/components/Prompt.svelte": '<script>import {value} from "../bridge/api";</script>',
        FRONT + "scripts/tests/bridge.test.ts": 'import "../../src/app/bridge/api";',
        RUST + "main.rs": "mod commands;\nmod engine;",
        RUST + "commands/mod.rs": "pub mod meeting_session;",
        RUST + "commands/meeting_session.rs": (
            "use crate::engine::runtime_state::{latest_runtime_session_state};\n"
            "mod lifecycle;\n"
        ),
        RUST + "commands/meeting_session/lifecycle.rs": (
            "use super::super::meeting_session::current_state;\n"
            "pub fn life() {}"
        ),
        RUST + "engine/mod.rs": "pub mod runtime_state;",
        RUST + "engine/runtime_state.rs": "pub fn latest_runtime_session_state() {}",
        WORKER + "worker_runtime_common.py": "STATE = True",
        WORKER + "worker_io_runtime.py": "import worker_runtime_common as common",
        WORKER + "realtime_local_worker_base.py": "import worker_io_runtime as io_runtime",
        WORKER + "tests/test_io.py": "from worker_io_runtime import helper",
    }
    for name, body in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")


class DerivedDependenciesTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.root = Path(self.tmp.name)
        fixture(self.root)

    def tearDown(self):
        self.tmp.cleanup()

    def test_typescript_transitive_dependency_and_test_consumer(self):
        graph = build_source_graph(self.root)
        dependency = FRONT + "src/app/bridge/api.ts"
        consumers = affected_consumers(graph, dependency)
        self.assertIn(FRONT + "src/app/runtime/controller.ts", consumers)
        self.assertIn(FRONT + "src/main.ts", consumers)
        self.assertIn(FRONT + "scripts/tests/bridge.test.ts", consumers)
        self.assertIn(FRONT + "src/app/components/Prompt.svelte", consumers)
        self.assertFalse(graph["unresolved"])
        self.assertEqual(graph["proof"], "PARTIAL_SOURCE_IMPORTS_NOT_COMPILE_PROOF")

    def test_rust_module_and_use_edges_recover_owner_and_caller(self):
        graph = build_source_graph(self.root)
        owner = RUST + "engine/runtime_state.rs"
        self.assertIn(RUST + "commands/meeting_session.rs", affected_consumers(graph, owner))
        self.assertIn(RUST + "commands/meeting_session/lifecycle.rs",
                      affected_consumers(graph, RUST + "commands/meeting_session.rs"))
        self.assertIn(RUST + "main.rs",
                      affected_consumers(graph, RUST + "commands/mod.rs"))

    def test_python_imports_and_test_consumers(self):
        graph = build_source_graph(self.root)
        owner = WORKER + "worker_runtime_common.py"
        consumers = affected_consumers(graph, owner)
        self.assertIn(WORKER + "worker_io_runtime.py", consumers)
        self.assertIn(WORKER + "realtime_local_worker_base.py", consumers)
        self.assertIn(WORKER + "tests/test_io.py", affected_consumers(graph, WORKER + "worker_io_runtime.py"))

    def test_changed_source_importer_unknown_is_not_silently_accepted(self):
        importer = FRONT + "src/app/runtime/controller.ts"
        (self.root / importer).write_text('import "../missing";', encoding="utf-8")
        graph = build_source_graph(self.root)
        unknown = relevant_unknown_imports(graph, {importer})
        self.assertEqual(len(unknown), 1)
        self.assertEqual(unknown[0]["importer"], importer)

    def test_malformed_python_source_fails_closed(self):
        (self.root / (WORKER + "worker_io_runtime.py")).write_text(
            "if syntax invalid\n", encoding="utf-8",
        )
        with self.assertRaises(ValueError):
            build_source_graph(self.root)

    def test_source_file_budget_is_bounded_without_modifying_source(self):
        with patch("repository_dependencies.MAX_FILES", 2):
            with self.assertRaises(ValueError):
                build_source_graph(self.root)

    def test_reverse_cycles_terminate_without_duplicate_consumers(self):
        graph = build_source_graph(self.root)
        (self.root / (WORKER + "worker_io_runtime.py")).write_text(
            "import worker_runtime_common\nimport realtime_local_worker_base", encoding="utf-8",
        )
        graph = build_source_graph(self.root)
        consumers = affected_consumers(graph, WORKER + "worker_runtime_common.py")
        self.assertEqual(consumers, sorted(set(consumers)))
        self.assertNotIn(WORKER + "worker_runtime_common.py", consumers)

    def test_graph_is_deterministic_and_not_persisted(self):
        a = build_source_graph(self.root)
        b = build_source_graph(self.root)
        self.assertEqual(a["edges"], b["edges"])
        self.assertEqual(a["indexedByLanguage"], b["indexedByLanguage"])
        self.assertTrue(all(not (self.root / sub).exists() for sub in (
            ".dependency-graph.json", ".cache/dependency-graph.json"
        )))

    def test_known_repo_has_rust_ts_python_import_surfaces(self):
        graph = build_source_graph(ROOT)
        counts = graph["indexedByLanguage"]
        self.assertTrue(all(counts[x] > 0 for x in ("typescript", "rust", "python")))
        self.assertIn(
            FRONT + "src/app/runtime/applicationController.ts",
            affected_consumers(graph, FRONT + "src/app/bridge/applicationRuntimeApi.ts"),
        )
        self.assertIn(
            WORKER + "realtime_local_worker_base.py",
            affected_consumers(graph, WORKER + "worker_io_runtime.py"),
        )
        self.assertFalse(any(x.startswith("UserData/") for x in graph["indexed"]))


if __name__ == "__main__":
    unittest.main()

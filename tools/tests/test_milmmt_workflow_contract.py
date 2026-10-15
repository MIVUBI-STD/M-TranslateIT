"""Source-fixture checks for canonical MiLMMT manual-only GitHub workflows."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

VALIDATOR = (
    Path(__file__).resolve().parents[1]
    / "translation_quality"
    / "validate_canonical_milmmt_repo.py"
)

spec = importlib.util.spec_from_file_location("milmmt_workflow_validator", VALIDATOR)
assert spec is not None and spec.loader is not None
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class MiLMMTWorkflowEventContractTests(unittest.TestCase):
    def test_valid_manual_only_workflow(self) -> None:
        validator.require_manual_workflow(
            "name: Contract\non:\n  workflow_dispatch:\n\njobs:\n  check:\n",
            "quality:workflow",
        )

    def test_push_trigger_is_rejected(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "manual_only"):
            validator.require_manual_workflow(
                "on:\n  workflow_dispatch:\n  push:\n    branches:\n      - Local\n",
                "quality:workflow",
            )

    def test_other_events_and_missing_trigger_are_rejected(self) -> None:
        for triggers in ("on:\n  schedule:\n", "on:\n  workflow_call:\n", "jobs:\n"):
            with self.subTest(triggers=triggers):
                with self.assertRaises(RuntimeError):
                    validator.require_manual_workflow(triggers, "quality:workflow")

    def test_duplicate_trigger_blocks_are_rejected(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "trigger_block"):
            validator.require_manual_workflow(
                "on:\n  workflow_dispatch:\non:\n  workflow_dispatch:\n",
                "quality:workflow",
            )


if __name__ == "__main__":
    unittest.main()

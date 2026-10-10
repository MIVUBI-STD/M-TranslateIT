"""Repository information architecture: one active plan and one runtime owner."""

from __future__ import annotations

from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import verify_repository


class RepositoryInformationArchitectureTests(unittest.TestCase):
    def test_current_routes_and_ownership(self):
        errors: list[str] = []
        verify_repository.check_document_ownership_separation(errors)
        self.assertEqual(errors, [])

    def test_missing_domain_router_is_rejected(self):
        read = verify_repository.text
        def changed(path: str) -> str:
            value = read(path)
            return value.replace("foundation|knowledge|system", "foundation|knowledge", 1) if path == "docs/README.md" else value
        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=changed):
            verify_repository.check_document_ownership_separation(errors)
        self.assertTrue(any("all active domains" in x for x in errors))

    def test_duplicate_runtime_authority_is_rejected(self):
        read = verify_repository.text
        def changed(path: str) -> str:
            value = read(path)
            return value + "\n## Product runtime authority\n" if path == "docs/knowledge/flow.md" else value
        errors: list[str] = []
        with patch.object(verify_repository, "text", side_effect=changed):
            verify_repository.check_document_ownership_separation(errors)
        self.assertTrue(any("duplicates ApplicationRuntime" in x for x in errors))


if __name__ == "__main__":
    unittest.main()

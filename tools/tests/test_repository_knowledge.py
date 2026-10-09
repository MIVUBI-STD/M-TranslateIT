"""Document architecture: stable identities, explicit edges, retrieval proof boundaries."""
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_knowledge import (  # noqa: E402
    read_metadata, document_target, retrieve, verify_knowledge,
)


class KnowledgeTests(unittest.TestCase):
    def test_every_active_document_is_routed(self):
        self.assertEqual(verify_knowledge(), [])

    def test_search_is_domain_scoped_and_provenance_preserving(self):
        matches = retrieve("Meeting", "foundation", limit=6)
        self.assertTrue(matches)
        self.assertTrue(all(x["path"].startswith("docs/foundation/") for x in matches))
        self.assertTrue(all(x["authority"] == "CANONICAL" for x in matches))

    def test_id_does_not_change_with_physical_path(self):
        header = "---\nid: document.knowledge.continuity\nclass: DOCUMENT\ndomain: knowledge\nrole: GUIDE\nauthority: CANONICAL\nlifecycle: ACTIVE\n---\n"
        self.assertEqual(read_metadata(header, "docs/knowledge/renamed.md")["id"], "document.knowledge.continuity")

    def test_unrouted_document_is_detected(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "docs/knowledge").mkdir(parents=True)
            def doc(id_, domain, role):
                return f"---\nid: {id_}\nclass: DOCUMENT\ndomain: {domain}\nrole: {role}\nauthority: CANONICAL\nlifecycle: ACTIVE\n---\n# Example\n"
            (root / "docs/README.md").write_text(doc("document.docs.router", "docs", "ROUTER"), encoding="utf-8")
            (root / "docs/knowledge/isolated.md").write_text(doc("document.knowledge.isolated", "knowledge", "GUIDE"), encoding="utf-8")
            self.assertTrue(any("not reachable" in x for x in verify_knowledge(root)))

    def test_external_links_are_not_invented_edges(self):
        self.assertIsNone(document_target("docs/knowledge/README.md", "https://example.com/docs.md"))
        self.assertIsNone(document_target("docs/README.md", "../AGENTS.md"))
        self.assertEqual(document_target("docs/knowledge/README.md", "../foundation/README.md"), "docs/foundation/README.md")


if __name__ == "__main__":
    unittest.main()

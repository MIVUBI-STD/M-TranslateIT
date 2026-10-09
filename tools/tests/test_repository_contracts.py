"""Interop contract references are complete, nonduplicated and verifier-bound."""
import copy
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_contracts import verify_contracts  # noqa: E402
import json


class ContractRegistryTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads((ROOT / "tools/interop-contracts.json").read_text(encoding="utf-8"))

    def test_current_registry(self):
        self.assertEqual(verify_contracts(), [])

    def test_duplicate_contract_identity_fails(self):
        data = copy.deepcopy(self.data)
        data["contracts"].append(copy.deepcopy(data["contracts"][0]))
        self.assertTrue(any("duplicate contract ID" in x for x in verify_contracts(data=data)))

    def test_missing_consumer_fails(self):
        data = copy.deepcopy(self.data)
        data["contracts"][0]["consumers"] = ["EngineData/DoesNotExist.rs"]
        self.assertTrue(any("missing contract owner/consumer/test" in x for x in verify_contracts(data=data)))

    def test_unwired_verifier_fails(self):
        data = copy.deepcopy(self.data)
        data["contracts"][0]["scripts"] = ["validate:phantom-contract"]
        self.assertTrue(any("missing existing verifier script" in x for x in verify_contracts(data=data)))


if __name__ == "__main__":
    unittest.main()

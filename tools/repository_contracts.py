"""Machine-checkable bindings for existing cross-language contract owners and tests.

Registry data is navigation and test coverage only; it cannot redefine semantics.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "tools" / "interop-contracts.json"
FRONTEND_PACKAGE = "EngineData/Frontend/RustApp/package.json"


def verify_contracts(root: Path = ROOT, data: dict[str, Any] | None = None) -> list[str]:
    if data is None:
        data = json.loads((root / "tools/interop-contracts.json").read_text(encoding="utf-8"))
    problems: list[str] = []
    if data.get("schemaVersion") != 1 or not isinstance(data.get("contracts"), list):
        return ["invalid interop contract registry schema"]
    scripts = json.loads((root / FRONTEND_PACKAGE).read_text(encoding="utf-8")).get("scripts", {})
    ids: set[str] = set()
    owners: set[str] = set()
    for item in data["contracts"]:
        id_ = item.get("id")
        owner = item.get("owner")
        if not isinstance(id_, str) or not id_:
            problems.append("contract missing ID")
            continue
        if id_ in ids:
            problems.append(f"duplicate contract ID: {id_}")
        ids.add(id_)
        if not isinstance(owner, str) or owner.startswith("/") or ".." in Path(owner).parts:
            problems.append(f"{id_}: invalid owner path")
            continue
        if owner in owners:
            problems.append(f"{id_}: duplicate canonical boundary owner: {owner}")
        owners.add(owner)
        for key in ("consumers", "scripts", "tests"):
            if not isinstance(item.get(key), list) or not item[key] or len(item[key]) != len(set(item[key])):
                problems.append(f"{id_}: missing or duplicated {key}")
        for path in [owner, *item.get("consumers", []), *item.get("tests", [])]:
            if not isinstance(path, str) or path.startswith("/") or ".." in Path(path).parts or "\\" in path:
                problems.append(f"{id_}: invalid referenced path {path}")
                continue
            if not (root / path).is_file():
                problems.append(f"{id_}: missing contract owner/consumer/test: {path}")
        for name in item.get("scripts", []):
            if name not in scripts:
                problems.append(f"{id_}: missing existing verifier script: {name}")
        for path in item.get("tests", []):
            if path.startswith("EngineData/Frontend/") and not path.endswith(".test.ts"):
                problems.append(f"{id_}: frontend regression is not routed into the test suite")
            if path.startswith("EngineData/Backend/") and "/tests/test_" not in path:
                problems.append(f"{id_}: Python regression is not pytest discoverable")
    return sorted(set(problems))

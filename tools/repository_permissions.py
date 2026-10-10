"""Deterministic, advisory agent permission preflight for the Local-only repository."""
from __future__ import annotations

import argparse
import fnmatch
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / ".agents" / "permissions" / "permission-policy.json"


def load_policy(path: Path = POLICY) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _safe_path(raw: Any) -> str | None:
    if not isinstance(raw, str) or not raw or "\x00" in raw or "\\" in raw or ":" in raw:
        return None
    if raw.startswith("/") or "//" in raw:
        return None
    if any(part in ("", ".", "..") for part in raw.split("/")):
        return None
    return raw


def _matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def evaluate_permission(
    request: dict[str, Any], policy: dict[str, Any] | None = None
) -> dict[str, str]:
    """A preflight decision; actual GitHub, OS and Tauri authorization remain independent."""
    rule = policy if policy is not None else load_policy()
    mode = request.get("mode")
    action = request.get("action")
    mode_kind = rule["modes"].get(mode)
    if mode_kind not in {"read-only", "development"}:
        return {"decision": "deny", "reason": "unknown work mode"}
    if not isinstance(action, str):
        return {"decision": "deny", "reason": "missing action"}
    raw_path = request.get("path")
    path = _safe_path(raw_path) if raw_path is not None else None
    if raw_path is not None and path is None:
        return {"decision": "deny", "reason": "unsafe or non-canonical path"}
    if action == "read":
        if path is None:
            return {"decision": "deny", "reason": "read requires an exact relative path"}
        if _matches(path, rule["privateReadPaths"]):
            return {"decision": "ask", "reason": "private data requires explicit authorization"}
        return {"decision": "allow", "reason": "read-only source inspection"}
    if mode_kind == "read-only":
        return {"decision": "deny", "reason": "inspection/planning modes cannot mutate or execute"}
    if action in rule.get("denyActions", []):
        return {"decision": "deny", "reason": "no-cloud policy forbids hosted or remote inference execution"}
    if action in rule["approvalActions"]:
        return {"decision": "ask", "reason": "privileged action requires explicit authorization"}
    if action == "run-tests":
        return {"decision": "allow", "reason": "bounded verification in development mode"}
    if action != "write":
        return {"decision": "deny", "reason": "unknown action"}
    if path is None:
        return {"decision": "deny", "reason": "write requires an exact relative path"}
    if _matches(path, rule["denyWritePaths"]):
        return {"decision": "deny", "reason": "private/secret material is never a repository write target"}
    scope = request.get("scope")
    if scope is None:
        return {"decision": "ask", "reason": "a canonical semantic scope is required"}
    paths = rule["scopes"].get(scope)
    if not isinstance(paths, list):
        return {"decision": "deny", "reason": "unregistered semantic scope"}
    if _matches(path, paths):
        return {"decision": "allow", "reason": "path is within declared scope; owner and user approval still apply"}
    return {"decision": "ask", "reason": "path crosses declared semantic scope"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True)
    parser.add_argument("--action", required=True)
    parser.add_argument("--scope")
    parser.add_argument("--path")
    args = parser.parse_args()
    result = evaluate_permission(vars(args))
    print(json.dumps(result, sort_keys=True))
    return {"allow": 0, "ask": 1, "deny": 2}[result["decision"]]


if __name__ == "__main__":
    raise SystemExit(main())

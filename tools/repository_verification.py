"""Opt-in, bounded affected verification using canonical repository owners.

Planning is always read-only and delegates dependency reasoning to
repository_impact.py. Execution is an explicit user action against a clean,
exact-SHA Local checkout, using fixed argv (shell=False), offline settings,
and existing repository test/validator entrypoints only.

Selected checks can pass; this does not prove full graph closure, release,
model quality, native device acceptance or any unexecuted check.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

# The planner itself must not generate Python bytecode in a clean source checkout.
sys.dont_write_bytecode = True

from repository_impact import FRONT, WORKER, canonical_path, plan_impacted
from repository_permissions import evaluate_permission, load_policy

ROOT = Path(__file__).resolve().parents[1]
RUST = FRONT + "src-tauri/"
NODE_TESTS = FRONT + "scripts/tests/"
PYTHON_TESTS = WORKER + "tests/"
MAX_SELECTED_TESTS = 8
MAX_CHECKS = 24
OUTPUT_LIMIT = 2400
SHA40 = re.compile(r"[0-9a-f]{40}\Z")
# Safe direct invocation of canonical Node validators; never npm lifecycle hooks.
# Each source must remain mapped to its exact original package.json script.
VALIDATORS = {
    "check:rust": "validate_rust_manifest_preflight.mjs",
    "typecheck": "run_svelte_check_strict.mjs",
    "validate:application-runtime": "validate_application_runtime_architecture.mjs",
    "validate:bridge-contract": "validate_bridge_contract.mjs",
    "validate:tauri-command-args": "validate_tauri_command_args.mjs",
    "validate:virtual-route": "validate_virtual_route_contract.mjs",
    "validate:reachability": "validate_frontend_reachability.mjs",
    "validate:runtime-api-usage": "validate_runtime_api_usage.mjs",
    "validate:dependency-usage": "validate_dependency_usage.mjs",
    "validate:rust-dependency-usage": "validate_rust_dependency_usage.mjs",
    "validate:test-references": "validate_test_reference_reachability.mjs",
    "validate:source-size": "validate_source_size_budget.mjs",
}
AST_SCRIPT = (
    "import ast,sys;from pathlib import Path;"
    "[ast.parse(Path(p).read_text(encoding='utf-8'), filename=p) "
    "for p in sys.argv[1:]]"
)


def _check(id_: str, domain: str, argv: list[str], cwd: str,
           reason: str, *, timeout: int = 120) -> dict:
    return {"id": id_, "domain": domain, "argv": argv, "cwd": cwd,
            "reason": reason, "timeoutSeconds": timeout}


def _node_check(name: str, package: dict) -> dict:
    source = VALIDATORS.get(name)
    if not source or package.get("scripts", {}).get(name) != f"node scripts/{source}":
        raise ValueError(f"unreviewed or drifted Node validator entrypoint: {name}")
    return _check("node:" + name, "frontend",
                  ["node", "scripts/" + source], FRONT,
                  "canonical source validator")


def _safe_known_tests(paths: list[str], prefix: str, suffix: str, root: Path) -> list[str]:
    chosen = []
    safe_directory = (root / prefix).resolve()
    if not safe_directory.is_relative_to(root.resolve()):
        raise ValueError("regression test directory escapes the repository")
    for path in paths:
        path = canonical_path(path)
        if not path.startswith(prefix) or not path.endswith(suffix):
            continue
        candidate = root / path
        if candidate.is_symlink() or not candidate.is_file():
            raise ValueError(f"regression test is missing/linked: {path}")
        if candidate.resolve().is_relative_to(safe_directory):
            chosen.append(path)
        else:
            raise ValueError("regression test escapes its canonical source root")
    return sorted(set(chosen))


def plan_verification(
    changed: list[str], *, mode: str, scope: str | None = None,
    include_offline_rust: bool = False, root: Path = ROOT,
) -> dict:
    """Select only existing entrypoints; never infer permissions from text."""
    policy = load_policy(root / ".agents/permissions/permission-policy.json")
    if mode not in policy["modes"]:
        raise ValueError("work mode must be explicitly selected from AGENTS.md")
    if scope is not None and scope not in policy["scopes"]:
        raise ValueError("unregistered semantic scope")
    permission = evaluate_permission(
        {"mode": mode, "action": "run-tests", "scope": scope}, policy
    )
    if scope is None and permission["decision"] == "allow":
        permission = {"decision": "ask", "reason": "explicit semantic scope is required"}
    impact = plan_impacted(changed, root=root)
    package = json.loads((root / FRONT / "package.json").read_text(encoding="utf-8"))
    paths = impact["changed"]
    domains = set(impact["domains"])
    checks: list[dict] = []
    ids: set[str] = set()

    def add(check: dict) -> None:
        if check["id"] not in ids:
            if len(checks) >= MAX_CHECKS:
                raise ValueError("bounded verification plan exceeds command budget")
            ids.add(check["id"])
            checks.append(check)

    def node(name: str) -> None:
        add(_node_check(name, package))

    if "repository" in domains:
        add(_check(
            "repository:policy", "repository",
            [sys.executable, "-B", "tools/verify_repository.py"], ".",
            "canonical root repository verifier", timeout=180,
        ))
        if any(path.startswith(("tools/", ".agents/")) for path in paths):
            add(_check(
                "repository:unit-contracts", "repository",
                [sys.executable, "-B", "-m", "unittest", "discover",
                 "-s", "tools/tests", "-p", "test_*.py"], ".",
                "existing governance/agent/impact regression suite", timeout=180,
            ))

    changed_python = [
        path for path in paths if path.startswith(WORKER) and path.endswith(".py")
    ]
    if changed_python:
        for name in changed_python:
            if not (root / name).is_file() or (root / name).is_symlink():
                raise ValueError(f"Python AST target missing/linked: {name}")
        add(_check(
            "worker:ast", "python",
            [sys.executable, "-B", "-c", AST_SCRIPT, *changed_python], ".",
            "bounded source syntax check; no worker inference/import execution",
        ))

    rust_changed = any(path.startswith(RUST) and path.endswith(".rs") for path in paths)
    frontend_changed = any(
        path.startswith(FRONT + "src/") and path.endswith((".ts", ".svelte"))
        for path in paths
    )
    if rust_changed:
        for name in ("check:rust", "validate:rust-dependency-usage",
                     "validate:bridge-contract", "validate:tauri-command-args"):
            node(name)
        if any("application_runtime/" in path or path.endswith("/registry.rs")
               for path in paths):
            node("validate:application-runtime")
        if any("meeting_session" in path or "/engine/audio/" in path or
               "virtual_mic_route" in path for path in paths):
            node("validate:virtual-route")
        if include_offline_rust:
            add(_check(
                "rust:offline-check", "rust",
                ["cargo", "check", "--locked", "--offline"], FRONT + "src-tauri/",
                "explicit optional Rust compilation using only cached locked crates",
                timeout=300,
            ))
    if frontend_changed:
        for name in ("validate:reachability", "validate:runtime-api-usage",
                     "validate:dependency-usage", "typecheck"):
            node(name)
    if any(path.startswith(NODE_TESTS) for path in paths):
        node("validate:test-references")
    # Explicit cross-language contract verifiers from the existing registry.
    # Unknown scripts cannot become executable command strings.
    for name in impact["existingFrontendValidators"]:
        if name in VALIDATORS:
            node(name)

    known = impact["knownRegressionTests"]
    js_tests = _safe_known_tests(known, NODE_TESTS, ".test.ts", root)
    py_tests = _safe_known_tests(known, PYTHON_TESTS, ".py", root)
    for path in paths:
        if path.startswith(NODE_TESTS) and path.endswith(".test.ts"):
            js_tests = sorted(set(js_tests + _safe_known_tests([path], NODE_TESTS, ".test.ts", root)))
        if path.startswith(PYTHON_TESTS) and path.endswith(".py"):
            py_tests = sorted(set(py_tests + _safe_known_tests([path], PYTHON_TESTS, ".py", root)))

    unselected = sorted(js_tests[MAX_SELECTED_TESTS:] + py_tests[MAX_SELECTED_TESTS:])
    for path in js_tests[:MAX_SELECTED_TESTS]:
        add(_check(
            "frontend-test:" + path, "frontend",
            ["node", "--experimental-strip-types", "--test",
             path[len(FRONT):]], FRONT,
            "known affected canonical TypeScript regression",
        ))
    for path in py_tests[:MAX_SELECTED_TESTS]:
        add(_check(
            "worker-test:" + path, "python",
            [sys.executable, "-B", "-m", "pytest", "-q", path], ".",
            "known affected pytest regression; never installs dependencies",
        ))

    residuals = []
    if impact["reviewRequired"]:
        residuals.append("UNPROVEN_SOURCE_DEPENDENCY_CLOSURE")
    if impact["sourceDependencyGraph"] is not None:
        residuals.append("PARTIAL_IMPORT_GRAPH_NOT_COMPILER_PROOF")
    if rust_changed and not include_offline_rust:
        residuals.append("RUST_COMPILER_NOT_SELECTED")
    if unselected:
        residuals.append("REGRESSION_BUDGET_EXCEEDED")
    if "release" in domains:
        residuals.append("RELEASE_PAYLOAD_AND_NATIVE_PROOF_REQUIRE_SEPARATE_APPROVAL")
    if "python" in domains and not changed_python and not py_tests:
        residuals.append("PYTHON_RUNTIME_CHECK_NOT_SELECTED")
    if not checks:
        residuals.append("NO_SUPPORTED_EXECUTABLE_CHECK")
    return {
        "schemaVersion": 1, "mode": mode, "scope": scope,
        "branch": "Local", "changed": paths,
        "permission": permission, "checks": checks,
        "unselectedKnownTests": unselected,
        "manualWorkflowCandidates": impact["manualWorkflowCandidates"],
        "reviewRequired": bool(residuals),
        "residualProof": sorted(set(residuals)),
        "dependencyCoverage": impact["dependencyCoverage"],
        "selectedChecksExecuted": False, "ciTriggered": False,
        "proof": "PLAN_ONLY_NO_COMMANDS_EXECUTED",
    }


def _checkout_identity(root: Path, expected_sha: str) -> None:
    if not isinstance(expected_sha, str) or not SHA40.fullmatch(expected_sha):
        raise ValueError("explicit exact 40-character expected SHA is required")
    commands = [
        (["git", "rev-parse", "--show-toplevel"], "repository root"),
        (["git", "branch", "--show-current"], "branch"),
        (["git", "rev-parse", "HEAD"], "head"),
        (["git", "status", "--porcelain", "--untracked-files=normal"], "working-tree status"),
    ]
    values = {}
    for argv, label in commands:
        try:
            item = subprocess.run(argv, cwd=root, shell=False, capture_output=True,
                                  text=True, timeout=10, check=False)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise ValueError(f"cannot prove exact Local checkout: {label}: {exc}") from exc
        if item.returncode != 0:
            raise ValueError(f"cannot prove exact Local checkout: {label}")
        values[label] = item.stdout.strip()
    if Path(values["repository root"]).resolve() != root.resolve():
        raise ValueError("source checkout does not match the canonical repository root")
    if values["branch"] != "Local":
        raise ValueError("verification execution requires branch Local; main is final-only")
    if values["head"] != expected_sha:
        raise ValueError("verification cannot use another revision's proof")
    if values["working-tree status"]:
        raise ValueError("worktree is dirty; cannot attribute proof to exact HEAD")


def execute_verification(
    plan: dict, *, expected_sha: str, root: Path = ROOT,
    authorized: bool = False,
) -> dict:
    """Execute known commands only after an explicit invocation and exact SHA."""
    if not authorized:
        raise ValueError("execution requires the explicit --execute user action")
    if plan.get("branch") != "Local" or plan.get("schemaVersion") != 1:
        raise ValueError("invalid verification plan authority")
    if plan.get("permission", {}).get("decision") != "allow":
        raise ValueError("permission preflight did not allow bounded verification")
    _checkout_identity(root, expected_sha)
    checks = plan.get("checks")
    if not isinstance(checks, list) or not checks or len(checks) > MAX_CHECKS:
        raise ValueError("verification plan has no bounded executable commands")
    # Rebuild from canonical current source so callers cannot provide tampered
    # argv/cwd/check arrays or piggyback other scope on a legitimate receipt.
    include_rust = any(c.get("id") == "rust:offline-check" for c in checks)
    expected = plan_verification(plan["changed"], mode=plan["mode"],
                                 scope=plan["scope"], include_offline_rust=include_rust, root=root)
    if expected != plan:
        raise ValueError("verification plan drifted or was modified")
    env = dict(os.environ)
    env.update({
        "PYTHONDONTWRITEBYTECODE": "1", "CI": "1",
        "npm_config_offline": "true", "CARGO_NET_OFFLINE": "true",
        "PIP_NO_INDEX": "1", "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1", "GIT_TERMINAL_PROMPT": "0",
    })
    receipts = []
    for check in checks:
        argv = check["argv"]
        if not argv or not isinstance(argv, list) or argv[0] not in {
            sys.executable, "node", "cargo",
        }:
            raise ValueError("unrecognized executable in selected proof check")
        if check["cwd"] not in (".", FRONT, FRONT + "src-tauri/"):
            raise ValueError("verification check escaped canonical working owners")
        if argv[0] in {"node", "cargo"} and shutil.which(argv[0]) is None:
            receipts.append({"id": check["id"], "status": "UNAVAILABLE_TOOL",
                             "argv": argv, "exitCode": None, "stdoutTail": "", "stderrTail": ""})
            break
        try:
            completed = subprocess.run(
                argv, cwd=root / check["cwd"], env=env,
                shell=False, capture_output=True, text=True,
                check=False, timeout=check["timeoutSeconds"],
            )
            state = "PASS" if completed.returncode == 0 else "FAIL"
            receipts.append({
                "id": check["id"], "status": state, "argv": argv,
                "exitCode": completed.returncode,
                "stdoutTail": completed.stdout[-OUTPUT_LIMIT:],
                "stderrTail": completed.stderr[-OUTPUT_LIMIT:],
            })
            if state != "PASS":
                break
        except subprocess.TimeoutExpired:
            receipts.append({"id": check["id"], "status": "TIMEOUT",
                             "argv": argv, "exitCode": None, "stdoutTail": "", "stderrTail": ""})
            break
        except OSError:
            receipts.append({"id": check["id"], "status": "UNAVAILABLE_TOOL",
                             "argv": argv, "exitCode": None, "stdoutTail": "", "stderrTail": ""})
            break

    # Checks may have altered tracked source. Re-check source identity before
    # reporting any selected check as exact-head executable evidence.
    _checkout_identity(root, expected_sha)
    selected_pass = len(receipts) == len(checks) and all(
        result["status"] == "PASS" for result in receipts
    )
    return {
        "schemaVersion": 1, "branch": "Local", "sha": expected_sha,
        "planned": len(checks), "executed": len(receipts),
        "checks": receipts, "selectedChecksPassed": selected_pass,
        "reviewRequired": plan["reviewRequired"],
        "residualProof": plan["residualProof"],
        "status": (
            "SELECTED_CHECKS_PASS_PARTIAL_PROOF" if selected_pass and plan["reviewRequired"]
            else "SELECTED_CHECKS_PASS" if selected_pass else "FAILED_OR_UNAVAILABLE"
        ),
        "proof": "EXECUTED_SOURCE_SELECTED_CHECKS_ONLY" if selected_pass
                 else "EXECUTION_INCOMPLETE",
        "fullDependencyClosureProven": False,
        "ciTriggered": False,
    }


def verify_verification(root: Path = ROOT) -> list[str]:
    """Static/planning smoke gate; does not spawn any verification process."""
    errors = []
    for paths, mode, scope in [
        (["docs/knowledge/flow.md"], "standard-development", "governance"),
        ([FRONT + "src/app/bridge/applicationRuntimeApi.ts"],
         "standard-development", "desktop-runtime-development"),
        ([WORKER + "worker_io_runtime.py"], "bounded-maintenance",
         "local-ai-runtime-development"),
    ]:
        try:
            plan = plan_verification(paths, mode=mode, scope=scope, root=root)
            if plan["selectedChecksExecuted"] or plan["ciTriggered"]:
                errors.append("verification planning unexpectedly asserted execution")
            if not plan["checks"]:
                errors.append(f"{paths}: no known affected verification available")
        except (OSError, ValueError, KeyError, TypeError) as exc:
            errors.append(f"{paths}: verification planning invalid: {exc}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changed", nargs="+", required=True)
    parser.add_argument("--mode", required=True)
    parser.add_argument("--scope")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--expected-sha")
    parser.add_argument("--include-offline-rust", action="store_true")
    args = parser.parse_args()
    try:
        plan = plan_verification(
            args.changed, mode=args.mode, scope=args.scope,
            include_offline_rust=args.include_offline_rust,
        )
        if not args.execute:
            print(json.dumps(plan, indent=2))
            return 0
        if not args.expected_sha:
            raise ValueError("--execute requires --expected-sha")
        receipt = execute_verification(plan, expected_sha=args.expected_sha, authorized=True)
        print(json.dumps(receipt, indent=2))
        return 0 if receipt["selectedChecksPassed"] and not receipt["reviewRequired"] else 1
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        parser.exit(2, f"affected verification invalid: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

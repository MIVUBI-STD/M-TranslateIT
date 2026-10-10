"""Conservative, on-demand changed-path → affected evidence planning.

Derives registered cross-language edges from existing interop-contracts.json and
literal regression references from source tests. It does not execute, dispatch,
or replace an authoritative runtime/dependency compiler.
"""
from __future__ import annotations

import argparse
import json
import posixpath
import re
from pathlib import Path

from repository_contracts import verify_contracts

ROOT = Path(__file__).resolve().parents[1]
FRONT = "EngineData/Frontend/RustApp/"
WORKER = "EngineData/Backend/LocalWorker/WorkerRuntime/"
RUST = FRONT + "src-tauri/"
URL_REF = re.compile(r"""new URL\(\s*["']([^"']+)["']\s*,\s*import\.meta\.url\s*\)""")
# Pure path routing; no persisted duplicate owner/asset/workflow graph.
WORKFLOWS = {
    "repository": "repository-verify.yml",
    "frontend": "code-health.yml",
    "rust": "code-health.yml",
    "python": "code-health.yml",
    "release": "release-payload-verify.yml",
    "translation-quality": "milmmt-repo-contract.yml",
    "asr-quality": "asr-quality-contract.yml",
    "tts-quality": "tts-quality-contract.yml",
    "quality-readiness": "quality-readiness-contract.yml",
    "worker-lock": "workerruntime-lock.yml",
}


def canonical_path(value: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise ValueError("path must be a nonempty canonical repository-relative name")
    if value.startswith("/") or ":" in value or "//" in value or any(
        part in ("", ".", "..") for part in value.split("/")
    ):
        raise ValueError(f"unsafe or non-canonical changed path: {value!r}")
    if "\n" in value or "\r" in value or any(ord(char) < 32 for char in value):
        raise ValueError("control characters in a changed path are forbidden")
    return value


def _base_domains(path: str) -> tuple[set[str], bool]:
    # This is an execution-scope fallback, not semantic product ownership.
    if path.startswith(("docs/", ".agents/")) or path in {
        "AGENTS.md", "GITHUB_RULES.md", "CONTEXT.md", "README.md",
        "CONTRIBUTING.md", "SECURITY.md", "toolchain.json",
        ".editorconfig", ".gitignore", ".gitattributes", "DEV.cmd",
    } or path.startswith(("tools/repository_", "tools/tests/", "tooling/")) or path == "tools/verify_repository.py":
        return {"repository"}, False
    if path.startswith(".github/"):
        d = {"repository"}
        name = path.rsplit("/", 1)[-1]
        if name == "code-health.yml":
            d.update({"frontend", "rust", "python"})
        elif name == "release-payload-verify.yml":
            d.add("release")
        elif name == "asr-quality-contract.yml":
            d.add("asr-quality")
        elif name == "tts-quality-contract.yml":
            d.add("tts-quality")
        return d, False
    for directory, domain in (
        ("tools/translation_quality/", "translation-quality"),
        ("tools/asr_quality/", "asr-quality"),
        ("tools/tts_quality/", "tts-quality"),
        ("tools/quality_readiness/", "quality-readiness"),
    ):
        if path.startswith(directory):
            return {domain}, False
    if path.startswith("EngineData/Backend/RuntimeAssets/"):
        return {"release", "python"}, True
    if path.startswith(WORKER):
        domains = {"python", "frontend"}  # worker outputs may reach frontend tests
        if path.endswith(("uv.lock", "pyproject.toml")):
            domains.add("worker-lock")
        return domains, True
    if path.startswith(RUST + "windows/") or path == RUST + "tauri.release.conf.json":
        return {"release", "rust"}, True
    if path.startswith(FRONT + "scripts/"):
        basename = path.rsplit("/", 1)[-1]
        if any(part in basename for part in ("release", "payload", "notices")) or basename.startswith("stage_"):
            return {"release", "frontend"}, True
        return {"frontend"}, True
    if path.startswith(FRONT + "src-tauri/"):
        # Rust may supply public IPC fields. Do not assume a complete call graph.
        return {"rust", "frontend"}, True
    if path.startswith(FRONT + "src/"):
        return {"frontend"}, True
    if path in {FRONT + "package.json", FRONT + "package-lock.json", FRONT + "tsconfig.json"}:
        return {"frontend"}, True
    if path.startswith("EngineData/"):
        return {"repository", "frontend", "rust", "python", "release"}, True
    return {"repository", "frontend", "rust", "python", "release"}, True


def _literal_test_edges(root: Path) -> dict[str, set[str]]:
    """Bounded read of the canonical frontend regression owner, not a repo scan."""
    tests = sorted((root / FRONT / "scripts/tests").glob("*.test.ts"))
    if len(tests) > 200:
        raise ValueError("frontend regression inventory exceeded bounded scan budget")
    referenced: dict[str, set[str]] = {}
    for test in tests:
        test_path = test.relative_to(root).as_posix()
        source = test.read_text(encoding="utf-8")
        for match in URL_REF.finditer(source):
            raw = match.group(1)
            if "$" in raw or not raw.startswith("."):
                continue
            destination = posixpath.normpath(posixpath.join(posixpath.dirname(test_path), raw))
            if destination.startswith("EngineData/"):
                referenced.setdefault(destination, set()).add(test_path)
    return referenced


def plan_impacted(changed_paths: list[str], root: Path = ROOT) -> dict[str, Any]:
    """Report candidate test owners and conservatively affected domains.

    A bounded source graph is intentionally not treated as complete. For every
    source change execution coverage remains conservative; unknown paths
    escalate, never narrow. This function does not call GitHub Actions.
    """
    if not changed_paths:
        raise ValueError("at least one changed path is required")
    changed = sorted(set(canonical_path(item) for item in changed_paths))
    registry = json.loads((root / "tools/interop-contracts.json").read_text(encoding="utf-8"))
    contract_issues = verify_contracts(root, registry)
    if contract_issues:
        raise ValueError("interop binding source is invalid: " + "; ".join(contract_issues[:5]))
    owners: set[str] = set()
    validators: set[str] = set()
    tests: set[str] = set()
    domains: set[str] = set()
    risks: list[str] = []
    entries: list[dict] = []
    refs = _literal_test_edges(root) if any(p.startswith("EngineData/") for p in changed) else {}
    for path in changed:
        base_domains, conservative = _base_domains(path)
        matched: list[str] = []
        linked_tests = set(refs.get(path, set()))
        for contract in registry["contracts"]:
            endpoints = {contract["owner"], *contract["consumers"], *contract["tests"]}
            if path in endpoints:
                matched.append(contract["id"])
                owners.add(contract["owner"])
                validators.update(contract["scripts"])
                linked_tests.update(contract["tests"])
                # One interop change can affect both producer and consumer domains.
                for endpoint in {contract["owner"], *contract["consumers"]}:
                    cross_domains, _ = _base_domains(endpoint)
                    base_domains.update(cross_domains)
        if linked_tests:
            base_domains.add("frontend")
            tests.update(linked_tests)
        domains.update(base_domains)
        if conservative and not matched:
            risks.append(path)
        entries.append({
            "path": path,
            "domains": sorted(base_domains),
            "interopContracts": sorted(matched),
            "directRegressionTests": sorted(linked_tests),
            "conservative": conservative and not matched,
        })
    manual_workflows = sorted({WORKFLOWS[d] for d in domains})
    app_scripts = json.loads((root / FRONT / "package.json").read_text(encoding="utf-8"))["scripts"]
    candidate_frontend_scripts = [
        name for name in ("typecheck", "test:frontend-runtime", "validate:source-contracts")
        if "frontend" in domains and name in app_scripts
    ]
    return {
        "schemaVersion": 1,
        "changed": changed,
        "domains": sorted(domains),
        "paths": entries,
        "matchedInteropOwners": sorted(owners),
        "knownRegressionTests": sorted(tests),
        "existingFrontendValidators": sorted(validators),
        "candidateFrontendScripts": candidate_frontend_scripts,
        "manualWorkflowCandidates": manual_workflows,
        "reviewRequired": bool(risks),
        "unprovenClosures": sorted(risks),
        "dependencyCoverage": "CONSERVATIVE" if risks else "REGISTERED_BOUNDARY_ONLY",
        "proof": "PLANNING_ONLY_NOT_EXECUTED",
        "ciTriggered": False,
    }


def verify_impact(root: Path = ROOT) -> list[str]:
    """Source-backed smoke cases, not executed runtime or complete dependency proof."""
    issues: list[str] = []
    cases = (
        (["docs/knowledge/flow.md"], {"repository"}),
        ([FRONT + "src-tauri/src/commands/registry.rs"], {"rust", "frontend"}),
        ([WORKER + "worker_io_runtime.py"], {"python", "frontend", "rust"}),
    )
    for paths, required in cases:
        try:
            plan = plan_impacted(paths, root)
            if not required.issubset(plan["domains"]):
                issues.append(f"{paths}: missing conservative impacted domains: {sorted(required)}")
            if plan["proof"] != "PLANNING_ONLY_NOT_EXECUTED" or plan["ciTriggered"]:
                issues.append(f"{paths}: impact plan overstated proof")
        except (OSError, ValueError, KeyError, TypeError) as exc:
            issues.append(f"{paths}: invalid impact planner: {exc}")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--changed", nargs="+", metavar="PATH")
    args = parser.parse_args()
    try:
        if args.check:
            issues = verify_impact()
            print(json.dumps({"passed": not issues, "issues": issues}, indent=2))
            return int(bool(issues))
        print(json.dumps(plan_impacted(args.changed), indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as exc:
        parser.exit(2, f"impact plan invalid: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

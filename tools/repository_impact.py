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
from typing import Any

from repository_contracts import verify_contracts
from repository_dependencies import (
    FRONT as FRONT_SOURCE_ROOT, RUST as RUST_SOURCE_ROOT, WORKER as PYTHON_SOURCE_ROOT,
    affected_consumers, build_source_graph, relevant_unknown_imports,
)

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
    source_paths = tuple((FRONT_SOURCE_ROOT + "src/", FRONT_SOURCE_ROOT + "scripts/tests/",
                          RUST_SOURCE_ROOT, PYTHON_SOURCE_ROOT))
    graph = build_source_graph(root) if any(p.startswith(source_paths) for p in changed) else None
    all_consumers: set[str] = set()
    graph_unknown: list[dict[str, str]] = []
    for path in changed:
        base_domains, conservative = _base_domains(path)
        matched: list[str] = []
        linked_tests = set(refs.get(path, set()))
        source_consumers: list[str] = []
        unknown: list[dict[str, str]] = []
        if graph is not None and path.startswith(source_paths):
            if path not in graph["indexed"]:
                risks.append(path)  # Removed/moved/unknown source: fail open is forbidden.
            else:
                source_consumers = affected_consumers(graph, path)
                projected = {path, *source_consumers}
                unknown = relevant_unknown_imports(graph, projected)
                graph_unknown.extend(unknown)
                for consumer in source_consumers:
                    affected_domains, _ = _base_domains(consumer)
                    base_domains.update(affected_domains)
                    linked_tests.update(refs.get(consumer, set()))
                    if consumer.endswith(".test.ts") or (
                        consumer.startswith(PYTHON_SOURCE_ROOT + "tests/") and
                        consumer.split("/")[-1].startswith("test_") and consumer.endswith(".py")
                    ):
                        linked_tests.add(consumer)
                all_consumers.update(source_consumers)
        else:
            projected = {path}
        for contract in registry["contracts"]:
            endpoints = {contract["owner"], *contract["consumers"], *contract["tests"]}
            if not projected.isdisjoint(endpoints):
                matched.append(contract["id"])
                owners.add(contract["owner"])
                validators.update(contract["scripts"])
                linked_tests.update(contract["tests"])
                # Interop producer/consumer validation is shared even when only
                # a transitive source consumer overlaps the registered contract.
                for endpoint in {contract["owner"], *contract["consumers"]}:
                    cross_domains, _ = _base_domains(endpoint)
                    base_domains.update(cross_domains)
        if linked_tests:
            if any(test.startswith(FRONT + "scripts/tests/") for test in linked_tests):
                base_domains.add("frontend")
            if any(test.startswith(PYTHON_SOURCE_ROOT + "tests/") for test in linked_tests):
                base_domains.add("python")
            tests.update(linked_tests)
        domains.update(base_domains)
        if (conservative and not matched) or unknown:
            risks.append(path)
        entries.append({
            "path": path,
            "domains": sorted(base_domains),
            "interopContracts": sorted(matched),
            "directRegressionTests": sorted(linked_tests),
            "directSourceConsumers": sorted(graph["reverse"].get(path, ())) if graph else [],
            "affectedSourceConsumers": source_consumers,
            "unknownSourceImports": unknown[:16],
            "sourceDependencyProof": (
                "PARTIAL_SOURCE_IMPORTS_NOT_COMPILE_PROOF" if graph and path in graph["indexed"]
                else "NOT_INDEXED"
            ),
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
        "unprovenClosures": sorted(set(risks)),
        "dependencyCoverage": (
            "CONSERVATIVE" if risks else
            "REGISTERED_BOUNDARY_PLUS_PARTIAL_IMPORTS" if graph else "REGISTERED_BOUNDARY_ONLY"
        ),
        "sourceDependencyGraph": ({
            "indexedByLanguage": graph["indexedByLanguage"],
            "knownEdges": len(graph["edges"]),
            "unresolvedReferences": len(graph["unresolved"]),
            "affectedSourceConsumers": sorted(all_consumers),
            "affectedUnknownImports": sorted(graph_unknown, key=lambda x: (
                x["importer"], x["specifier"])),
            "fullClosureProven": False,
            "proof": graph["proof"],
        } if graph else None),
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
        ([FRONT + "src/app/bridge/applicationRuntimeApi.ts"], {"frontend"}),
        ([RUST + "src/engine/runtime_state.rs"], {"rust", "frontend"}),
    )
    for paths, required in cases:
        try:
            plan = plan_impacted(paths, root)
            if not required.issubset(plan["domains"]):
                issues.append(f"{paths}: missing conservative impacted domains: {sorted(required)}")
            if plan["proof"] != "PLANNING_ONLY_NOT_EXECUTED" or plan["ciTriggered"]:
                issues.append(f"{paths}: impact plan overstated proof")
            if any(p.startswith(FRONT + "src/") or p.startswith(WORKER) for p in paths):
                graph = plan["sourceDependencyGraph"]
                if graph is None or graph["fullClosureProven"] is not False:
                    issues.append(f"{paths}: source graph missing its partial-proof boundary")
            if paths == [WORKER + "worker_io_runtime.py"] and (
                WORKER + "realtime_local_worker_base.py"
                not in plan["sourceDependencyGraph"]["affectedSourceConsumers"]
            ):
                issues.append("Python worker importer is missing from the derived consumer graph")
            if paths == [FRONT + "src/app/bridge/applicationRuntimeApi.ts"] and (
                FRONT + "src/app/runtime/applicationController.ts"
                not in plan["sourceDependencyGraph"]["affectedSourceConsumers"]
            ):
                issues.append("TypeScript transitive controller consumer was not discovered")
            if paths == [RUST + "src/engine/runtime_state.rs"] and (
                FRONT + "src-tauri/src/commands/meeting_session.rs"
                not in plan["sourceDependencyGraph"]["affectedSourceConsumers"]
            ):
                issues.append("Rust runtime-state caller was not discovered")
            if paths == ["docs/knowledge/flow.md"] and plan["sourceDependencyGraph"] is not None:
                issues.append("documentation change unnecessarily loaded the entire source graph")
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

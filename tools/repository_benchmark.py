"""Deterministic, source-derived TranslateIT method-flow conformance benchmark.

This replays *declared* agent routing cases through the existing explicit
context planner. It DOES NOT select skills using an AI model, execute tests,
measure model latency, run CI, verify native Windows, or persist result state.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
from typing import Any

from repository_agent_evals import validate_agent_evals
from repository_context import plan_context
from repository_impact import plan_impacted
from repository_knowledge import catalog, verify_knowledge
from repository_permissions import evaluate_permission
from repository_skill_admission import admit_skills

ROOT = Path(__file__).resolve().parents[1]
ROUTING = ".agents/evals/skill-routing.json"
PERMISSIONS = ".agents/evals/permission-cases.json"
BRIEF = ".agents/skills/development-brief/SKILL.md"
DOC_IMPACT = "docs/system/zero-waste-execution.md"
SOURCE_IMPACT = "EngineData/Frontend/RustApp/src/app/bridge/applicationRuntimeApi.ts"


def _json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def replay_declared_routing(cases: list[dict[str, Any]], root: Path = ROOT) -> dict[str, Any]:
    """Replay explicit expectations, not observed AI task-classification quality."""
    issues: list[str] = []
    rows: list[dict[str, Any]] = []
    for item in cases:
        identity = item.get("id", "<unknown>")
        mode = item.get("expectedMode")
        scope = item.get("expectedScope")
        specialist = item.get("expectedSpecialist")
        requires_brief = item.get("requiresBrief")
        try:
            result = plan_context(
                intent=item["prompt"], mode=mode, scope=scope,
                activate_specialist=specialist is not None, root=root,
            )
            required = result["context"]["REQUIRED"]
            conditional = result["context"]["CONDITIONAL"]
            actual_skills = sorted(p for p in required if p.startswith(".agents/skills/") and p.endswith("/SKILL.md"))
            expected_skills = ([BRIEF] if requires_brief else [])
            if specialist is not None:
                expected_skills.append(f".agents/skills/{specialist}/SKILL.md")
            excluded_specialists = set(item.get("forbiddenSpecialists", []))
            safe = (
                result["branch"] == "Local"
                and result["mode"] == mode and result["scope"] == scope
                and result["activeSpecialist"] == specialist
                and result["requiresDevelopmentBrief"] is requires_brief
                and actual_skills == sorted(expected_skills)
                and not excluded_specialists.intersection(
                    p.split("/")[-2] for p in actual_skills
                )
                and result["execution"] == "NOT_EXECUTED"
                and result["ciTriggered"] is False
                and result["knownImpact"] is None
                and result["rankedKnowledge"] is None
                and result["writePreflight"]["status"] == "NOT_REQUESTED"
                and len(required) <= 5
                and len(required) == len(set(required))
                and len({p["path"] for p in conditional}) == len(conditional)
                and (mode not in {"plan", "context-recovery"} or not actual_skills)
            )
            if not safe:
                issues.append(f"{identity}: source context projection diverges from declared route")
            rows.append({
                "id": identity, "mode": mode, "sourceProjectionConformant": safe,
                "requiredContextFiles": len(required),
                "conditionalContextFiles": len(conditional),
                "activatedSkills": len(actual_skills),
            })
        except (KeyError, TypeError, ValueError, OSError) as exc:
            issues.append(f"{identity}: source planner rejected declared expectation: {exc}")
            rows.append({"id": identity, "mode": mode, "sourceProjectionConformant": False})
    complete = sum(bool(row["sourceProjectionConformant"]) for row in rows)
    return {
        "cases": len(rows),
        "conformant": complete,
        "sourceProjectionConformance": complete == len(rows) and len(rows) > 0,
        "requiredContextCounts": dict(sorted(Counter(
            row.get("requiredContextFiles", -1) for row in rows
        ).items())),
        "conditionalContextCounts": dict(sorted(Counter(
            row.get("conditionalContextFiles", -1) for row in rows
        ).items())),
        "rows": rows,
        "issues": issues,
        "modelRoutingAccuracy": None,
        "observedAgentBehaviorVerified": False,
    }


def check_declared_permissions(cases: list[dict[str, Any]],
                               policy: dict[str, Any]) -> dict[str, Any]:
    """Reuse the one existing permission evaluator and golden corpus."""
    issues: list[str] = []
    by_decision: Counter[str] = Counter()
    for item in cases:
        actual = evaluate_permission(item["request"], policy)["decision"]
        by_decision[actual] += 1
        if actual != item["expected"]:
            issues.append(f"{item['id']}: permission expected {item['expected']}, got {actual}")
    return {
        "cases": len(cases),
        "decisions": dict(sorted(by_decision.items())),
        "sourcePermissionConformance": bool(cases) and not issues,
        "issues": issues,
    }


def check_source_impact(root: Path, *, extended: bool = False) -> dict[str, Any]:
    """Check cheap repository-domain planning; optionally inspect one source graph."""
    issues: list[str] = []
    doc = plan_impacted([DOC_IMPACT], root=root)
    if ("repository" not in doc["domains"]
            or doc["sourceDependencyGraph"] is not None
            or doc["proof"] != "PLANNING_ONLY_NOT_EXECUTED"
            or doc["ciTriggered"]):
        issues.append("documentation-only change triggered unnecessary source graph or overstated proof")
    source: dict[str, Any] | None = None
    if extended:
        impacted = plan_impacted([SOURCE_IMPACT], root=root)
        graph = impacted["sourceDependencyGraph"]
        if (graph is None or graph["fullClosureProven"] is not False
                or impacted["proof"] != "PLANNING_ONLY_NOT_EXECUTED"
                or impacted["ciTriggered"]):
            issues.append("cross-language affected proof is not conservatively bounded")
        source = {
            "path": SOURCE_IMPACT,
            "domains": impacted["domains"],
            "knownConsumers": len(graph["affectedSourceConsumers"]) if graph else None,
            "unknownImports": len(graph["affectedUnknownImports"]) if graph else None,
            "fullClosureProven": False,
        }
    return {
        "documentOnly": {
            "path": DOC_IMPACT,
            "domains": doc["domains"],
            "sourceGraphBuilt": doc["sourceDependencyGraph"] is not None,
        },
        "optionalSourceGraph": source,
        "conservativeImpactConformance": not issues,
        "issues": issues,
    }


def benchmark(root: Path = ROOT, *, extended: bool = False) -> dict[str, Any]:
    """Read-only conformance and cost-shape report; no saved benchmark database."""
    issues = validate_agent_evals(root) + admit_skills(root) + verify_knowledge(root)
    routing = _json(root / ROUTING)
    permissions = _json(root / PERMISSIONS)
    policy = _json(root / ".agents/permissions/permission-policy.json")
    route_report = replay_declared_routing(routing["cases"], root=root)
    permission_report = check_declared_permissions(permissions["cases"], policy)
    impact_report = check_source_impact(root, extended=extended)
    documents, links, catalog_issues = catalog(root)
    issues.extend(catalog_issues)
    issues.extend(route_report["issues"])
    issues.extend(permission_report["issues"])
    issues.extend(impact_report["issues"])
    return {
        "schemaVersion": 1,
        "type": "SOURCE_DERIVED_METHOD_FLOW_CONFORMANCE",
        "sourceChecksConformant": not issues,
        "inventory": {
            "documents": len(documents),
            "documentRelations": len(links),
            "routingCases": route_report["cases"],
            "permissionCases": permission_report["cases"],
            "procedureCases": len(_json(root / ".agents/evals/skill-procedure.json")["cases"]),
            "registeredSkills": len(_json(root / ".agents/skill-registry.json")["skills"]),
        },
        "routingReplay": route_report,
        "permissionEvaluation": permission_report,
        "affectedPlanning": impact_report,
        "issues": sorted(set(issues)),
        "proof": "EXECUTED_SOURCE_PLAN_REPLAY_ONLY_WHEN_RUN",
        "observedAgentBehaviorVerified": False,
        "modelRoutingAccuracy": None,
        "modelRuns": 0,
        "ciTriggered": False,
        "targetWindowsAcceptanceVerified": False,
        "performanceGainMeasured": False,
        "unverified": [
            "actual AI specialist selection and procedure obedience",
            "real task runtime, token savings and end-to-end speedup",
            "compiler/test execution outside this source-planning tool",
            "installed target Windows, audio/device/GPU and meeting acceptance",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="read source and evaluate declared conformance")
    parser.add_argument("--extended", action="store_true",
                        help="opt into one source-import graph projection")
    parser.add_argument("--summary", action="store_true",
                        help="hide per-case rows; keep all failure evidence")
    args = parser.parse_args()
    try:
        result = benchmark(extended=args.extended)
    except (OSError, ValueError, TypeError, KeyError) as exc:
        parser.exit(2, f"benchmark source invalid: {exc}\n")
    if args.summary:
        result["routingReplay"].pop("rows", None)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["sourceChecksConformant"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

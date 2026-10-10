"""Validate TranslateIT skill-routing expectations and score external agent-run receipts.

A valid golden corpus is policy structure, not evidence of model routing accuracy.
No model/API is invoked. An observed receipt must come from an actual external run.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ".agents/evals/manifest.json"
ROUTING = ".agents/evals/skill-routing.json"
PROCEDURES = ".agents/evals/skill-procedure.json"
KINDS = {"positive", "collision", "pressure", "no-skill"}
PROCEDURE_KINDS = {"positive", "collision", "pressure"}
DISPOSITIONS = {"BOUNDED_WORK", "REPORT_RESIDUE", "REQUEST_DECISION", "PROPOSE_HANDOFF"}
ID = re.compile(r"^[a-z][a-z0-9-]+$")


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("expected a JSON object: " + str(path))
    return value


def validate_agent_evals(root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    manifest = _read_json(root / MANIFEST)
    corpus = _read_json(root / ROUTING)
    registry = _read_json(root / ".agents/skill-registry.json")
    permissions = _read_json(root / ".agents/permissions/permission-policy.json")
    if manifest.get("schemaVersion") != 1 or corpus.get("schemaVersion") != 1:
        errors.append("agent eval manifest/routing schema version is not 1")
    if manifest.get("routingCorpus") != ROUTING or manifest.get("permissionCorpus") != ".agents/evals/permission-cases.json":
        errors.append("agent eval corpus ownership differs from canonical paths")
    if manifest.get("scorer") != "tools/repository_agent_evals.py":
        errors.append("agent eval scorer ownership differs from canonical helper")
    if (manifest.get("procedureCorpus") != PROCEDURES or
            manifest.get("procedureScorer") != "tools/repository_agent_evals.py"):
        errors.append("procedure scorer/corpus differs from canonical manifest")
    specialists = {
        item["id"] for item in registry["skills"]
        if item.get("kind") == "specialist"
    }
    modes = set(permissions["modes"])
    scopes = set(permissions["scopes"])
    ids: set[str] = set()
    routed_modes: set[str] = set()
    routed_specialists: set[str] = set()
    kinds: set[str] = set()
    entries = corpus.get("cases")
    if not isinstance(entries, list) or not entries:
        return errors + ["skill-routing corpus must contain cases"]
    for case in entries:
        if not isinstance(case, dict):
            errors.append("routing case must be an object")
            continue
        key = case.get("id")
        if not isinstance(key, str) or not ID.fullmatch(key) or key in ids:
            errors.append("invalid or duplicate routing case ID: " + str(key))
        if isinstance(key, str):
            ids.add(key)
        kind = case.get("kind")
        if kind not in KINDS:
            errors.append(f"{key}: unknown routing case kind")
        else:
            kinds.add(kind)
        if kind == "no-skill" and case.get("expectedSpecialist") is not None:
            errors.append(f"{key}: no-skill case cannot activate a specialist")
        prompt = case.get("prompt")
        if not isinstance(prompt, str) or not (10 <= len(prompt.strip()) <= 450):
            errors.append(f"{key}: routing prompt length invalid")
        mode = case.get("expectedMode")
        specialist = case.get("expectedSpecialist")
        scope = case.get("expectedScope")
        forbids = case.get("forbiddenSpecialists")
        if mode not in modes:
            errors.append(f"{key}: mode not in canonical permission-policy")
        else:
            routed_modes.add(mode)
        if specialist is not None and specialist not in specialists:
            errors.append(f"{key}: specialist is not a registered domain specialist")
        if specialist in specialists:
            routed_specialists.add(specialist)
        if not isinstance(forbids, list) or not forbids or len(forbids) != len(set(map(str, forbids))):
            errors.append(f"{key}: negative-routing constraints missing/duplicated")
        elif any(forbidden not in specialists for forbidden in forbids):
            errors.append(f"{key}: forbidden specialist must be registered")
        elif specialist in forbids:
            errors.append(f"{key}: expected specialist is also forbidden")
        if scope is not None and scope not in scopes:
            errors.append(f"{key}: expected scope is not registered")
        if specialist is not None and scope != specialist:
            errors.append(f"{key}: declared specialist and scope disagree")
        if mode in ("plan", "context-recovery") and (scope is not None or specialist is not None):
            errors.append(f"{key}: read-only case must not activate a write scope or specialist")
        if case.get("requiresBrief") is not (mode == "complex-development"):
            errors.append(f"{key}: development-brief activation disagrees with work mode")
    if routed_modes != modes:
        errors.append("routing corpus does not cover every work mode")
    if routed_specialists != specialists:
        errors.append("routing corpus does not cover every registered specialist")
    if kinds != KINDS or len(entries) < 24:
        errors.append("routing corpus lacks positive/collision/pressure/no-skill coverage")
    errors.extend(validate_agent_procedures(root))
    return sorted(set(errors))



def validate_agent_procedures(root: Path = ROOT, corpus: dict[str, Any] | None = None) -> list[str]:
    """Check evaluation structure only. Each SKILL.md owns its actual procedure."""
    errors: list[str] = []
    data = corpus if corpus is not None else _read_json(root / PROCEDURES)
    if data.get("schemaVersion") != 1 or not isinstance(data.get("cases"), list):
        return ["invalid procedure corpus schema"]
    routing = {case["id"]: case for case in _read_json(root / ROUTING)["cases"]}
    registry = _read_json(root / ".agents/skill-registry.json")["skills"]
    skills = {skill["id"] for skill in registry}
    specialists = {skill["id"] for skill in registry if skill["kind"] == "specialist"}
    ids: set[str] = set()
    routed: set[str] = set()
    positive_skills: set[str] = set()
    kinds: set[str] = set()
    no_skill, handoffs = 0, 0
    for case in data["cases"]:
        if not isinstance(case, dict):
            errors.append("procedure case is not an object")
            continue
        key = case.get("id")
        if not isinstance(key, str) or not ID.fullmatch(key) or key in ids:
            errors.append("invalid or duplicate procedure case ID: " + str(key))
            continue
        ids.add(key)
        route_id = case.get("routingCaseId")
        if route_id not in routing or route_id in routed:
            errors.append(f"{key}: missing or duplicated routing case reference")
            continue
        routed.add(route_id)
        route = routing[route_id]
        skill = case.get("skill")
        if skill not in skills | {None}:
            errors.append(f"{key}: procedure skill not registered")
        elif skill is None:
            no_skill += 1
            if route["expectedSpecialist"] is not None or route["requiresBrief"]:
                errors.append(f"{key}: no-skill procedure conflicts with routing")
        elif skill == "development-brief":
            if route["expectedMode"] != "complex-development" or not route["requiresBrief"]:
                errors.append(f"{key}: development brief not eligible for this route")
        elif skill != route["expectedSpecialist"]:
            errors.append(f"{key}: specialist differs from expected routing")
        kind = case.get("kind")
        if kind not in PROCEDURE_KINDS:
            errors.append(f"{key}: unknown procedure case kind")
        else:
            kinds.add(kind)
            if kind == "positive" and skill in skills:
                positive_skills.add(skill)
        required = case.get("requiredMilestones")
        forbidden = case.get("forbiddenActions")
        for name, labels in (("requiredMilestones", required), ("forbiddenActions", forbidden)):
            if (not isinstance(labels, list) or not 2 <= len(labels) <= 8 or
                any(not isinstance(v, str) or not ID.fullmatch(v) for v in labels)):
                errors.append(f"{key}: {name} contains invalid milestone/action IDs")
            elif len(labels) != len(set(labels)):
                errors.append(f"{key}: {name} repeats milestone/action IDs")
        if isinstance(required, list) and isinstance(forbidden, list):
            if all(isinstance(v, str) for v in required + forbidden):
                if set(required) & set(forbidden):
                    errors.append(f"{key}: a milestone is also forbidden")
        disposition = case.get("expectedDisposition")
        target = case.get("handoffTarget")
        if disposition not in DISPOSITIONS:
            errors.append(f"{key}: invalid procedure disposition")
        elif disposition == "PROPOSE_HANDOFF":
            handoffs += 1
            if target not in specialists or target == skill:
                errors.append(f"{key}: handoff target must be a different specialist")
        elif target is not None:
            errors.append(f"{key}: handoff target without handoff disposition")
    if len(data["cases"]) < 16 or kinds != PROCEDURE_KINDS:
        errors.append("procedure cases missing positive/collision/pressure coverage")
    if positive_skills != skills:
        errors.append("positive cases do not cover every canonical skill")
    if no_skill < 2 or handoffs < 1:
        errors.append("procedure cases missing no-skill or handoff boundaries")
    return sorted(set(errors))


def score_procedure_runs(observed: dict[str, Any], root: Path = ROOT) -> dict[str, Any]:
    """Score provider-supplied receipts; claims within receipts are not verified."""
    problems = validate_agent_procedures(root)
    if problems:
        raise ValueError("invalid procedure corpus: " + "; ".join(problems[:3]))
    if observed.get("schemaVersion") != 1 or not isinstance(observed.get("runs"), list):
        raise ValueError("procedure receipts need schemaVersion 1 and runs[]")
    cases = _read_json(root / PROCEDURES)["cases"]
    expected = {case["id"]: case for case in cases}
    seen: set[str] = set()
    exact = 0
    failures: list[dict] = []
    for run in observed["runs"]:
        if not isinstance(run, dict) or run.get("id") not in expected or run["id"] in seen:
            raise ValueError("unknown or duplicated procedure receipt case ID")
        case = expected[run["id"]]
        seen.add(run["id"])
        milestones = run.get("observedMilestones")
        actions = run.get("observedActions")
        if not isinstance(milestones, list) or not isinstance(actions, list) or any(
            not isinstance(v, str) or not ID.fullmatch(v)
            for v in milestones + actions
        ):
            raise ValueError("observed procedure milestones/actions must have structured IDs")
        ref = run.get("evidenceRef")
        checks = {
            "skill": run.get("selectedSkill") == case["skill"],
            "required": set(case["requiredMilestones"]).issubset(milestones),
            "forbidden": set(case["forbiddenActions"]).isdisjoint(actions),
            "disposition": run.get("disposition") == case["expectedDisposition"],
            "handoff": run.get("handoffTarget") == case["handoffTarget"],
            "evidence": isinstance(ref, str) and re.fullmatch(
                r"[A-Za-z0-9][A-Za-z0-9:/#._-]{4,239}", ref
            ) is not None,
        }
        if all(checks.values()):
            exact += 1
        else:
            failures.append({"id": run["id"], **checks})
    missing = sorted(set(expected) - seen)
    return {
        "schemaVersion": 1,
        "cases": len(cases),
        "observed": len(seen),
        "exactMatches": exact,
        "coverage": len(seen) / len(cases),
        "receiptConsistency": exact / len(seen) if seen else None,
        "complete": not missing,
        "passed": bool(seen) and not failures and not missing,
        "missing": missing,
        "failures": failures,
        "proof": "EXTERNAL_RECEIPT_CONSISTENCY_ONLY",
        "observedAgentBehaviorVerified": False,
        "ciTriggered": False,
    }


def score_agent_runs(observed: dict[str, Any], root: Path = ROOT) -> dict[str, Any]:
    """Score actual externally supplied decisions; never synthesize agent decisions."""
    cases = _read_json(root / ROUTING)["cases"]
    if observed.get("schemaVersion") != 1 or not isinstance(observed.get("runs"), list):
        raise ValueError("agent-run receipts must use schemaVersion 1 and runs[]")
    expected = {case["id"]: case for case in cases}
    seen: set[str] = set()
    failures: list[dict] = []
    exact = 0
    for run in observed["runs"]:
        if not isinstance(run, dict) or run.get("id") not in expected or run["id"] in seen:
            raise ValueError("unknown or duplicate agent-run case ID")
        case = expected[run["id"]]
        seen.add(run["id"])
        mode_ok = run.get("mode") == case["expectedMode"]
        specialist_ok = run.get("specialist") == case["expectedSpecialist"]
        scope_ok = run.get("scope") == case["expectedScope"]
        brief_ok = run.get("requiresBrief") is case["requiresBrief"]
        forbids_ok = run.get("specialist") not in case["forbiddenSpecialists"]
        if all((mode_ok, specialist_ok, scope_ok, brief_ok, forbids_ok)):
            exact += 1
        else:
            failures.append({
                "id": run["id"],
                "mode": mode_ok, "specialist": specialist_ok,
                "scope": scope_ok, "brief": brief_ok, "forbidden": forbids_ok,
            })
    missing = sorted(set(expected) - seen)
    # Missing cases are incomplete evidence, never a passing score.
    return {
        "schemaVersion": 1,
        "cases": len(cases),
        "observed": len(seen),
        "exactMatches": exact,
        "coverage": len(seen) / len(cases),
        "observedAccuracy": exact / len(seen) if seen else None,
        "complete": not missing,
        "passed": bool(seen) and not failures and not missing,
        "missing": missing,
        "failures": failures,
        "proof": "external-agent-observations-only",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_mutually_exclusive_group(required=True)
    commands.add_argument("--check", action="store_true")
    commands.add_argument("--score", type=Path, metavar="AGENT_RUNS_JSON")
    commands.add_argument("--score-procedures", type=Path, metavar="PROCEDURE_RUNS_JSON")
    args = parser.parse_args()
    try:
        if args.check:
            issues = validate_agent_evals()
            print(json.dumps({"passed": not issues, "issues": issues}, indent=2))
            return int(bool(issues))
        if args.score_procedures:
            result = score_procedure_runs(_read_json(args.score_procedures))
        else:
            result = score_agent_runs(_read_json(args.score))
        print(json.dumps(result, indent=2))
        return 0 if result["passed"] else 1
    except (OSError, ValueError, KeyError, TypeError) as exc:
        parser.exit(2, f"agent evaluation input invalid: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

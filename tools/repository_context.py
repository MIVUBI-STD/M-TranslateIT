"""Read-only development context projection, not an AI router or workflow executor.

Explicit task/mode/scope comes from current user + root AGENTS.md. Reuse the
canonical skill inventory, permission policy, document retrieval, and impact
planner instead of reimplementing their semantics or persisting task state.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from repository_impact import canonical_path, plan_impacted
from repository_knowledge import retrieve
from repository_permissions import evaluate_permission, load_policy

ROOT = Path(__file__).resolve().parents[1]
TASKS = {
    "context-recovery": frozenset(("INSPECT", "DIAGNOSE")),
    "plan": frozenset(("PLAN",)),
    "bounded-maintenance": frozenset(("DEVELOP", "REPAIR", "VALIDATE")),
    "standard-development": frozenset(("DEVELOP", "REPAIR", "VALIDATE", "RELEASE")),
    "complex-development": frozenset(("DEVELOP", "REPAIR", "VALIDATE", "RELEASE")),
}
DEFAULT_TASK = {
    "context-recovery": "INSPECT",
    "plan": "PLAN",
    "bounded-maintenance": "REPAIR",
    "standard-development": "DEVELOP",
    "complex-development": "DEVELOP",
}
ROOT_CONTEXT = ("AGENTS.md", "GITHUB_RULES.md")
DEVELOPMENT_DISCIPLINE = "docs/knowledge/development-discipline.md"
BRIEF = ".agents/skills/development-brief/SKILL.md"
CONTINUATION = "planning/development.md"
OWNER_MAP = "docs/knowledge/source-ownership.md"


def _nonempty(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 1024:
        raise ValueError(f"{field} must be nonempty bounded text")
    return value.strip()


def _unique_paths(paths: list[str] | None) -> list[str]:
    if paths is None:
        return []
    if not isinstance(paths, list):
        raise ValueError("paths must be an array")
    return sorted({canonical_path(path) for path in paths})


def _handoff(
    source_scope: str | None,
    value: dict[str, Any],
    registered_scopes: set[str],
) -> dict[str, str]:
    if not isinstance(value, dict):
        raise ValueError("handoff must be a structured object")
    if not source_scope:
        raise ValueError("handoff requires a known source scope")
    target = _nonempty(value.get("target_scope"), "handoff target_scope")
    if target == source_scope or target not in registered_scopes:
        raise ValueError("handoff must name a distinct, registered target scope")
    fields = {
        "observed": _nonempty(value.get("observed"), "handoff observed"),
        "expected": _nonempty(value.get("expected"), "handoff expected"),
        "minimum_evidence": _nonempty(value.get("minimum_evidence"), "handoff minimum_evidence"),
        "resume_stage": _nonempty(value.get("resume_stage"), "handoff resume_stage"),
    }
    return {
        "state": "PROPOSED_NOT_ACTIVATED",
        "source_scope": source_scope,
        "target_scope": target,
        **fields,
        "authorization": "NO_IMPLICIT_SCOPE_SWITCH",
    }


def plan_context(
    *,
    intent: str,
    mode: str,
    task: str | None = None,
    scope: str | None = None,
    owner: str | None = None,
    changed: list[str] | None = None,
    proposed_writes: list[str] | None = None,
    activate_specialist: bool = False,
    resume: bool = False,
    knowledge_query: str | None = None,
    knowledge_domain: str | None = None,
    handoff: dict[str, Any] | None = None,
    branch: str = "Local",
    root: Path = ROOT,
) -> dict[str, Any]:
    """Project minimum decision context; never choose a lane from prompt keywords."""
    _nonempty(intent, "intent")  # Deliberately not echoed to avoid recording user data.
    if branch != "Local":
        raise ValueError("development context is bound to Local; main is final-only")
    policy = load_policy(root / ".agents/permissions/permission-policy.json")
    inventory = json.loads((root / ".agents/skill-registry.json").read_text(encoding="utf-8"))
    if mode not in TASKS or mode not in policy["modes"]:
        raise ValueError("unknown work mode; select it explicitly from AGENTS.md")
    selected_task = task or DEFAULT_TASK[mode]
    if selected_task not in TASKS[mode]:
        raise ValueError("task class is not allowed by the selected work mode")
    scopes = set(policy["scopes"])
    specialists = {
        item["id"] for item in inventory["skills"] if item["kind"] == "specialist"
    }
    if scope is not None and scope not in scopes:
        raise ValueError("unknown semantic scope; do not invent a specialist")
    read_only = policy["modes"][mode] == "read-only"
    writes = _unique_paths(proposed_writes)
    changed_paths = _unique_paths(changed)
    if read_only and (scope is not None or writes or activate_specialist or handoff):
        raise ValueError("INSPECT/PLAN cannot request writes, specialists or handoffs")
    if selected_task == "VALIDATE" and writes:
        raise ValueError("VALIDATE is evidence-only, not mutation authorization")
    if selected_task == "RELEASE" and scope != "release-packaging-development":
        raise ValueError("RELEASE requires the existing release-packaging scope")
    if activate_specialist and scope not in specialists:
        raise ValueError("only an existing semantic specialist may be activated")
    if activate_specialist and selected_task in {"VALIDATE", "RELEASE"}:
        raise ValueError("verification or publication does not activate development specialists")
    if handoff is not None and selected_task not in {"DEVELOP", "REPAIR"}:
        raise ValueError("handoff requires a development or repair task")

    exact_owner = canonical_path(owner) if owner is not None else None
    if exact_owner is not None:
        if not (root / exact_owner).is_file():
            raise ValueError("exact source owner is absent; use the canonical owner map")
        permission = evaluate_permission(
            {"mode": mode, "action": "read", "path": exact_owner}, policy
        )
        if permission["decision"] != "allow":
            raise ValueError("source owner cannot be read without explicit authorization")

    required = list(ROOT_CONTEXT)
    if not read_only:
        required.append(DEVELOPMENT_DISCIPLINE)
    if mode == "complex-development":
        required.append(BRIEF)
    if exact_owner is not None:
        required.append(exact_owner)
    if resume:
        required.append(CONTINUATION)
    if activate_specialist:
        required.append(f".agents/skills/{scope}/SKILL.md")
    required = list(dict.fromkeys(required))
    for path in required:
        if not (root / path).is_file():
            raise ValueError(f"required context owner is missing: {path}")

    conditional: list[dict[str, str]] = []
    def add_conditional(path: str, reason: str) -> None:
        if path not in required and not any(item["path"] == path for item in conditional):
            conditional.append({"path": path, "when": reason})

    if exact_owner is None:
        add_conditional(OWNER_MAP, "source/consumer ownership not yet identified")
    if not resume:
        add_conditional(CONTINUATION, "actual unfinished task continuity is material")
    add_conditional("CONTEXT.md", "stable product facts affect current decision")
    if selected_task not in {"INSPECT", "PLAN"}:
        add_conditional("docs/foundation/README.md", "product law or acceptance changes")
    if scope is None:
        add_conditional("docs/knowledge/skills/activation-matrix.md",
                        "semantic specialist or scope is unresolved")
    elif scope in specialists and not activate_specialist:
        add_conditional(f".agents/skills/{scope}/SKILL.md",
                        "domain procedure changes a material decision")
    add_conditional("docs/knowledge/current-validation.md",
                    "actual previous proof status changes decision")
    for item in conditional:
        if not (root / item["path"]).is_file():
            raise ValueError(f"conditional context owner is missing: {item['path']}")

    # One active scope; private/untrusted source does not become a context read.
    checks = []
    for path in writes:
        decision = evaluate_permission(
            {"mode": mode, "action": "write", "scope": scope, "path": path}, policy
        )
        checks.append({"path": path, **decision})
    overall = ("DENY" if any(x["decision"] == "deny" for x in checks)
               else "ASK" if any(x["decision"] == "ask" for x in checks)
               else "ADVISORY_ALLOW" if checks else "NOT_REQUESTED")
    if selected_task == "RELEASE" and overall != "DENY":
        overall = "ASK"
    release_gate = (evaluate_permission(
        {"mode": mode, "action": "release-publish", "scope": scope}, policy
    ) if selected_task == "RELEASE" else None)
    ranked = []
    if knowledge_query is not None:
        _nonempty(knowledge_query, "knowledge query")
        if knowledge_domain not in {"foundation", "knowledge", "system"}:
            raise ValueError("ranked retrieval requires an explicit documentation domain")
        ranked = retrieve(knowledge_query, knowledge_domain, root=root, limit=4)
    elif knowledge_domain is not None:
        raise ValueError("documentation domain without a query is not a task")
    impact = plan_impacted(changed_paths, root=root) if changed_paths else None
    proposed_handoff = _handoff(scope, handoff, scopes) if handoff is not None else None

    return {
        "schemaVersion": 1,
        "branch": "Local",
        "mode": mode,
        "task": selected_task,
        "lane": "READ_ONLY" if read_only else "DEVELOPMENT",
        "scope": scope,
        "exactOwner": exact_owner,
        "ownerResolved": exact_owner is not None,
        "activeSpecialist": scope if activate_specialist else None,
        "requiresDevelopmentBrief": mode == "complex-development",
        "context": {
            "REQUIRED": required,
            "CONDITIONAL": conditional,
            "EXCLUDED": [
                "unselected specialist skills",
                "unrelated documentation domains",
                "full repository and historical transcript scans",
                "target Windows/device proof unless explicitly needed",
            ],
        },
        "writePreflight": {"status": overall, "paths": checks,
                           "advisoryOnly": True},
        "releasePreflight": release_gate,
        "handoff": proposed_handoff,
        "knownImpact": impact,
        "rankedKnowledge": {"status": "RANKING_NOT_AUTHORITY",
                            "matches": ranked} if knowledge_query is not None else None,
        "execution": "NOT_EXECUTED",
        "ciTriggered": False,
        "next": ("STOP_AND_RESOLVE_SCOPE_OR_APPROVAL" if overall in {"ASK", "DENY"}
                 else "READ_REQUIRED_CONTEXT_THEN_VALIDATE_OWNER"),
    }


def verify_context(root: Path = ROOT) -> list[str]:
    """A small structural smoke check. Model routing is evaluated separately."""
    cases = [
        {"intent": "Amati source", "mode": "context-recovery"},
        {"intent": "Plan architecture", "mode": "plan"},
        {"intent": "Perbaiki governance", "mode": "standard-development",
         "scope": "governance", "owner": "GITHUB_RULES.md",
         "proposed_writes": ["docs/knowledge/flow.md"]},
        {"intent": "Diagnose complex issue", "mode": "complex-development"},
    ]
    issues = []
    for case in cases:
        try:
            plan = plan_context(root=root, **case)
            if plan["execution"] != "NOT_EXECUTED" or plan["ciTriggered"]:
                issues.append("context planner claimed execution")
            if case["mode"] == "complex-development" and BRIEF not in plan["context"]["REQUIRED"]:
                issues.append("complex development failed to include brief")
        except (OSError, ValueError, KeyError, TypeError) as exc:
            issues.append(f"context projection invalid: {exc}")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--intent")
    parser.add_argument("--mode")
    parser.add_argument("--task")
    parser.add_argument("--scope")
    parser.add_argument("--owner")
    parser.add_argument("--changed", action="append")
    parser.add_argument("--write", action="append")
    parser.add_argument("--activate-specialist", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--knowledge-query")
    parser.add_argument("--knowledge-domain", choices=("foundation", "knowledge", "system"))
    parser.add_argument("--handoff", help="Explicit JSON handoff; never switches lanes")
    parser.add_argument("--branch", default="Local")
    args = parser.parse_args()
    try:
        if args.check:
            errors = verify_context()
            print(json.dumps({"passed": not errors, "errors": errors}, indent=2))
            return int(bool(errors))
        value = json.loads(args.handoff) if args.handoff else None
        plan = plan_context(
            intent=args.intent, mode=args.mode, task=args.task,
            scope=args.scope, owner=args.owner,
            changed=args.changed, proposed_writes=args.write,
            activate_specialist=args.activate_specialist, resume=args.resume,
            knowledge_query=args.knowledge_query, knowledge_domain=args.knowledge_domain,
            handoff=value, branch=args.branch,
        )
        print(json.dumps(plan, indent=2))
        return 0 if plan["writePreflight"]["status"] not in {"ASK", "DENY"} else 1
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        parser.exit(2, f"context projection invalid: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

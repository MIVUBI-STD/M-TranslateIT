from __future__ import annotations

import json
import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]

REQUIRED_PATHS = (
    "README.md",
    "AGENTS.md",
    "GITHUB_RULES.md",
    "CONTEXT.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "DEV.cmd",
    "toolchain.json",
    "tooling/windows-toolchain/dev.ps1",
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".github/CODEOWNERS",
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/workflows/repository-verify.yml",
    ".github/workflows/code-health.yml",
    ".github/workflows/milmmt-repo-contract.yml",
    ".github/workflows/asr-quality-contract.yml",
    ".github/workflows/tts-quality-contract.yml",
    ".github/workflows/quality-readiness-contract.yml",
    ".github/workflows/workerruntime-lock.yml",
    ".github/workflows/release-payload-verify.yml",
    "docs/README.md",
    "docs/foundation/README.md",
    "docs/knowledge/skills/README.md",
    "tools/repository_knowledge.py",
    "tools/repository_contracts.py",
    "tools/repository_impact.py",
    "tools/tests/test_repository_impact.py",
    "tools/interop-contracts.json",
    "tools/tests/test_repository_knowledge.py",
    "tools/tests/test_repository_contracts.py",
    "docs/foundation/01-product-overview.md",
    "docs/foundation/02-product-requirements.md",
    "docs/foundation/03-acceptance-scenarios.md",
    "docs/README.md",
    "docs/foundation/README.md",
    "docs/knowledge/README.md",
    "docs/knowledge/skills/README.md",
    "docs/knowledge/flow.md",
    "docs/knowledge/development-discipline.md",
    "docs/knowledge/next-action.md",
    "docs/knowledge/current-validation.md",
    "docs/knowledge/source-ownership.md",
    "docs/knowledge/decision-log.md",
    "docs/knowledge/decisions/README.md",
    "docs/knowledge/decisions/history-legacy.md",
    "docs/knowledge/operations/README.md",
    "docs/knowledge/skills/activation-matrix.md",
    "docs/knowledge/skills/skill-map.md",
    ".agents/evals/manifest.json",
    ".agents/evals/skill-routing.json",
    "tools/repository_agent_evals.py",
    "tools/tests/test_repository_agent_evals.py",
    ".agents/skills/development-brief/SKILL.md",
    ".agents/skills/desktop-runtime-development/SKILL.md",
    ".agents/skills/desktop-ui-design-development/SKILL.md",
    ".agents/skills/local-ai-runtime-development/SKILL.md",
    ".agents/skills/windows-audio-runtime-development/SKILL.md",
    ".agents/skills/release-packaging-development/SKILL.md",
    "EngineData/Frontend/RustApp/scripts/validate_bridge_contract.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_tauri_command_args.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_application_runtime_architecture.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_frontend_reachability.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_runtime_api_usage.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_test_reference_reachability.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_test_ci_routing.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_source_size_budget.mjs",
    "EngineData/Frontend/RustApp/scripts/tests/code_health_proof_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/workflow_proof_surfaces_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/tauri_capability_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_controller_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/app_listener_lifecycle_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_event_deep_refresh_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_event_ordering_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_event_reason_policy_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_event_snapshot_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_intent_parity_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/runtime_event_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/runtime_settings_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/worker_task_protocol_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/worker_status_protocol_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/application_unavailable_snapshot_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/worker_request_metadata_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/voice_build_runtime_sync_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/voice_event_refresh_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/helper_lifecycle_authority_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/readiness_authority_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/shell_controller_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/product_state_split_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/native_close_dialog_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/meeting_reconcile_contract.test.ts",
    "EngineData/Frontend/RustApp/scripts/tests/meeting_diagnostics_owner_contract.test.ts",
    "EngineData/Frontend/RustApp/src-tauri/capabilities/default.json",
    "EngineData/Frontend/RustApp/src-tauri/capabilities/translation-overlay.json",
)

FORBIDDEN_PATHS = (
    "DevelopingData",
    ".github/workflows/stable-release-verify.yml",
    "tools/verify_frontend_runtime_policy_tests.py",
    "EngineData/Frontend/RustApp/scripts/validate_bridge_type_safety.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_command_parity.mjs",
)

ACTIVE_GOVERNANCE = (
    "README.md",
    "AGENTS.md",
    "GITHUB_RULES.md",
    "CONTEXT.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "docs/foundation/01-product-overview.md",
    "docs/foundation/02-product-requirements.md",
    "docs/foundation/03-acceptance-scenarios.md",
    "docs/knowledge/README.md",
    "docs/knowledge/flow.md",
    "docs/knowledge/next-action.md",
    "docs/knowledge/current-validation.md",
    "docs/knowledge/source-ownership.md",
    "docs/knowledge/decision-log.md",
    "docs/knowledge/decisions/README.md",
    "docs/knowledge/operations/README.md",
    "docs/knowledge/skills/activation-matrix.md",
    "docs/knowledge/skills/skill-map.md",
)


LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
ACTION_RE = re.compile(r"(?m)^\s*uses:\s+([^@\s]+)@([^\s#]+)(?:\s+#\s*(.+))?$")
SHA40_RE = re.compile(r"^[0-9a-f]{40}$")
MAIN_BRANCH_LINE_RE = re.compile(r"(?m)^\s*-\s+main\s*$")
AUTO_WORKFLOW_EVENT_RE = re.compile(r"(?m)^  (?:push|pull_request|pull_request_target|schedule|workflow_run|release|create|delete|merge_group|issue_comment):\s*$")


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def text(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def check_structure(errors: list[str]) -> None:
    for rel in REQUIRED_PATHS:
        if not (ROOT / rel).is_file():
            fail(errors, f"missing required path: {rel}")
    for rel in FORBIDDEN_PATHS:
        if (ROOT / rel).exists():
            fail(errors, f"stale or forbidden path remains active: {rel}")

    skills_root = ROOT / ".agents" / "skills"
    if not skills_root.is_dir():
        fail(errors, "missing .agents/skills")
        return
    actual = {p.name for p in skills_root.iterdir() if p.is_dir() and (p / "SKILL.md").is_file()}
    registry_path = ROOT / ".agents" / "skill-registry.json"
    if registry_path.is_file():
        try:
            registry = json.loads(registry_path.read_text(encoding="utf-8"))
            entries = registry["skills"]
            expected = {entry["id"] for entry in entries}
            if registry.get("schemaVersion") != 1 or len(expected) != len(entries):
                fail(errors, "agent skill registry version/unique identity mismatch")
            if actual != expected:
                fail(errors, f"agent skill inventory mismatch: expected={sorted(expected)} actual={sorted(actual)}")
            for entry in entries:
                skill = entry["id"]
                if entry.get("kind") not in ("planning", "specialist") or not isinstance(entry.get("domain"), str):
                    fail(errors, f"agent skill registry invalid classification: {skill}")
                    continue
                skill_path = skills_root / skill / "SKILL.md"
                if skill_path.is_file() and not re.search(r"(?m)^name:\s*" + re.escape(skill) + r"\s*$", skill_path.read_text(encoding="utf-8")):
                    fail(errors, f"agent skill registry SKILL.md identity mismatch: {skill}")
                if skill not in text("docs/knowledge/skills/skill-map.md"):
                    fail(errors, f"agent skill missing from canonical human skill map: {skill}")
        except (OSError, ValueError, KeyError, TypeError) as exc:
            fail(errors, f"invalid agent skill registry: {exc}")
    else:
        fail(errors, "missing .agents/skill-registry.json")


def check_compactness(errors: list[str]) -> None:
    budgets = {
        "AGENTS.md": 14_000,
        "GITHUB_RULES.md": 26_000,
        "CONTEXT.md": 9_000,
        "docs/knowledge/next-action.md": 3_000,
        "docs/knowledge/current-validation.md": 7_000,
        "docs/knowledge/source-ownership.md": 9_000,
    }
    for rel, maximum in budgets.items():
        path = ROOT / rel
        if path.is_file() and path.stat().st_size > maximum:
            fail(errors, f"{rel} exceeds compactness budget: {path.stat().st_size} > {maximum}")


def check_development_foundation(errors: list[str]) -> None:
    agents = text("AGENTS.md")
    discipline = text("docs/knowledge/development-discipline.md")
    ownership = text("docs/knowledge/source-ownership.md")
    toolchain = text("toolchain.json")
    dev = text("tooling/windows-toolchain/dev.ps1")

    for marker in ("REPO_READ", "REPO_WRITE", "LOCAL_SHELL", "CI_CONTROL", "ARTIFACT_ACCESS", "NATIVE_HOST"):
        if marker not in agents:
            fail(errors, f"AGENTS.md missing capability marker: {marker}")
    for marker in ("STATIC_SOURCE", "EXECUTED_SOURCE", "INTEGRATION_FIXTURE", "PACKAGE_SMOKE", "LIVE_RUNTIME", "NATIVE_ACCEPTANCE"):
        if marker not in discipline:
            fail(errors, f"development discipline missing proof type: {marker}")
    for marker in ("No change required?", "first wrong owner", "smallest complete change"):
        if marker not in discipline:
            fail(errors, f"development discipline missing minimum-flow marker: {marker}")
    for marker in ('"major": 22', '"version": "3.12.10"', '"minimum": "1.77"', '"minimum": "0.12.0"'):
        if marker not in toolchain:
            fail(errors, f"toolchain.json missing repository-derived policy: {marker}")
    for marker in ('"doctor"', '"setup"', '"check"', '"build"', '"test"', '"package"'):
        if marker not in dev:
            fail(errors, f"developer router missing command: {marker}")
    if "doctorMustNotInstallOrRepairAutomatically" not in toolchain or "No installation or repair was performed." not in dev:
        fail(errors, "developer doctor must remain observation-only")
    if "Unified Windows developer routing" not in ownership:
        fail(errors, "source ownership must identify the developer router owner")
    github_rules = text("GITHUB_RULES.md")
    for marker in ("Commit-based continuity and cold-start recovery", "Work:", "State:", "Proof:", "expected `Local` SHA"):
        if marker not in github_rules:
            fail(errors, f"GITHUB_RULES.md lost material commit continuity: {marker}")
    if "Short-prompt and session recovery" not in agents:
        fail(errors, "AGENTS.md must preserve source-grounded cold-start recovery")
    if "Development preflight:" not in discipline or "Completion review:" not in discipline:
        fail(errors, "development discipline lost mandatory preflight/completion review")
    for marker in (
        "CI result is evidence; branch protection/rulesets are enforcement.",
        "successful workflow still counts only as exact-SHA evidence",
    ):
        if marker not in github_rules:
            fail(errors, f"GITHUB_RULES.md missing hosted-proof enforcement boundary: {marker}")


def check_branch_authority(errors: list[str]) -> None:
    for rel in ("README.md", "AGENTS.md", "GITHUB_RULES.md", "CONTEXT.md", "CONTRIBUTING.md"):
        value = text(rel)
        if "Local-only" not in value:
            fail(errors, f"{rel} must state the Local-only repository model")
        for stale in (
            "stable/default repository authority",
            "Local → main",
            "Local -> main",
            "main stable/default authority",
            "Stable Release Gate",
            "Developing remains the GitHub default branch",
            "GitHub default/recovery branch: Developing",
            "retain Developing as a PR base",
        ):
            if stale in value:
                fail(errors, f"{rel} contains stale branch lifecycle language: {stale}")


def check_continuation_and_product(errors: list[str]) -> None:
    next_action = text("docs/knowledge/next-action.md")
    for heading in ("## Current Status", "## Active Boundary", "## Next Step"):
        if next_action.count(heading) != 1:
            fail(errors, f"next-action.md must contain exactly one {heading}")
    for stale in (
        "V1-Advance",
        "Developing` remains",
        "Start built-in voices integration",
        "stable/default authority",
        "Stable Release Gate",
        "Local → main",
    ):
        if stale in next_action:
            fail(errors, f"next-action.md contains stale continuation marker: {stale}")

    validation = text("docs/knowledge/current-validation.md")
    for heading in ("## Current Source Proof", "## Verification surfaces", "## Proof Boundaries", "## Target Windows"):
        if heading not in validation:
            fail(errors, f"current-validation.md missing section: {heading}")
    if "one SHA does not prove another SHA" not in validation:
        fail(errors, "current-validation.md must preserve exact-SHA evidence discipline")
    if "Local-only" not in validation:
        fail(errors, "current-validation.md must state Local-only source authority")

    overview = text("docs/foundation/01-product-overview.md")
    requirements = text("docs/foundation/02-product-requirements.md")
    acceptance = text("docs/foundation/03-acceptance-scenarios.md")
    for marker in ("Built-in Male/Female", "last 3 committed own-voice", "incoming remains context-free", "Svelte 5"):
        if marker not in overview:
            fail(errors, f"product overview missing current marker: {marker}")
    for marker in (
        "PR-045 — Context asymmetry",
        "selected Meeting voice (Built-in or approved My Voice)",
        "PR-119 — Selected Meeting voice",
        "CC-BY-4.0",
    ):
        if marker not in requirements:
            fail(errors, f"product requirements missing current contract: {marker}")
    if "does **not** store run outcomes" not in acceptance:
        fail(errors, "acceptance scenarios must remain outcome-free policy")
    if "a selected built-in or approved My Voice" not in acceptance:
        fail(errors, "acceptance scenarios must allow built-in day-one Meeting readiness")


def normalize_link_target(source: Path, raw: str) -> Path | None:
    target = raw.strip().strip("<>")
    if not target:
        return None
    lower = target.lower()
    if target.startswith("#") or "://" in target or lower.startswith(("mailto:", "tel:", "data:", "skills:", "sandbox:")):
        return None
    target = unquote(target.split("#", 1)[0].split("?", 1)[0]).strip()
    if not target:
        return None
    return ROOT / target.lstrip("/") if target.startswith("/") else source.parent / target


def check_governance_links(errors: list[str]) -> None:
    for rel in ACTIVE_GOVERNANCE:
        path = ROOT / rel
        if not path.is_file():
            continue
        for raw in LINK_RE.findall(path.read_text(encoding="utf-8")):
            target = normalize_link_target(path, raw)
            if target is not None and not target.resolve().exists():
                fail(errors, f"broken relative governance link in {rel}: {raw}")


def check_workflows(errors: list[str]) -> None:
    root = ROOT / ".github" / "workflows"
    if not root.is_dir():
        fail(errors, "missing .github/workflows")
        return

    temp = sorted(p.name for p in root.glob("temp-*"))
    if temp:
        fail(errors, f"temporary workflows are forbidden: {temp}")

    for path in sorted(root.glob("*.yml")):
        value = path.read_text(encoding="utf-8")
        for action, revision, note in ACTION_RE.findall(value):
            if action.startswith("./"):
                continue
            if not SHA40_RE.fullmatch(revision):
                fail(errors, f"{path.name} uses mutable action ref: {action}@{revision}")
            if not note.strip().startswith("v"):
                fail(errors, f"{path.name} action pin missing version comment: {action}@{revision}")
        if "actions/checkout@" in value and "persist-credentials: false" not in value:
            fail(errors, f"{path.name} checkout must disable persisted credentials")
        if "timeout-minutes:" not in value:
            fail(errors, f"{path.name} must have bounded job timeout")
        if MAIN_BRANCH_LINE_RE.search(value):
            fail(errors, f"{path.name} must not target main under the Local-only model")
        if "on:\n  workflow_dispatch:\n" not in value:
            fail(errors, f"{path.name} must have a manual workflow_dispatch trigger")
        if AUTO_WORKFLOW_EVENT_RE.search(value):
            fail(errors, f"{path.name} must be manual-only (no push/PR/schedule/release trigger)")
        for forbidden in ("contents: write", "pull-requests: write", "git push", "pull_request_target"):
            if forbidden in value:
                fail(errors, f"{path.name} contains forbidden verification behavior: {forbidden}")

    repository = text(".github/workflows/repository-verify.yml")
    if "workflow_dispatch:" not in repository or "python tools/verify_repository.py" not in repository:
        fail(errors, "Repository Verify must use manual dispatch and run tools/verify_repository.py")


def check_ci_efficiency_contract(errors: list[str]) -> None:
    code_health = text(".github/workflows/code-health.yml")
    for marker in (
        "Select complete manual source proof",
        'echo "frontend=true"',
        'echo "python=true"',
        'echo "rust=true"',
        "Exact SHA proof summary",
        "Enforce matching changed-domain proof",
        "A skipped domain is not evidence for that domain.",
        "needs.changes.outputs.frontend == 'true'",
        "needs.changes.outputs.python == 'true'",
        "needs.changes.outputs.rust == 'true'",
        "name: Python source and unit health",
        "name: Rust source and unit health",
        "name: Windows Rust source and unit health",
        "npm run build:frontend",
        "npm run test:frontend-runtime",
        "npm run validate:bridge-contract",
        "npm run validate:tauri-command-args",
        "npm run validate:application-runtime",
        "npm run validate:reachability",
        "npm run validate:runtime-api-usage",
        "npm run validate:test-references",
        "npm run validate:test-ci-routing",
        "npm run preflight:tauri-package",
    ):
        if marker not in code_health:
            fail(errors, f"Code Health lost full manual source-proof contract: {marker}")
    for stale in ("git diff --name-only", "PUSH_BASE", "PR_BASE"):
        if stale in code_health:
            fail(errors, f"Code Health still contains unreachable automatic CI source routing: {stale}")

    release = text(".github/workflows/release-payload-verify.yml")
    for marker in (
        "workflow_dispatch:",
        "Require controlled manual release proof",
        'echo "controlled=true"',
        "name: R3 source contract",
        "name: Controlled Windows payload proof",
        "if: needs.payload-scope.outputs.controlled == 'true'",
        "Exact SHA release proof summary",
        "Enforce release proof completeness",
        "Controlled Windows payload proof required but result was",
    ):
        if marker not in release:
            fail(errors, f"R3 Release Contract lost controlled manual proof: {marker}")
    for stale in ("git diff --name-only", "PUSH_BASE", "github.event_name == 'push'"):
        if stale in release:
            fail(errors, f"R3 Release Contract still contains automatic payload routing: {stale}")

    for workflow in (
        ".github/workflows/repository-verify.yml",
        ".github/workflows/milmmt-repo-contract.yml",
        ".github/workflows/asr-quality-contract.yml",
        ".github/workflows/tts-quality-contract.yml",
        ".github/workflows/quality-readiness-contract.yml",
        ".github/workflows/workerruntime-lock.yml",
    ):
        value = text(workflow)
        for proof_marker in (
            "workflow_dispatch:",
            "concurrency:",
            "cancel-in-progress: true",
            "Write exact-SHA proof summary",
            "GITHUB_SHA",
            "not TARGET_WINDOWS native acceptance",
        ):
            if proof_marker not in value:
                fail(errors, f"{workflow} missing exact-SHA proof marker: {proof_marker}")

    package = text("EngineData/Frontend/RustApp/package.json")
    for marker in (
        "scripts/tests/*.test.ts",
        '"validate:bridge-contract"',
        '"validate:tauri-command-args"',
        '"validate:application-runtime"',
        '"validate:source-size"',
        '"validate:reachability"',
        '"validate:runtime-api-usage"',
        '"validate:test-references"',
        '"validate:source-contracts"',
    ):
        if marker not in package:
            fail(errors, f"frontend package lost canonical source-validation entrypoint: {marker}")


def check_tauri_security_contract(errors: list[str]) -> None:
    main = json.loads(text("EngineData/Frontend/RustApp/src-tauri/capabilities/default.json"))
    overlay = json.loads(text("EngineData/Frontend/RustApp/src-tauri/capabilities/translation-overlay.json"))
    tauri_config = json.loads(text("EngineData/Frontend/RustApp/src-tauri/tauri.conf.json"))
    release_config = json.loads(text("EngineData/Frontend/RustApp/src-tauri/tauri.release.conf.json"))

    if "app" in release_config and isinstance(release_config.get("app"), dict) and "security" in release_config["app"]:
        fail(errors, "release config must not override canonical Tauri security policy")

    csp = str(tauri_config.get("app", {}).get("security", {}).get("csp", ""))
    for required in (
        "default-src 'self'",
        "script-src 'self'",
        "object-src 'none'",
        "frame-src 'none'",
        "base-uri 'none'",
        "form-action 'none'",
    ):
        if required not in csp:
            fail(errors, f"Tauri CSP missing required directive: {required}")
    for forbidden in (
        "'unsafe-eval'",
        "default-src *",
        "script-src *",
        "connect-src *",
        "frame-src *",
        "object-src *",
    ):
        if forbidden in csp:
            fail(errors, f"Tauri CSP contains forbidden broad directive: {forbidden}")

    if main.get("windows") != ["main"]:
        fail(errors, "main Tauri capability must target only the main window")
    if overlay.get("windows") != ["translation-overlay"]:
        fail(errors, "overlay Tauri capability must target only translation-overlay")

    main_permissions = set(main.get("permissions", []))
    overlay_permissions = set(overlay.get("permissions", []))

    for required in (
        "core:default",
        "core:window:allow-destroy",
        "core:window:allow-show",
        "core:window:allow-hide",
    ):
        if required not in main_permissions:
            fail(errors, f"main Tauri capability missing permission: {required}")
    for forbidden in (
        "core:window:allow-set-size",
        "core:window:allow-set-position",
        "core:window:allow-start-dragging",
    ):
        if forbidden in main_permissions:
            fail(errors, f"main Tauri capability is broader than required: {forbidden}")

    for required in (
        "core:default",
        "core:window:allow-set-size",
        "core:window:allow-set-position",
        "core:window:allow-start-dragging",
        "core:window:allow-hide",
    ):
        if required not in overlay_permissions:
            fail(errors, f"overlay Tauri capability missing permission: {required}")
    for forbidden in (
        "core:window:allow-destroy",
        "core:window:allow-show",
    ):
        if forbidden in overlay_permissions:
            fail(errors, f"overlay Tauri capability is broader than required: {forbidden}")


def check_decision_boundary(errors: list[str]) -> None:
    current = text("docs/knowledge/decisions/README.md")
    legacy = text("docs/knowledge/decision-log.md")
    for marker in ("D-001 — Local-only repository authority", "D-035", "CC-BY-4.0"):
        if marker not in current:
            fail(errors, f"current decision register missing marker: {marker}")
    if "historical evidence" not in legacy:
        fail(errors, "decision-log compatibility pointer must mark legacy content historical")


def check_agent_permission_policy(errors: list[str]) -> None:
    try:
        from repository_permissions import evaluate_permission, load_policy
        policy = load_policy()
        corpus = json.loads((ROOT / ".agents/evals/permission-cases.json").read_text(encoding="utf-8"))
        if policy.get("schemaVersion") != 1 or corpus.get("schemaVersion") != 1:
            fail(errors, "agent permission policy/corpus schemaVersion mismatch")
        if set(policy["modes"]) != {"context-recovery", "plan", "bounded-maintenance", "standard-development", "complex-development"}:
            fail(errors, "permission modes differ from canonical AGENTS.md modes")
        registry = json.loads((ROOT / ".agents/skill-registry.json").read_text(encoding="utf-8"))
        expected_scopes = {"governance"} | {item["id"] for item in registry["skills"] if item["kind"] == "specialist"}
        if set(policy["scopes"]) != expected_scopes:
            fail(errors, "permission scopes disagree with registered specialists")
        cases = corpus["cases"]
        if len({case["id"] for case in cases}) != len(cases):
            fail(errors, "duplicate agent permission evaluation case")
        for case in cases:
            actual = evaluate_permission(case["request"], policy)["decision"]
            if actual != case["expected"]:
                fail(errors, f"agent permission case {case['id']}: expected {case['expected']}, received {actual}")
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        fail(errors, f"invalid agent permission policy: {exc}")


def check_agent_skill_evaluations(errors: list[str]) -> None:
    try:
        from repository_agent_evals import validate_agent_evals
        errors.extend(f"skill routing: {issue}" for issue in validate_agent_evals())
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        fail(errors, f"invalid agent routing evaluations: {exc}")


def check_repository_information_architecture(errors: list[str]) -> None:
    try:
        from repository_knowledge import verify_knowledge
        errors.extend(f"knowledge catalog: {issue}" for issue in verify_knowledge())
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        fail(errors, f"invalid repository knowledge architecture: {exc}")
    try:
        from repository_contracts import verify_contracts
        errors.extend(f"interop contracts: {issue}" for issue in verify_contracts())
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        fail(errors, f"invalid cross-language contract inventory: {exc}")
    try:
        from repository_impact import verify_impact
        errors.extend(f"affected-proof planning: {issue}" for issue in verify_impact())
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        fail(errors, f"invalid affected-proof planner: {exc}")


def main() -> int:
    errors: list[str] = []
    check_structure(errors)
    check_repository_information_architecture(errors)
    check_agent_permission_policy(errors)
    check_agent_skill_evaluations(errors)
    check_compactness(errors)
    check_development_foundation(errors)
    check_branch_authority(errors)
    check_continuation_and_product(errors)
    check_governance_links(errors)
    check_workflows(errors)
    check_ci_efficiency_contract(errors)
    check_tauri_security_contract(errors)
    check_decision_boundary(errors)
    if errors:
        print("REPOSITORY VERIFY FAILED")
        for error in errors:
            print(f"- {error}")
        return 1

    print("REPOSITORY VERIFY PASSED")
    print("- repository authority: Local only")
    print("- canonical skills: exact inventory")
    print("- development foundation: capability/proof/toolchain/dev routing contracts")
    print("- governance links: resolved")
    print("- workflows: immutable/read-only/bounded and Local-routed")
    print("- CI: selective domains + exact-SHA proof summaries + canonical source contracts")
    print("- release: controlled payload triggers remain isolated")
    print("- Tauri security: main/overlay capabilities remain least-privilege")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

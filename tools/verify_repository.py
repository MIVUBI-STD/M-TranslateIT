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
    "docs/foundation/01-product-overview.md",
    "docs/foundation/02-product-requirements.md",
    "docs/foundation/03-acceptance-scenarios.md",
    "docs/knowledge/README.md",
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
    ".agents/skills/development-brief/SKILL.md",
    ".agents/skills/desktop-runtime-development/SKILL.md",
    ".agents/skills/desktop-ui-design-development/SKILL.md",
    ".agents/skills/local-ai-runtime-development/SKILL.md",
    ".agents/skills/windows-audio-runtime-development/SKILL.md",
    ".agents/skills/release-packaging-development/SKILL.md",
    "EngineData/Frontend/RustApp/scripts/validate_bridge_contract.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_application_runtime_architecture.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_frontend_reachability.mjs",
    "EngineData/Frontend/RustApp/scripts/validate_source_size_budget.mjs",
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

CANONICAL_SKILLS = {
    "development-brief",
    "desktop-runtime-development",
    "desktop-ui-design-development",
    "local-ai-runtime-development",
    "windows-audio-runtime-development",
    "release-packaging-development",
}

LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
ACTION_RE = re.compile(r"(?m)^\s*uses:\s+([^@\s]+)@([^\s#]+)(?:\s+#\s*(.+))?$")
SHA40_RE = re.compile(r"^[0-9a-f]{40}$")
MAIN_BRANCH_LINE_RE = re.compile(r"(?m)^\s*-\s+main\s*$")


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
    if actual != CANONICAL_SKILLS:
        fail(errors, f"canonical skill inventory mismatch: expected={sorted(CANONICAL_SKILLS)} actual={sorted(actual)}")


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
        for forbidden in ("contents: write", "pull-requests: write", "git push", "pull_request_target"):
            if forbidden in value:
                fail(errors, f"{path.name} contains forbidden verification behavior: {forbidden}")

    repository = text(".github/workflows/repository-verify.yml")
    if "- Local" not in repository or "python tools/verify_repository.py" not in repository:
        fail(errors, "Repository Verify must target Local and run tools/verify_repository.py")


def check_ci_efficiency_contract(errors: list[str]) -> None:
    code_health = text(".github/workflows/code-health.yml")
    for marker in (
        "Detect changed source domains",
        "workflow_dispatch:",
        "Exact SHA proof summary",
        "Enforce matching changed-domain proof",
        "A skipped domain is not evidence for that domain.",
        'EVENT_NAME" == "workflow_dispatch"',
        "fetch-depth: 0",
        "git diff --name-only",
        "needs.changes.outputs.frontend == 'true'",
        "needs.changes.outputs.python == 'true'",
        "needs.changes.outputs.rust == 'true'",
        "npm run build:frontend",
        "npm run validate:bridge-contract",
        "npm run validate:reachability",
        '"EngineData/Frontend/RustApp/src-tauri/tauri.conf.json"',
        '"EngineData/Frontend/RustApp/src-tauri/capabilities/**"',
        "npm run preflight:tauri-package",
    ):
        if marker not in code_health:
            fail(errors, f"Code Health lost selective source-proof contract: {marker}")

    release = text(".github/workflows/release-payload-verify.yml")
    if '"EngineData/Frontend/RustApp/scripts/**"' in release:
        fail(errors, "R3 Release Contract must not rebuild payloads for every frontend script change")
    for marker in (
        '"EngineData/Frontend/RustApp/package.json"',
        '"EngineData/Frontend/RustApp/scripts/build_r3_external_payload.py"',
        '"EngineData/Frontend/RustApp/scripts/validate_release_payload.mjs"',
        '"EngineData/Frontend/RustApp/src-tauri/windows/**"',
        "workflow_dispatch:",
        'EVENT_NAME" == "workflow_dispatch"',
        "controlled=true",
        "Exact SHA release proof summary",
        "Enforce release proof completeness",
        "Controlled Windows payload proof required but result was",
    ):
        if marker not in release:
            fail(errors, f"R3 Release Contract missing release/proof contract: {marker}")

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
        '"validate:application-runtime"',
        '"validate:source-size"',
        '"validate:reachability"',
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


def main() -> int:
    errors: list[str] = []
    check_structure(errors)
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

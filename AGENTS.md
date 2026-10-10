# TranslateIT Agent Routing

Repository state is authoritative. Chat history and old evidence are supporting context only.

## Branch and execution authority

**Local-only development model:** `Local` owns current development, source, verification planning, governance, and continuation. The GitHub default branch `main` is reserved for finalized outcomes only; it must not be modified or used as development/CI fallback before explicit release approval.

- Material GitHub work follows root `GITHUB_RULES.md`.
- GitHub Actions verification is **manual-only**, on demand and not a prerequisite for ordinary source-verifiable REMOTE_GITHUB work. Use the smallest decisive evidence; do not trigger CI for progress ceremony. Default-branch administration is not part of normal development; do not change `main` or the default branch to enable CI. Genuine Windows/device proof remains a separate capability.
- Do not fall back to `main` or another branch for current source, proof, or continuation.
- Do not create alternate development branches as part of the normal method.
- Historical branches/reports are recovery evidence only and are not current task or product authority.

## Capability Gate

Resolve actual capability before choosing execution mechanics. Capability describes what the current session can do; it is not proof by itself.

```text
REPO_READ       = inspect current repository source/history/docs
REPO_WRITE      = mutate the authoritative Local ref
LOCAL_SHELL     = execute commands against an exact local checkout/toolchain
CI_CONTROL      = inspect/dispatch matching CI and exact-run evidence
ARTIFACT_ACCESS = inspect/download exact build or proof artifacts
NATIVE_HOST     = exercise installed TranslateIT on the target Windows machine
```

Capabilities are additive. `REPO_WRITE` does not imply `LOCAL_SHELL`; hosted Windows CI does not imply `NATIVE_HOST`. Use the smallest capability set that can satisfy the claim and finish lower-capability partitions before handing off only the remaining residue.

## Execution Context Gate

Classify by actual capability:

```text
CONTEXT: REMOTE_GITHUB
CONTEXT: LOCAL_CODE
CONTEXT: TARGET_WINDOWS
```

```text
REMOTE_GITHUB  = repository + GitHub CI
LOCAL_CODE     = exact checkout + development toolchain/filesystem
TARGET_WINDOWS = LOCAL_CODE + installed TranslateIT + real Windows GPU/audio/device/meeting environment
```

Proof ceiling follows actual context. Exhaust the `REMOTE_GITHUB`-valid partition before handing off only genuinely higher-context residue.

## Observe / recover context

For `amati`, inspect, audit, understand, or recovery:

```text
AGENTS.md
→ GITHUB_RULES.md Core Rules
→ CONTEXT.md / next-action only if material
→ smallest owner
→ report
→ STOP
```

Read-only means no edit, CI trigger, continuation advance, or execution of the recorded next step.

**Short-prompt and session recovery:** for a repository URL or "amati", inspect the exact `Local` ref and only the relevant current owner. Use commit `Work`, `State`, and `Proof` as revision-scoped historical evidence, then reconcile with current source and `docs/knowledge/next-action.md` only when continuity is material. An old `Next` does not authorize a new mutation. A clear "continue" retains the current bounded work; do not silently switch specialists or reconstruct missing chat details as facts.

## Work mode after context

### Bounded Maintenance

Use for a concrete bug, stale rule, stale test, CI-routing defect, or behavior-preserving cleanup.

```text
Goal
Failure Classification / first wrong owner
Acceptance
Proof Required
STOP Condition
```

### Standard Development

Use when requirement and semantic owner are clear but work exceeds bounded maintenance.

```text
Goal
Success Metric
Forbidden Proxy / Non-Goal
First Evidence Required / first wrong owner
In Scope / Out of Scope
Execution Partition / higher-context residue
Proof Required
STOP Condition
```

### Complex / Ambiguous Development

Use `.agents/skills/development-brief/SKILL.md` for architecture/redesign, unclear or cross-owner requirements, material public contracts, unresolved success criteria, quality/efficiency work, or a change whose safe boundary cannot be expressed by the Standard contract.

### Plan

Use when a high-impact product/architecture/release decision remains unresolved.

```text
recover current Local authority
→ inspect smallest relevant evidence
→ resolve/present the decision
→ NO IMPLEMENTATION
→ STOP
```

No silent transition from Plan to Development.

## Agent execution and bounded context

`AGENTS.md` remains the canonical work-mode and specialist policy. Classify the user's task explicitly as `INSPECT | PLAN | DIAGNOSE | DEVELOP | REPAIR | VALIDATE | RELEASE` and select the existing work mode before consulting any helper. For ambiguous ownership or a genuine cross-language task, `python tools/repository_context.py --mode <mode> --intent "<goal>" [--scope <registered-scope>] [--owner <exact-file>] [--write <proposed-path>]` produces a **read-only** projection of `REQUIRED / CONDITIONAL / EXCLUDED` context, optional permission preflight, and optional impact/retrieval evidence. Never treat this planner as a natural-language router, autonomous approval, executor, new agent, or source of product semantics.

Known bounded owners go directly to their exact source. Invoke context projection only if it can change a material decision; do not load all specialists or use a full-repo scan by default. `INSPECT/PLAN` remain read-only. An explicit `--activate-specialist` may select at most one existing domain skill for development; complex work additionally includes `development-brief`. Cross-scope symptoms require an evidence-bounded handoff and STOP, not automatic lane switching. Stable procedure is in `docs/knowledge/flow.md`.

## Repository knowledge and contract access

Select one document domain through `docs/README.md`, then follow stable document IDs and explicit links. `tools/repository_knowledge.py` derives a read-only Catalog/Graph and scoped section retrieval directly from canonical Markdown; an index match, reference, or historical record is not proof. For interop boundaries use `tools/interop-contracts.json` only to locate source owners and their existing validators. Do not create a second protocol/DTO truth or manually promote test presence to executed success.

For an ambiguous change across Rust, TypeScript, Python or release boundaries, `python tools/repository_impact.py --changed <exact-repository-path> [...]` derives bounded source-import consumers, registered interop contracts, and candidate affected proof. `tools/repository_dependencies.py` constructs the partial graph **on demand** from source; no graph or second ownership store is persisted. This does not execute tests, dispatch CI, prove complete dependency closure, or replace `source-ownership.md`. When an actual `Local` checkout is available, `tools/repository_verification.py` can select existing affected checks; it is plan-only by default and executes only via explicit `--execute --expected-sha <Local-SHA>` with an authorized development mode/scope. It never installs dependencies, invokes CI, publishes releases, or upgrades partial import evidence to full proof.

## Skill registry and permission preflight

- `.agents/skill-registry.json` owns the machine-readable inventory of the six existing project skills; `docs/knowledge/skills/skill-map.md` explains roles without becoming a second inventory authority.
- For material mutation, choose the current work mode and semantic scope, then evaluate `python tools/repository_permissions.py --mode <mode> --action write --scope <scope> --path <repository-relative-path>`.
- `allow` is an advisory preflight, **not** GitHub/Tauri/OS authorization or evidence of a correct semantic owner. `ask` requires explicit approval or a bounded re-scope; `deny` forbids the proposed action. Never use another scope just to bypass the result.
- UserData, private meeting bodies, voice recordings, credentials, and generated runtime artifacts are not routine repository write targets. Root governance and the existing six skills retain their current authority and specialist budget.

## Agent routing evaluation

The **existing** skill inventory remains frozen at six. `.agents/evals/skill-routing.json` owns bilingual positive/negative/ambiguous task-routing expectations, and `.agents/evals/skill-procedure.json` references them for procedure cases. `tools/repository_agent_evals.py` validates both and scores only externally supplied receipts; `tools/repository_skill_admission.py` statically rejects unsafe Skill packages. A valid corpus or synthetic scorer test is not proof that an AI agent routed real tasks correctly. The `.agents/evals/permission-cases.json` corpus continues to own action/path permission examples; do not duplicate it in the routing corpus or create a seventh generic specialist.

## Specialist budget

Canonical project skills are:

```text
development-brief
desktop-runtime-development
desktop-ui-design-development
local-ai-runtime-development
windows-audio-runtime-development
release-packaging-development
```

Budget:

```text
Bounded Maintenance → zero/one specialist when diagnosis needs it
Standard Development → zero/one specialist
Complex Development → development-brief + zero/one specialist
Plan / Context Recovery → none by default
```

Choose by semantic responsibility, not language/framework/library names.

## Semantic routing

```text
desktop shell/navigation/readiness/settings/bridge
→ desktop-runtime-development

visual hierarchy/layout/tokens/component states/rendered UI
→ desktop-ui-design-development

ASR/translation/TTS/model/AI worker/runtime
→ local-ai-runtime-development

physical mic/capture/VAD/Windows devices/Meeting route
→ windows-audio-runtime-development

installer/private Python/runtime assets/models/provider delivery
→ release-packaging-development
```

If investigation reveals a second independent problem, finish/reframe the current boundary instead of stacking specialists.

## Canonical state owners

| Information | Owner |
|---|---|
| GitHub/ref/history/CI/security/transfer/retry/STOP | `GITHUB_RULES.md` |
| Agent mode/context/routing/skill budget | `AGENTS.md` |
| Current product/repository orientation | `CONTEXT.md` |
| Current product/system law | `docs/foundation/` |
| Active continuation + one next step | `docs/knowledge/next-action.md` |
| Current proof interpretation | `docs/knowledge/current-validation.md` |
| Responsibility → current source owner | `docs/knowledge/source-ownership.md` |
| Durable decisions/reasons | `docs/knowledge/decisions/` |
| Operational runbooks | `docs/knowledge/operations/` |
| Skill inventory/routing | `docs/knowledge/skills/` |
| Actual behavior | current `Local` source + matching proof |

Do not create parallel status, plan, TODO, completion, review-state, roadmap, or session-memory systems.

## Source precedence

1. current explicit user instruction for task intent/new decision;
2. current `docs/foundation/` law;
3. current `Local` source + matching proof for actual implementation behavior;
4. target evidence for target-only claims;
5. `next-action.md` for continuation;
6. `source-ownership.md` for navigation;
7. `CONTEXT.md` for current orientation;
8. `docs/knowledge/decisions/` for durable why/history;
9. Git history as bounded recovery evidence.

Conflict handling:

```text
foundation vs source
→ desired behavior vs implementation gap

current user decision vs foundation
→ reconcile foundation first

next-action vs source
→ source wins for implementation state
→ reconcile stale continuation

historical evidence vs current owner
→ current owner wins unless history is explicitly revalidated
```

## Root-cause and minimum-complete gate

Before a material edit establish:

1. what happens now;
2. first wrong owner;
3. expected behavior;
4. why the proposed change addresses that owner;
5. cheapest proof that can falsify the result.

Every persistent file/module/dependency/config/fallback/cache/workflow/state must trace to the goal, acceptance, required contract, proved cause, or required proof.

Do not hide unknown causes with blind retry, arbitrary delay, broad catch/fallback, parallel services, compatibility aliases, duplicate state, or generic frameworks.

## Evidence language

Capability and proof type are separate. Use the cheapest proof capable of falsifying the changed claim.

```text
STATIC_SOURCE       = source/config/docs inspection only
EXECUTED_SOURCE     = source-level build/lint/unit execution
INTEGRATION_FIXTURE = controlled multi-component or protocol fixture
PACKAGE_SMOKE       = packaged artifact/install boundary exercised
LIVE_RUNTIME        = exact live application/runtime path exercised
NATIVE_ACCEPTANCE   = target Windows GPU/audio/device/meeting behavior exercised
UNKNOWN             = evidence cannot yet separate the owner; name the next separating evidence
UNSUPPORTED         = required capability is unavailable
```

Build success is not runtime success. Hosted Windows is not automatically native acceptance. Historical proof is not current proof unless the claim and source identity still match. Canonical diagnosis and proof rules live in `docs/knowledge/development-discipline.md`.

## User-facing reporting

For material work:

```text
Status: Selesai | Perlu pemeriksaan | Terhenti
Hasil:
Bukti:
Batasan:
Next step:
```

Use exactly one `Next step` when work remains. Update only canonical owners whose actual state changed, then STOP.

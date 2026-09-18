# TranslateIT Agent Routing

Repository state is authoritative. Chat history and old evidence are supporting context only.

## Branch and execution authority

**Local-only repository model:** `Local` is the sole active repository authority for development, governance, CI, proof, continuation, and release-source validation.

- Material GitHub work follows root `GITHUB_RULES.md`.
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

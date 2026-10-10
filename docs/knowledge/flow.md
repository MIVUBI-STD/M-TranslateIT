---
id: document.knowledge.flow
class: DOCUMENT
domain: knowledge
role: WORKFLOW
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Work Flow

## Canonical flow

```text
User request
→ PIN repository + Local HEAD
→ classify EXECUTION CONTEXT
→ choose Context Recovery | Plan | Maintenance | Standard | Complex
→ read minimum current authority
→ identify first wrong owner
→ define acceptance + proof ceiling
→ finish current-context partition
→ TOOL + TRANSFER GATE
→ minimum complete change
→ cheapest falsifiable proof
→ one logical Local delivery
→ update only changed canonical state owners
→ exactly one next step when work remains
→ STOP
```

## Local-only repository model

`Local` is the sole active **development** authority: source, governance, verification planning, continuation and release-source validation remain on `Local`. `main` stays the GitHub default/final-result branch; it is not a source, fallback, or write target during unfinished work. Any final promotion requires explicit user approval and suitable acceptance evidence.

## Execution contexts

```text
REMOTE_GITHUB
→ GitHub source/version control and static review only; no hosted compute

LOCAL_CODE
→ exact Local checkout + development toolchain/filesystem

TARGET_WINDOWS
→ installed TranslateIT + real GPU/audio/device/meeting environment
```

Never transfer an entire task because one check requires execution. Complete GitHub source/contract review, record missing build/model/device evidence as `UNKNOWN / NOT EXECUTED`, and preserve the **no user-PC handoff** for incremental changes. No cloud or hosted runner. The single grouped local build/package/native stage is owned by `GITHUB_RULES.md` and `docs/knowledge/operations/target-windows-performance.md`.

## Modes

### Context Recovery

Read-only. Current authority only. Never execute a historical next step merely because it exists.

### Plan

Resolve a material product/architecture/release choice. No implementation.

### Bounded Maintenance

```text
Goal
Failure Classification / first wrong owner
Acceptance
Proof Required
STOP Condition
```

### Standard Development

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

Use `.agents/skills/development-brief/SKILL.md`, then at most one semantic specialist.

## Task class, context projection, and handoff

Work mode remains owned by `AGENTS.md`. Classify a task, then use the minimum canonical owner:

```text
context-recovery → INSPECT | DIAGNOSE (read-only)
plan             → PLAN (read-only)
bounded          → REPAIR | DEVELOP | VALIDATE
standard         → DEVELOP | REPAIR | VALIDATE | RELEASE
complex          → DEVELOP | REPAIR | VALIDATE | RELEASE; development-brief required
```

`RELEASE` is an explicit request requiring separate authorization; context selection never publishes, dispatches CI, changes branches, or approves a release. Scope is the existing governance or registered domain-specialist boundary, not a new task database.

Use `python tools/repository_context.py --mode <mode> --intent "<goal>" --scope <scope> --owner <exact-file>` **only** when owner, permission, context budget, or cross-language impact is materially uncertain. The projection composes `repository_permissions.py`, `repository_knowledge.py`, and `repository_impact.py`; it never duplicates their authority or infers mode/specialist from keywords. Add `--write <path>` for advisory write preflight, `--changed <path>` for optional conservative affected evidence, `--resume` only for actual continuity, or `--knowledge-query "<terms>" --knowledge-domain foundation|knowledge|system` for explicitly scoped *ranking*, not truth. The output is not executable proof.

```text
REQUIRED     current root GitHub/agent rules + the exact selected owner
             development discipline when developing
             development-brief only for complex mode
             the one specialist only when explicitly selected and useful
CONDITIONAL  CONTEXT.md, continuation, selected product law, other specific
             owner/validator only when it changes the next decision
EXCLUDED     all other specialists, sibling docs domains, broad source/history
             and target Windows acceptance unless intrinsically relevant
```

For a cross-domain defect, do not silently change the active scope. A `DEVELOPMENT_HANDOFF` must name `source_scope`, distinct registered `target_scope`, `observed`, `expected`, `minimum_evidence`, and `resume_stage`; the resulting packet is **PROPOSED_NOT_ACTIVATED**. Finish or STOP the originating bounded task before a new authorized scope begins. Historical test/commit evidence cannot replace current source truth.

## Routing evidence

Six canonical skills remain unchanged. Golden routing cases validate expected modes and specialist ownership; only actual observed agent-run receipts may support a model-routing accuracy claim. A semantic specialist never overrides the active lane's permission or starts an unrelated release.

## Skill procedure and supply-chain evidence

`tools/repository_skill_admission.py --check` treats committed `.agents/skills/` assets as untrusted **data**: registered package names, metadata, nested references, file types, symlinks, size, and direct command/instruction overrides are checked without executing their contents. Admission does not certify every possible prompt injection or authorize imported execution.

`.agents/evals/skill-routing.json` owns route expectations; `.agents/evals/skill-procedure.json` adds positive, collision, pressure and handoff procedure scenarios linked to those routes. `python tools/repository_agent_evals.py --check` validates these source fixtures. `--score` and `--score-procedures` accept **external agent-run receipts**, never invoke a provider. Their scores check receipt consistency only; a synthetic test passing does not prove a model followed the instruction, that a handoff was authorized, or that target Windows execution succeeded.

## Derived dependency intelligence

`python tools/repository_impact.py --changed <repository-relative-file> [...]` can identify known import consumers and cross-language contract regressions before source modification or targeted validation. `tools/repository_dependencies.py` parses current TypeScript/Svelte relative imports, Rust module/use relationships, and local Python AST imports into an **ephemeral** reverse graph; no second knowledge/owner registry or generated graph file exists.

Only explicit source edges are derived. Dynamic imports, macro expansions, service IPC protocols, type/value semantics, and runtime behavior are outside this partial graph. Registered Rust↔TypeScript↔Python interop boundaries retain their canonical `tools/interop-contracts.json` owners and verifiers. Unknown imports, unindexed sources or uncertain closure require review or broader proof, never silent test omission. Each run is `PLANNING_ONLY_NOT_EXECUTED`; it cannot start tests, CI, release, or update `main`.

## Opt-in affected verification

`python tools/repository_verification.py --mode standard-development --scope desktop-runtime-development --changed EngineData/Frontend/RustApp/src/app/bridge/applicationRuntimeApi.ts` prints the recommended checks **without executing anything**. The planner uses the existing `repository_impact.py`, `repository_permissions.py`, `package.json` and canonical test files, not a new workflow or test registry.

To execute the selected source checks on a real, clean `Local` checkout, the operator must explicitly add `--execute --expected-sha <exact-40-character-Local-HEAD>`. The tool verifies branch, SHA and worktree both before and after checking, passes fixed command arguments without a shell, does not install dependencies, and stops on the first failure/unavailable tool. Missing Node, pytest, Cargo or local source dependencies remain proof residues. Tests are repository-owned programs and are **not sandboxed** by this runner; review and authorize execution in the current host context first.

The default Rust check is an existing cheap manifest/contract validator, **not** `cargo check`. `--include-offline-rust` additionally selects `cargo check --locked --offline` only by explicit choice. Dependency-graph closure is partial even when checks succeed. A `SELECTED_CHECKS_PASS_PARTIAL_PROOF` outcome is **not** full acceptance; the CLI uses a nonzero exit when review is still required. No CI or release workflow is dispatched by this tool.

## First-wrong-owner examples

```text
product rule wrong
→ docs/foundation

source violates correct rule
→ source owner

source correct, test stale
→ test owner

source/test correct, workflow wrong
→ CI owner

claim needs real mic/GPU/installer evidence
→ TARGET_WINDOWS proof owner
```

## Proof rule

Routine source work uses exact-head source/contract evidence first, without automatic CI. Manual `workflow_dispatch` is reserved for stronger executable/integration/release evidence when genuinely available. The default branch remains `main`; Local-only workflow definitions cannot be assumed dispatchable until GitHub's default-branch registration requirement is satisfied through a separately authorized final/release process. Do not move unfinished source to `main` to create proof.


One scenario proves one claim. Source/static proof is never upgraded to target-Windows proof. Run only the proof that can falsify the changed claim. Broader source confidence means the relevant checks must succeed on the same exact `Local` SHA.

## Session recovery and continuation

For a repository-only "amati" request, pin `Local`, recover only relevant source owners and topic-specific commit trailers, compare them with newer changes, report the current proof ceiling, and STOP without edits. For authorized continuation, select the same bounded work from current source and the canonical `planning/development.md`; historical commit `Next` never upgrades to current instruction. Commit trailers preserve revision-scoped rationale, not a second task-state system.

## State routing

```text
what is active?       → planning/development.md
what is proven?       → current-validation.md
who owns it?          → source-ownership.md
what must it do?      → docs/foundation/
why was it chosen?    → decisions/
what does it do now?  → current Local source + matching proof
```

## Application runtime ownership

The canonical cross-feature lifecycle, snapshot, controller, facade and state-projection architecture is in [Application Runtime](../system/application-runtime.md). This document owns development modes, GitHub execution, evidence and specialist handoffs only.

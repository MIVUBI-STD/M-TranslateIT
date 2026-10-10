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
→ source/static/CI-verifiable work

LOCAL_CODE
→ exact Local checkout + development toolchain/filesystem

TARGET_WINDOWS
→ installed TranslateIT + real GPU/audio/device/meeting environment
```

Never transfer an entire task because one residue requires a higher context. Finish independent GitHub-valid source/test/harness/provenance work first and hand off only the intrinsic residue.

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

Use `python tools/repository_context.py --mode <mode> --intent "<goal>" --scope <scope> --owner <exact-file>` **only** when owner, permission, context budget, or cross-language impact is materially uncertain. The projection composes `repository_permissions.py`, `repository_knowledge.py`, and `repository_impact.py`; it never duplicates their authority or infers mode/specialist from keywords. Add `--write <path>` for advisory write preflight, `--changed <path>` for optional conservative affected evidence, `--resume` only for actual continuity, or `--knowledge-query "<terms>" --knowledge-domain foundation|knowledge` for explicitly scoped *ranking*, not truth. The output is not executable proof.

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

For a repository-only "amati" request, pin `Local`, recover only relevant source owners and topic-specific commit trailers, compare them with newer changes, report the current proof ceiling, and STOP without edits. For authorized continuation, select the same bounded work from current source and the canonical `next-action.md`; historical commit `Next` never upgrades to current instruction. Commit trailers preserve revision-scoped rationale, not a second task-state system.

## State routing

```text
what is active?       → next-action.md
what is proven?       → current-validation.md
who owns it?          → source-ownership.md
what must it do?      → docs/foundation/
why was it chosen?    → decisions/
what does it do now?  → current Local source + matching proof
```


## Product runtime authority

Interactive product flows now converge through one application-level authority instead of letting individual UI pages own cross-feature lifecycle decisions.

```text
Svelte UI
→ Product intent
→ ApplicationRuntime
→ subsystem command / resource owner
→ canonical ApplicationSnapshot
→ runtime event
→ UI reconciliation
```

Current migrated vertical slices:

- Meeting start / stop
- Mic Test start / stop
- Setup recovery / helper readiness
- Shared audio device selection
- My Voice recording start / stop
- My Voice build start / cancel / approve
- Built-in Meeting voice selection

Rules:

1. UI may present convenience guards, but the backend runtime is authoritative for resource ownership and lifecycle acceptance.
2. Cross-feature actions use `dispatch_product_intent`; feature-local read APIs may remain direct until their migration is justified.
3. `ApplicationSnapshot` is the canonical cross-feature reconciliation payload. Feature-specific snapshots remain valid internal detail, not competing product truth.
4. Runtime events signal state changes. Polling remains a reconciliation/watchdog mechanism, not the preferred owner of lifecycle transitions.
5. Superseded direct mutation commands are removed from the public Tauri surface after migration. Feature-local read APIs remain direct only when they are still useful and do not create a competing lifecycle authority.
6. New cross-feature conflicts must be solved in `ApplicationRuntime` or the underlying resource owner, never by adding another page-specific boolean maze.


### Intentional frontend-owned module: translation overlay

The floating translation overlay remains frontend-owned. It manages window presentation, local preferences, caption rendering, and position state; these are UI concerns rather than shared resource ownership. Do not move overlay window implementation into ApplicationRuntime. Only promote an overlay concern into the application kernel if it becomes a real cross-feature lifecycle/resource invariant.


### Snapshot cost rule

`ApplicationSnapshot.revision` is a monotonic snapshot-build sequence, not a semantic state-change revision. Use owner/session/generation/domain fields to detect meaningful state transitions.

`ApplicationSnapshot` is a cheap reconciliation contract. Building it may inspect local in-process/native status, but it must not run expensive worker requests, model inference, functional probes, or recovery actions. Cheap capabilities therefore cover only what current local state can prove; detailed Text translation readiness remains owned by the product readiness layer after explicit worker-capability inspection. Deep AI readiness belongs to the product/detail query that explicitly needs it. This keeps close checks, settings mutations, and runtime events bounded and prevents orchestration from creating hidden latency.


### Event-first Meeting reconciliation

Live Meeting UI freshness uses lightweight backend change events from authoritative committed-turn state. The event payload contains only revision/reason/session/sequence metadata; the frontend then reads authoritative Meeting state. A 10-second Meeting poll remains only as reconciliation fallback if an event is missed.

Reliability monitoring remains polling-based where the condition itself is time-dependent (watchdog, device-loss detection, long-session pressure). Watchdog + device-loss share one live reliability IPC every 8 seconds; long-session health remains on its separate 30-second cadence. UI-only advisory checks such as meeting-app detection and idle audio-quality status are on-demand/page-entry reads rather than permanent polling.


### Frontend application controller

The frontend shell consumes one `ProductRuntimeSnapshot` through `src/app/runtime/applicationController.ts`. The controller owns refresh sequencing, setup-state transitions, settings projection, and Meeting-session projection. `App.svelte` owns only UI-shell concerns such as route, transient notices, busy flags, close-dialog state, and the live transcript view.

Do not reintroduce parallel `$state` copies for helper, worker, input, settings, approved voice readiness, or Meeting runtime state in `App.svelte`. Derived views must come from the controller snapshot.


### Shell controller split

`App.svelte` is intentionally a composition shell. Runtime-heavy view coordination is split into focused frontend controllers:

- `applicationController.ts` — product snapshot, setup transition, refresh sequencing.
- `meetingLiveController.ts` — transcript reconciliation, overlay revision, Meeting live-view freshness.
- `closeController.ts` — native-close dialog/check/stop flow.

These controllers may coordinate existing public runtime/query APIs, but they must not absorb domain algorithms. The shell keeps route, notices, and transient button busy state only.

### Product facade split

`runtimeProductFacade.ts` is a compatibility/public composition surface, not a domain implementation owner. Product audio, text translation, and setup/recovery logic live in `productAudioFacade.ts`, `productTranslationFacade.ts`, and `productSetupFacade.ts` respectively. New unrelated product behavior must not be appended to the root facade by default.


### Product-state policy split

Frontend product state policy is split by concern:

- `productMeetingState.ts` — Meeting lifecycle/status/preflight mapping.
- `productReadinessState.ts` — cross-capability readiness, blockers, voice gate, and product status copy.
- `workerCapabilities.ts` — parsing worker capability evidence only.
- `runtimeProductState.ts` — compatibility re-export surface only.

Do not merge these implementations back into one large product-state file. Compatibility surfaces may re-export stable APIs, but policy ownership stays domain-scoped.

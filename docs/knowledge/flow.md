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

`Local` is the sole active repository authority. Current source, governance, CI, continuation, and release-source validation remain on `Local`. Other branches are not part of the normal work flow.

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

One scenario proves one claim. Source/static proof is never upgraded to target-Windows proof. Run only the proof that can falsify the changed claim. Broader source confidence means the relevant checks must succeed on the same exact `Local` SHA.

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

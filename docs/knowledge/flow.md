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
5. Existing commands remain temporarily available as compatibility paths while vertical slices migrate. Do not duplicate new orchestration rules in both the UI and feature commands.
6. New cross-feature conflicts must be solved in `ApplicationRuntime` or the underlying resource owner, never by adding another page-specific boolean maze.


### Intentional frontend-owned module: translation overlay

The floating translation overlay remains frontend-owned. It manages window presentation, local preferences, caption rendering, and position state; these are UI concerns rather than shared resource ownership. Do not move overlay window implementation into ApplicationRuntime. Only promote an overlay concern into the application kernel if it becomes a real cross-feature lifecycle/resource invariant.

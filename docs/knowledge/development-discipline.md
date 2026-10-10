---
id: document.knowledge.development-discipline
class: DOCUMENT
domain: knowledge
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Development Discipline

This file owns the minimum development flow, failure classification, ownership economy, proof vocabulary, and STOP discipline. Product behavior remains in `docs/foundation/`; source ownership remains in `source-ownership.md`.

## Minimum flow

```text
requested outcome
→ current evidence
→ failure classification
→ first wrong owner
→ smallest complete change
→ matching proof
→ STOP
```

Before adding code, ask in order:

```text
No change required?
→ can an unnecessary path be deleted?
→ can the current owner/path be reused?
→ can a native/platform/current dependency solve it?
→ smallest complete addition
→ new abstraction/system only after repeated responsibility is proved
```

Do not create duplicate managers, caches, routers, provider registries, state stores, fallback engines, compatibility layers, or generic frameworks merely to make the code look more extensible.

## Failure classification

Use the first material wrong boundary:

- `SEMANTIC_CONTRACT` — product/domain meaning is wrong or incomplete.
- `IMPLEMENTATION` — semantics are correct but source behavior is wrong.
- `PRESENTATION` — canonical result is correct but UI/state presentation is wrong.
- `ROUTING` — CI/tool/developer routing reaches the wrong owner.
- `ENVIRONMENT` — required toolchain/runtime capability is unavailable or mismatched.
- `PROOF` — requested evidence has not actually exercised the claim.
- `STALE_TEST` — implementation is correct and the assertion is obsolete.
- `UNKNOWN` — current evidence cannot separate owners; name one next separating observation.

Do not convert `UNKNOWN` into retries, delays, broader catches, fallback engines, or parallel ownership.

## Ownership economy

- One responsibility has one canonical owner.
- One persisted fact has one authority; other layers derive or present it.
- One user action has one primary execution path.
- Commands/UI/bridges are adapters and must not silently become duplicate business authority.
- Cross-owner work is sequential: Owner A decides/proves its boundary, emits the minimum typed result, then Owner B consumes it without recomputing Owner A truth.
- Extract abstractions only after a real repeated responsibility exists.

For TranslateIT, semantic responsibility is selected by domain: desktop runtime, desktop UI, local AI, Windows audio, or release packaging. Programming language alone never selects ownership.

## Proof types

```text
STATIC_SOURCE
EXECUTED_SOURCE
INTEGRATION_FIXTURE
PACKAGE_SMOKE
LIVE_RUNTIME
NATIVE_ACCEPTANCE
```

Examples:

- repository contract inspection → `STATIC_SOURCE`
- npm build / cargo test / pytest → `EXECUTED_SOURCE`
- controlled worker↔Rust protocol fixture → `INTEGRATION_FIXTURE`
- generated installer starts with expected payload boundary → `PACKAGE_SMOKE`
- exact changed desktop/worker path exercised → `LIVE_RUNTIME`
- physical microphone, GPU, VB-CABLE, meeting-app reception, or measured target latency → `NATIVE_ACCEPTANCE`

Execution location does not automatically upgrade proof type. Hosted Windows remains hosted execution unless the exact native acceptance condition was exercised.

## Capability / context

Capabilities are defined in root `AGENTS.md`. Existing execution contexts remain useful orientation:

```text
REMOTE_GITHUB
LOCAL_CODE
TARGET_WINDOWS
```

Context is not permission and is not proof. Use actual capability plus observed evidence.

## Development operating standard

Before material work, translate the request into an observable goal, constraints, non-goals, known consumer and canonical owner, cheapest falsifying evidence, and STOP condition. Choose exactly one current work mode; do not promote speculative evidence to a higher proof tier. Prefer: no change → delete an unnecessary path → correct the current owner → reuse an existing capability → minimal complete addition.

**Development preflight:** identify the first wrong owner, direct caller/consumer, proposed change scope, actual permission/ability to execute, and the narrow evidence capable of rejecting the result. A missing compiler or native Windows environment is an explicit proof residue, not permission to edit tests or ask the user for a local checkout by default.

**Completion review:** verify no unauthorized scope expansion, new alias/duplicate authority, missing downstream consumer, stale documentation reference, unconsumed fixture/artifact, or proof overclaim. Record current evidence and the exact unverified residue. STOP on the requested bounded outcome instead of continuing adjacent refactors.

## Change discipline

For non-trivial mutation:

1. establish actual behavior;
2. identify first wrong owner;
3. state expected behavior;
4. choose the smallest complete change;
5. choose the cheapest falsifying proof;
6. update only canonical owners whose state changed;
7. stop after acceptance.

Prefer targeted checks during iteration. Do not run broad verification repeatedly for reassurance. When changed-source dependencies cross Rust/TypeScript/Python owners, the optional `tools/repository_impact.py --changed <path>` provides conservative candidate tests and full-manual-CI choices; `CONSERVATIVE` or unknown edges require broader proof or explicit review, never an unjustified skip.

## Performance work

Performance changes must preserve accepted behavior unless the requirement explicitly changes it.

```text
measure
→ identify dominant owner
→ change one owner
→ compare matching evidence
→ retain only proven improvement
```

Do not reduce translation quality, voice quality, model contract, ordering, bounded queues, failure safety, or readiness merely to improve a proxy metric.

## STOP

Completion is terminal. Do not automatically redesign adjacent layers, upgrade dependencies, add future-proofing, create a second state system, or continue because more tooling exists.

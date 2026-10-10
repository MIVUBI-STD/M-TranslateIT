---
name: development-brief
description: Front door for complex or ambiguous TranslateIT development. Resolve goal, success metric, forbidden proxy, first evidence, semantic owner, execution partition, scope, proof and STOP condition before implementation; then use at most one project specialist.
---

# TranslateIT Development Brief

Use for **Complex / Ambiguous Development** as defined by root `AGENTS.md`. Bounded Maintenance and Standard Development use their compact root contracts unless diagnosis shows the boundary is actually complex.

Root `GITHUB_RULES.md` owns GitHub-first partitioning, atomic delivery, transfer, CI, retries and STOP.

## Continuity boot

```text
AGENTS.md
→ GITHUB_RULES.md Core Rules
→ CONTEXT.md / planning/development.md only if material
→ this brief
→ smallest current owner/evidence
```

If continuation conflicts with source, source wins for actual implementation state; reconcile continuation before proceeding.

## Required contract

```text
Goal
Success Metric
Forbidden Proxy / Non-Goal
Suggested method (if any)
Actual requirement
Execution Context
First Evidence Required / first wrong owner
Input authority
Expected output
Build POV
Acceptance POV
In Scope / Out of Scope
Execution Partition / higher-context residue
Acceptance criteria (2–5)
Proof Required
STOP Condition
Open high-impact decisions
```

This is an internal execution contract, not a new tracked plan file.

## Procedure

1. Separate requested outcome from suggested method.
2. Recover discoverable current facts before asking the user.
3. If a high-impact product/privacy/release/architecture choice remains unresolved, return to Plan instead of inventing it.
4. Identify the semantic owner and direct caller/contract.
5. Define 2–5 falsifiable acceptance criteria.
6. Limit `REMOTE_GITHUB` to source and static review. Cloud/hosted execution is prohibited; group local build and `TARGET_WINDOWS` evidence after source integration.
7. Select zero or one project specialist.
8. Implement the minimum complete owner change.
9. Run the cheapest proof that can falsify each changed claim.
10. Return to the same contract and STOP when satisfied.

## Quality guard

Before implementation require:

- **one owner** — no duplicate service/runtime/store/controller/policy authority;
- **real success** — no placeholder, marker, mock, stale manifest or UI presence promoted beyond what it proves;
- **forbidden proxy explicit** — e.g. a button existing is not proof the Meeting voice works;
- **known fallback only** — broad fallback/retry never hides unknown failure;
- **every addition earns its place** — files/dependencies/workflows/state map to acceptance or proof;
- **proof matches claim** — source/build/CI is not target-Windows/device/model-quality proof unless that context ran.

## Specialist routes

```text
desktop product/runtime shell → desktop-runtime-development
visual/UI craft              → desktop-ui-design-development
AI worker/inference          → local-ai-runtime-development
Windows audio/device route   → windows-audio-runtime-development
release/install/runtime data → release-packaging-development
```

Framework/library helpers are technical tools, not additional TranslateIT specialists.

## Completion

Root `GITHUB_RULES.md` limits GitHub to **source/version control only**: no cloud compute, hosted CI, remote build, or cloud inference. Keep build/model/device proof `UNKNOWN / NOT EXECUTED` until the grouped local build-and-native acceptance phase; never request per-commit tests on the user's PC.

If implementation is complete but required higher-context proof is unavailable, report that residue precisely instead of claiming completion above the proof ceiling. Update only changed canonical state owners and use exactly one next step when work remains.

---
id: document.system.zero-waste-execution
class: DOCUMENT
domain: system
role: WORKFLOW
authority: CANONICAL
lifecycle: ACTIVE
---

# Zero-Waste Execution

## Existing source-derived execution flow

```text
user goal and explicit work mode
→ exact Local SHA and semantic owner
→ minimum relevant owner/consumer reads
→ requirement and first wrong owner
→ optional repository_context / repository_knowledge projection
→ optional repository_impact partial dependency graph
→ minimum complete source change
→ optional repository_verification selected checks
→ exact-revision evidence and remaining UNKNOWN
→ one logical commit and STOP
```

This is a composition of **existing** owners, not a second repository orchestrator, persistent task graph, proof cache or autonomous routing agent. Root [GitHub rules](../../GITHUB_RULES.md) govern tool limits, fast-forward delivery, retry and no-user-PC work; [development discipline](../knowledge/development-discipline.md) governs acceptance and minimum changes.

## Current capability and gaps

| Concern | Existing source owner | Boundary |
|---|---|---|
| Agent context/mode projection | `tools/repository_context.py` | explicit user-selected mode/scope, not keyword inference |
| Stable docs, derived catalog and links | `tools/repository_knowledge.py` | documents are meaning; index/score is navigation |
| Rust/TS/Python static import consumers | `tools/repository_dependencies.py` | incomplete graph, not compiler/runtime truth |
| Changed-path test impact | `tools/repository_impact.py` | conservative candidates, never skip unknown closure |
| Opt-in selected check execution | `tools/repository_verification.py` | requires an actual authorized clean checkout, exact SHA and tools |
| Work recovery | Git commit trailers + `planning/development.md` | no duplicated live status/cache |

Do not invent a general-purpose cache, automatic CI, hosted cloud execution, source manager or seventh skill. GitHub is source/version control only; no incremental user-PC tests. Build and native acceptance are grouped once a source-integrated candidate is ready.

## Batch 5 measurement and conformance

A benchmark must count **correctly scoped outcomes and truthful evidence** rather than declare latency, context, token or model savings without observations. For a fixed, documented set of positive/negative scenarios, review:

1. **Routing correctness:** selected mode, owner, specialist budget, denial and handoff match existing agent evaluations.
2. **Context economy:** required/conditional/excluded document and source counts; unrelated specialists and broad scans remain zero unless specifically needed.
3. **Affected-proof safety:** changed paths, evidence candidates and UNKNOWN closure; no omitted relevant checks inferred from partial imports.
4. **Execution honesty:** planning cannot invoke CI/tests; unexecuted source/native claims stay NOT EXECUTED.
5. **Maintenance overhead:** persistent authority/file/copy count; new caches/registries/aliases must remain zero without a measured need.

Positive cases alone cannot demonstrate conformance. Compare counterexamples (ambiguous owner, missing dependency, invalid permission, native-only claim). An executable source test, model-run trace, performance measurement and target Windows acceptance are separate proof levels. **Implementation:** `tools/repository_benchmark.py` replays *declared* routing cases through the explicit context planner, compares the existing permission corpus, checks document routing and conservative impact, and reports context-file counts and source-proof ceilings. `python tools/repository_benchmark.py --check --summary` is an optional source-level command; `--extended` explicitly opts into one source dependency graph. It never invokes a model, CI, real audio, installer, or Windows runtime and never saves a second benchmark database. Successful source replay is not observed agent accuracy or measured speedup.

## STOP and escalation

After the requested source outcome and proportionate evidence, STOP. Only request more context/proof when it changes a material decision. An unverified claim is an explicit bounded residue, not a task for the user's PC or reason to add unrelated framework code.

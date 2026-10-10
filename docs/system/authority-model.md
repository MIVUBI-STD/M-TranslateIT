---
id: document.system.authority-model
class: DOCUMENT
domain: system
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# Repository Authority Model

## One-way ownership

```text
current user decision + product foundation → desired behavior
current Local source + matching executed evidence → actual implementation
canonical docs/skill/procedure → reusable policy and workflow
planning/development.md → active work intent only
Git commits → historical revision-scoped decisions, not current state
Catalog → Graph → Retrieval → Context → navigation only
```

Authority is **scoped by concern**, not by recency or document length. Do not use README, a derived graph, CI workflow presence, chat memory or old commit trailers to override current source and matching proof. No second state registry, manual "PASS" snapshot, parallel roadmap or session transcript database.

## Resource classes and direction

- `CANONICAL` documents own one durable requirement, architecture rule or procedure.
- `REFERENCE` and `HISTORICAL` sources provide context with their applicability visible; they cannot prove current runtime behavior.
- `DERIVED` catalog, dependency graph, affected-test plan and search results derive from existing owners; they cannot authorize writes, skip unknown checks, or become another truth store.
- Source code owns implementation. Matching tests/CI/logs are evidence **only** for the exact behavior, revision and execution environment they exercised.
- `planning/` owns the next step; not `docs/`. Runtime `UserData/` is private user state, not a repository workspace.

## Update, move, retire

```text
new evidence / decision
→ identify first wrong canonical owner
→ correct or reuse existing owner
→ update affected consumers and links
→ verify exact current source state
→ STOP
```

For document moves retain a stable `id` when its semantic identity is unchanged; update its domain metadata, router, consumer paths and verifier as one bounded migration. Do not keep a duplicate live copy as a redirect. Remove retired owner references; Git history preserves older source. Do not edit generated projections to conceal an upstream contract defect.

Use [naming](./canonical-naming.md) for path decisions, [source ownership](../knowledge/source-ownership.md) for implementation responsibilities, [GITHUB_RULES](../../GITHUB_RULES.md) for GitHub mutation and proof ceilings, and [product foundation](../foundation/README.md) for product behavior.

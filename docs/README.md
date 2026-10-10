---
id: document.docs.router
class: DOCUMENT
domain: docs
role: ROUTER
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Documentation

**Start here for durable documentation.** Select the smallest semantic domain and current owner, not a broad file scan. A stable frontmatter `id` is the document identity; path is its location.

## Domain routing

| Responsibility | Canonical domain |
|---|---|
| Current product behavior, requirements, acceptance and latency law | [Foundation](./foundation/README.md) |
| System structure, naming, authority, ApplicationRuntime and zero-waste execution | [System](./system/README.md) |
| Development flow/discipline, implementation ownership, proof, decisions, runbooks and skills | [Knowledge](./knowledge/README.md) |

## Current work is not documentation

[Planning](../planning/README.md) owns current/future development intent and exactly one next action. Git commit `Work/State/Proof` trailers hold revision-scoped historical recovery evidence. `UserData/` is private runtime data, not a tracked `workspace/`. Do not create empty workspace/experiments folders or duplicate progress/state ledgers.

## Document access and authority

```text
specific question → one domain router → stable ID/link
→ selected canonical owner and H2/H3 section
→ relevant source, skill or proof only when needed → STOP
```

`tools/repository_knowledge.py` derives a read-only Catalog, Graph and section retrieval from these source Markdown files. It never stores a second truth. `--summary` enumerates resources; `--id <document-id>` locates one; `--query "terms" --domain foundation|knowledge|system` ranks bounded sources; `--all` is explicit cross-domain diagnosis.

Every `docs/**/*.md` carries `id`, `class`, `domain`, `role`, `authority`, `lifecycle`. `ROUTER` links, `CONTRACT` owns one invariant, `ARCHITECTURE` explains a system boundary and `GUIDE` navigates. `CANONICAL` is current only for its own concern; `REFERENCE`, `HISTORICAL` and `DERIVED` cannot override source or matching proof. Link instead of copying an owner.

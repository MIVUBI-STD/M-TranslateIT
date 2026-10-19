---
id: document.system.canonical-naming
class: DOCUMENT
domain: system
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# Canonical Naming

## One responsibility, one canonical name

Source implementations have one current owner/path. Git history records revisions; do not create `-new`, `-final`, `-latest`, `-v2`, `backup`, or mirrored source files to preserve earlier work. Preserve explicit version suffixes for **actual external protocol/schema, model, artifact, release or package contracts**, not internal iterations. A public compatibility alias needs a known consumer, lifecycle and retirement criterion.

A rename is a migration: identify its semantic owner and external/internal consumers; update imports, tests, validators, documentation links, graph bindings and package entrypoints together; review affected proof. Never rename purely to emulate another project's folder names.

## Repository vocabulary

| Path | Single responsibility |
|---|---|
| `EngineData/Frontend/RustApp/` | Svelte/Tauri product UI and Rust application/runtime |
| `EngineData/Backend/LocalWorker/WorkerRuntime/` | canonical Python local AI worker |
| `EngineData/Backend/RuntimeAssets/` | controlled runtime asset descriptions and tracked references |
| `UserData/` | ignored user/runtime data; repository holds documentation structure only |
| `docs/foundation/` | product requirements and acceptance law |
| `docs/knowledge/` | established development procedures, proof/owners, decisions, skills, operations |
| `docs/system/` | distinct durable repository/application system architecture |
| `planning/` | active/future development intent, not proof or implementation |
| `tools/` | repository/quality Python validators and existing contract tooling |
| `tooling/` | Windows developer routing under `tooling/windows/`; one `manage.cmd` entrypoint |
| `.workspace/` | ignored/generated developer outputs, distinct from user-owned data |
| `.agents/` | existing bounded skills, one registry, one permission policy, evaluation corpora |
| `.github/` | GitHub service metadata and optional manual verification workflows |

`Local` is the **GitHub branch** for development, not a user-PC directory. `main` is final-only until separately authorized. `My Voice` is the product/UI name; `VoiceLab` persists only where source protocol/storage compatibility requires it.

## Document and knowledge terms

`id` is durable document identity; path is navigation. Metadata `class`, `domain`, `role`, `authority`, `lifecycle` classify the document, not the truth of an executed test.

- `CANONICAL` — current rule only for the concern it owns
- `REFERENCE` — useful context, not current requirement
- `HISTORICAL` — old evidence/rationale, never current source proof
- `DERIVED` — reconstructible index/graph/retrieval output, not independent state

`ROUTER` links to owners; `GUIDE` provides navigation; `CONTRACT` defines an invariant; `ARCHITECTURE` explains distinct structure. Stable IDs may survive physical moves. Route through [documentation](../README.md), not broad file scans or name guesses.

## File and module decisions

Use size and dependency counts only as signals for focused review, never automatic blockers or targets. Split by independent feature responsibility, state/lifecycle ownership, or a proven reuse boundary. Keep simple presentation next to its existing owner when extracting it would only increase props, imports, and tests. Rejoining an artificial wrapper is a maintenance correction, not a naming migration.

Use precise names identifying the thing owned; avoid `common`, `utils`, `manager`, `service` or alias-only wrappers unless consumers establish a genuinely shared boundary. Extend an existing semantic owner before adding a new file. For Rust/TS/Python bridge contracts, [source ownership](../knowledge/source-ownership.md) identifies the owner; [verification](../knowledge/current-validation.md) identifies the claim-specific proof.

A generated artifact, test fixture, external license/reference, signed package and runtime user data each retain their own identity and lifecycle. Never confuse them with canonical source.

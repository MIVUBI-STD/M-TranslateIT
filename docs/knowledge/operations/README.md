---
id: document.knowledge.operations.router
class: DOCUMENT
domain: knowledge
role: ROUTER
authority: CANONICAL
lifecycle: ACTIVE
---

# Operations

Retain only repeatable operator/developer procedures that are still current and have a real owner.

Suitable examples:

- local/target acceptance runbooks;
- release/install recovery procedures;
- reproducible evidence capture that cannot live cleanly beside the owning script.

Do not store milestone status, per-task reports, chat handoffs, historical CI audits, architecture plans or one-use checklists here. Continuation belongs in `../../../planning/development.md`; proof interpretation in `../current-validation.md`; history in Git.

## Current runbooks

- [REA external-system investigation](./rea-investigation.md) — scoped, authorized reverse engineering and source/runtime evidence reconciliation for external competitors and local artifacts; never a shipped dependency or substitute for native translation acceptance.

- [`target-windows-performance.md`](target-windows-performance.md) — one integrated Windows source build and developer-mode functional baseline for real Meeting latency, audio stability, CPU/RAM/GPU/VRAM pressure, Stop, and repeated-session decisions; controlled package/install acceptance remains a separate release gate.

- [Signed updater release](./signed-updater-release.md) → signed one-shot updater release/publication procedure and native acceptance boundary.

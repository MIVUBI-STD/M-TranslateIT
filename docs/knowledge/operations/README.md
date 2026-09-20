# Operations

Retain only repeatable operator/developer procedures that are still current and have a real owner.

Suitable examples:

- local/target acceptance runbooks;
- release/install recovery procedures;
- reproducible evidence capture that cannot live cleanly beside the owning script.

Do not store milestone status, per-task reports, chat handoffs, historical CI audits, architecture plans or one-use checklists here. Continuation belongs in `../next-action.md`; proof interpretation in `../current-validation.md`; history in Git.

## Current runbooks

- [`target-windows-performance.md`](target-windows-performance.md) — one TARGET_WINDOWS baseline for real Meeting latency, audio stability, CPU/RAM/GPU/VRAM pressure, Stop, and repeated-session decisions after source checks are green.

- `signed-updater-release.md` → signed one-shot updater release/publication procedure and native acceptance boundary.

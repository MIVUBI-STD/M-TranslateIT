# TranslateIT Tooling

Platform-specific optional developer routing, distinct from the Python repository/quality `tools/` family.

```text
DEV.cmd
→ tooling/windows-toolchain/dev.ps1
→ existing npm / Cargo / uv / packaging owners
```

This is a **thin entrypoint**, not a competing build system, application runtime or mandatory ChatGPT workflow. Local Windows commands may be used by a developer who intentionally has a checkout; ChatGPT-owned repository work remains GitHub source-only and must not demand incremental user-PC commands.

### Developer local test (no release signing)

`DEV.cmd doctor` validates the existing pinned toolchains without installing anything. `DEV.cmd setup` explicitly installs the lock-resolved development dependencies once when required. `DEV.cmd check` runs the canonical source/test suite against the already-synced worker environment without silently syncing packages. `DEV.cmd build` uses the existing SHA/clean-tree guarded frontend+Rust compile check; set `TRANSLATEIT_EXPECTED_SHA` externally before the integrated acceptance build. `DEV.cmd run` launches the existing Tauri development app (`npm run dev:app`) for the one planned local functional test; actual local models/audio devices must already be available and runtime readiness must still prove itself.

### Controlled signed release (separate gate)

`DEV.cmd package "<quality-readiness-report.json>"` delegates to the existing release build with **explicit Quality Readiness evidence**. Signing keys, verified model/provider payload, third-party notices and all other release requirements remain mandatory; the developer test entrypoint is not a substitute for an installable package or E3–E6 evidence.

[Toolchain policy](../toolchain.json) defines supported tools; [repository tools](../tools/README.md) own static governance verification. See the [integrated Windows acceptance runbook](../docs/knowledge/operations/target-windows-performance.md).

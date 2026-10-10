# TranslateIT Tooling

Platform-specific optional developer routing, distinct from the Python repository/quality `tools/` family.

```text
DEV.cmd
→ tooling/windows-toolchain/dev.ps1
→ existing npm / Cargo / uv / packaging owners
```

This is a **thin entrypoint**, not a competing build system, application runtime or mandatory ChatGPT workflow. Local Windows commands may be used by a developer who intentionally has a checkout; ChatGPT-owned repository work stays on GitHub/cloud and must not demand user-PC commands. [Toolchain policy](../toolchain.json) defines supported tools; [repository tools](../tools/README.md) own static governance verification.

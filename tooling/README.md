# TranslateIT Windows Developer Tooling

`manage.cmd` is the **optional developer convenience** launcher, **not a step ChatGPT assigns** to a user's PC during routine source development.

```text
manage.cmd
  -> tooling/windows/manage.ps1
     -> tooling/windows/modules/Workspace.psm1
     -> existing npm, Cargo, uv and signed release owners
```

The only authoritative development requirements are
`tooling/windows/config/requirements.json` and the existing Node, Cargo
and Python lockfiles. These are review/pin inputs, not download destinations.

| Command | Current meaning |
|---|---|
| `manage.cmd status` | Inspect required toolchain without installing or repairing |
| `manage.cmd prepare` | Install locked frontend and Python **developer dependencies** into their existing local paths; cache under `.workspace` |
| `manage.cmd verify` | Source/contract tests only, **not** model or device readiness |
| `manage.cmd build` | Existing exact-SHA frontend and locked Tauri Rust compile gate |
| `manage.cmd run` | Existing Tauri development app, needing real local models/audio |
| `manage.cmd test` | Existing frontend, Rust and worker regression suites |
| `manage.cmd package "<quality-report.json>"` | Controlled signed R3 release, separately gated; not a development setup shortcut |

Process-scoped cache settings do not reinstall Rustup, Windows SDK, WebView2
or VB-CABLE, and do not make installed app data portable. The unchanged R3
staging flow still needs a separate safe environment/retry migration before
`prepare` can include its downloads. Do not delete or reset any
`UserData/SavedProject` content during build, verification or preparation.

[Workspace ownership](../docs/system/windows-workspace-boundaries.md) ·
[Windows acceptance](../docs/knowledge/operations/target-windows-performance.md) ·
[Repository tools](../tools/README.md).

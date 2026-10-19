---
id: document.system.windows-workspace-boundaries
class: DOCUMENT
domain: system
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# Windows Workspace Ownership Boundaries

The Windows development root is derived from the location of `manage.cmd`,
not a fixed C: or D: drive. This is a **file ownership contract**, not proof
that installed TranslateIT or every Windows tool has become portable.

## Folder owners and durability

| Directory | Semantic owner | Retention rule |
|---|---|---|
| `EngineData/Frontend/RustApp/` | Tracked Svelte/Tauri/Rust source | Retain source; derived `node_modules`, `dist` and `src-tauri/target` are renewable |
| `EngineData/Backend/LocalWorker/WorkerRuntime/` | Tracked canonical Python worker and one `uv.lock` | Retain source; developer `.venv` is renewable, never release staging |
| `EngineData/Backend/LocalWorker/PythonRuntime/` | Controlled private packaged Python runtime | Never remove until verified replacement and recovery |
| `EngineData/Backend/RuntimeAssets/` | Checked ASR, translation, GPT-SoVITS, voice references and provider inputs | Runtime assets **are not generic cache**; pin/hash verification before replacement |
| `tooling/windows/` | One Windows developer command router, requirements and environment module | Tracked source; not a second build system or model downloader |
| `.workspace/` | Generated development caches, downloads, staging, temp and reports | Ignored by Git; renewable with explicit safe cleanup and stage recovery |
| `UserData/CacheData/` | Temporary application runtime data | Runtime-only cleanup after session ownership is released |
| `UserData/LogData/` | Minimal/redacted diagnostics | Product-owned retention and privacy rules |
| `UserData/SavedProject/` | User-owned accepted My Voice/recordings, backups and Meeting transcript archives | **Never deleted, moved or overwritten by developer prepare/build/reset** |
| `docs/`, `planning/`, `tools/` | Tracked documentation, planning and repository checks | Not generated data |

## Generated development workspace

```text
.workspace/
  tools/
    node/                 # pinned official Windows x64 Node/npm ZIP
    python/               # uv-managed pinned CPython 3.12.10 nested install
    uv/                   # pinned uv.exe ZIP
    cargo/bin/            # local rustup.exe, rustc.exe, cargo.exe proxies
    rustup/toolchains/    # rustup-managed installed toolchain (not auto-downloaded)
  downloads/verified/    # SHA-256 verified reusable release archives (no private content)
  cache/npm/
  cache/uv/
  cache/pip/
  cache/huggingface/
  staging/               # unfinished stages must not be silently deleted
  temp/
  reports/
```

Explicit `manage.cmd prepare` now uses `PortableToolchain.psm1` to fetch the pinned Windows x64 ZIP/executable archives through the SHA-256 checked workspace cache, install uv-managed Python, and bootstrap Rustup/Rust using project-local homes. Interrupted or partial installations fail closed; no system-wide PATH modification or user-owned data removal is permitted. Full first-install and MSVC interoperability on the target PC remain untested.

The `Toolchain.psm1` owner prefers **complete** tool bundles under
`.workspace/tools/`. The read-only status shows `workspace`,
`external`, or `INCOMPLETE_LOCAL` without silently mixing local and
system executables. A partial local bundle fails closed before any
developer command mutates dependency files. `CARGO_HOME` routes Cargo
registry/git downloads to `.workspace/tools/cargo/`; `RUSTUP_HOME`
is set there only when a complete local Rust proxy/toolchain exists.
This selection is **not** a tool installer or evidence of full portability.

`tooling/windows/modules/Workspace.psm1` assigns per-process `TEMP`,
`TMP`, `npm_config_cache`, `UV_CACHE_DIR`, `PIP_CACHE_DIR`,
`HF_HOME`, Hugging Face caches, and uv-managed Python/tool locations.
It must never change global `USERPROFILE`, `APPDATA` or `LOCALAPPDATA`
nor route private runtime data into `.workspace`. Existing workspace
directories that are reparse points are rejected instead of followed.

The current `manage.cmd status` is read-only and reports actual workspace versus external tool origins; `verify` can write an ignored local source-readiness JSON report. Neither claims models, GPU, native audio or system components have been tested. `prepare` installs locked
**development dependencies**, not AI models or Windows audio drivers.
`verify` runs source-level tests, not live GPU/Meeting/device readiness.
The model download authority remains
`EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json` and
`prepare_model_assets.py`. No duplicate provider/downloader is authorized.

## Exceptions and unfinished integration

- Release staging now uses an isolated `UV_PROJECT_ENVIRONMENT` inside
  `.workspace/staging/` and repository-owned uv/Hugging Face caches. The
  release staging wrapper creates a unique private payload tree, verifies its
  pinned inputs, then promotes only five approved runtime subtrees with backups
  and rollback on ordinary failures. If the process is interrupted during
  promotion, `release-rollback` remains a fail-closed recovery marker: **never
  delete that marker automatically or retry staging until reviewed**.
  Full GB-scale release execution, crash interruption, filesystem reparse
  cases, installer payload consistency and model usability remain unexecuted;
  no automatic invocation from developer `prepare` is permitted.
- Rustup, Cargo, MSVC/Windows SDK, WebView2, Windows driver store and OS
  installer state are not yet relocated/verified. Zero system-drive writes
  cannot be guaranteed by setting process cache variables alone.
- Developer Tauri resolves writable app data in repository `UserData/`.
  Packaged Tauri currently resolves `app_local_data_dir()` separately;
  self-contained installed-app storage requires distinct ACL, privacy,
  data-migration, uninstall and crash-recovery acceptance.
- Existing signed NSIS Setup + R3 payload paths are an independent release
  contract. Do not move their roots merely for development naming symmetry.
- Source Windows CI at an exact `Local` SHA cannot establish real CUDA,
  microphone, VB-CABLE, device output reception, installation or model quality.

**Safety rule:** never include `UserData/SavedProject` in any developer
cleanup/rollback, and never delete an existing asset or backup before its
verified replacement and explicit ownership are established.

# RustApp Scripts

This directory contains TranslateIT's source validators and Windows release entrypoints. Acceptance is scenario-based per `docs/foundation/03-acceptance-scenarios.md`; the former all-in-one target-PC harness was retired (decision D-031).

## Source checks

```text
validate:source-contracts
├─ bridge contract
│  ├─ Tauri command registry ↔ frontend invocation parity
│  ├─ selected Rust response struct ↔ TypeScript field parity
│  └─ no explicit `any` escape at bridge boundaries
├─ frontend source reachability from src/main.ts
├─ source-size ownership budget
├─ Meeting virtual-route contract
├─ Rust/Tauri manifest preflight
└─ R3 Tauri/package source checks

validate:quick
└─ source checks + TypeScript/Svelte typecheck + frontend runtime-policy tests
```

The bridge contract is intentionally one owner. Do not add a second validator that separately re-checks command parity or bridge type escape rules. Frontend reachability catches orphan `.ts`/`.svelte` modules that normal unused-local checks cannot see.

Functional behavior coverage remains with the owning runtime: GPU worker smoke (`WorkerRuntime/run_realtime_worker_smoke.ps1`), Rust unit tests for session/audio decisions, and pytest contract tests. Static source checks do not claim target-Windows runtime behavior.

`check:tauri-rust-local` is explicit local compile verification and is not run merely to validate documentation/source routing.

## R3 release boundary

User-facing release shape:

```text
src-tauri/target/translateit-release/
├─ TranslateIT-Setup.exe
└─ TranslateIT-Payload.7z
```

`build_release.ps1` is the controlled Windows release entry. It requires a committed tracked working tree, records the exact Git source commit, validates staged inputs, applies the reviewed release optimizer, regenerates third-party notices, builds the external payload, renders the NSIS hook, builds Tauri/NSIS, and requires the user-facing release directory to contain exactly Setup + Payload. Build evidence under ignored `src-tauri/target/` records the source commit, both SHA-256 values, and the app/payload identity.

## Release input staging

A plain Git clone intentionally does **not** contain the large release runtime/model payload. `stage_release_inputs.ps1` (PowerShell-version compatibility entrypoint over `stage_release_inputs_impl.ps1`) prepares the controlled inputs for `build_release.ps1` and for CI's payload-proof job:

```text
Node.js / npm
host Python 3.12.x + uv (no system pip installs)
Rust toolchain (cargo/rustc)
7-Zip CLI
Visual Studio 2022 C++ Build Tools
```

It stages the private Python/dependencies, ASR, MiLMMT, GPT-SoVITS assets/source, NLTK data, FFmpeg, and VB-CABLE, then `stage_release_license_material.py` completes reviewed license material. Its package environment is now isolated from developer `.venv` and system pip; shared downloads/cache live under the repository `.workspace`. Runtime inputs are now built in an isolated payload tree; promotion has reviewed source-level backup/rollback logic and fails closed on a retained interrupted backup. Full GB-scale native execution and crash recovery are **not yet proven**, so this staging must not be run automatically by development `prepare`. Pinned release archives with reviewed SHA-256 are now reused from the repository-local `.workspace/downloads/verified/` cache, rejecting poisoned bytes; the unpinned GPT-SoVITS source archive is never treated as a verified cached input. The first preparation may download several GB. Do not manually copy models, install a second Python runtime into the product, or bypass the staged identity/hash checks.

Windows PowerShell 5.1 cannot run `Set-Content -Encoding utf8NoBOM`; staging keeps an explicit compatibility boundary in which PowerShell 7+ executes the implementation unchanged while 5.1 executes a temporary same-directory copy with only the unsupported encoding enum replaced for the temporary generated Python helper. Runtime/model sources, revisions, hashes, identity, and staging behavior are otherwise unchanged.

## Acceptance

Acceptance runs one scenario at a time per `docs/foundation/03-acceptance-scenarios.md`. Installer-bound checks are deferred Group E scenarios that call `build_release.ps1` directly; its build evidence under ignored `src-tauri/target/` records the source commit plus both SHA-256 values. Headless runtime evidence lands under `UserData/LogData/RustAppValidation/`.

## CI ownership

`.github/workflows/release-payload-verify.yml` is the single R3 release workflow owner:

- relevant pull requests to `Local` run the source-contract job;
- relevant pushes to `Local` run the source-contract job and the controlled Windows payload-proof job;
- the source-contract job parse-checks the Windows release/staging PowerShell helpers;
- canonical MiLMMT validation also preserves the release staging model-acquisition contract;
- there is no manual-dispatch path for the current branch model.

Worker dependency-lock consistency is owned separately by the read-only WorkerRuntime lock workflow. Do not create overlapping release or proof workflows.

## Proof boundary

Source/hosted verification can establish declarations, controlled staging, payload structure, build evidence, and acceptance-tool syntax. It does not prove actual Windows Setup execution, driver consent/restart, installed model execution, GPU/audio behavior, Meeting delivery, or clean-machine readiness.

## Rules

- Do not re-embed the large R3 payload in `tauri.release.conf.json`.
- Do not add network bootstrap/model downloads to installed Setup.
- Do not add a user-facing 7-Zip dependency, manual extraction flow, or second installer.
- Do not create another release pipeline merely to gather duplicate evidence.
- Keep generated hook/payload/build/acceptance evidence under ignored output paths.

# Signed Updater Release

This runbook owns the release-operation boundary for TranslateIT app updates. It does not replace target-Windows acceptance.

## Product contract

TranslateIT checks for an app update once per application launch.

It does **not** run an updater daemon, periodic polling loop, Windows service, tray updater, or scheduled task.

Update application is blocked while Meeting/Mic Test, My Voice recording, or a My Voice build owns runtime resources.

## Required signing material

Release builds require:

```text
TRANSLATEIT_UPDATER_PUBLIC_KEY
TAURI_SIGNING_PRIVATE_KEY
```

or:

```text
TRANSLATEIT_UPDATER_PUBLIC_KEY
TAURI_SIGNING_PRIVATE_KEY_PATH
```

The private key must remain outside repository history and release artifacts. The public key is compiled into the release build for update verification.

## App-only update boundary

The automatic updater is intentionally app-only for the current release shape.

When Tauri launches the NSIS installer with `/UPDATE`, Setup verifies the already-installed external runtime and preserves it instead of requiring `TranslateIT-Payload.7z` beside the temporary updater installer.

The app-only update is allowed only when the installed runtime still matches the current required:

- payload schema;
- Python/Torch/Transformers/Tokenizers identities;
- ASR revision;
- MiLMMT revision;
- GPT-SoVITS revision;
- required built-in voices;
- VB-CABLE provider presence.

If those requirements change, the automatic installer fails closed before replacing the app. That release requires the full colocated:

```text
TranslateIT-Setup.exe
TranslateIT-Payload.7z
```

This prevents routine UI/Rust/worker fixes from redownloading multi-gigabyte model/runtime assets while still blocking an incompatible app/runtime combination.

## Publication surface

The installed app checks:

```text
https://github.com/MIVUBI-STD/M-TranslateIT/releases/latest/download/latest.json
```

A production update is incomplete until the matching GitHub Release contains a valid updater manifest and the signed Windows updater artifact referenced by that manifest.

Publishing a GitHub Release is an explicit release action. A push to `Local` must never publish an update automatically.

## Release sequence

```text
validated Local source
→ Quality Readiness evidence
→ controlled Windows release build
→ signed updater artifacts
→ inspect version + hashes + signature
→ create explicit GitHub Release
→ attach latest.json + referenced updater artifact/signature
→ installed previous version performs one-shot startup check
→ user explicitly starts update
→ installed version is re-verified
```

## Version discipline

The application version must match across the package and Tauri configuration before release packaging. Do not overwrite an already published version with different bytes.

Use a new semantic version for every published updater artifact.

## Required negative tests

Before calling updater delivery production-proven, verify on an installed Windows build:

- no network / GitHub unavailable → startup remains usable;
- malformed or unavailable `latest.json` → startup remains usable;
- invalid signature → update is rejected;
- current version already latest → no update UI;
- update available → one update action appears;
- Meeting/Mic Test active → installation is blocked;
- My Voice recording active → installation is blocked;
- My Voice build active → installation is blocked;
- successful signed update → new version installs and relaunch/reopen succeeds;
- `UserData` and approved My Voice remain intact.

## Proof boundary

REMOTE_GITHUB can prove source contracts, signing requirements, artifact-generation configuration, and CI correctness.

Only TARGET_WINDOWS / NATIVE_ACCEPTANCE can prove an installed old version discovers, verifies, installs, and survives a real signed update.

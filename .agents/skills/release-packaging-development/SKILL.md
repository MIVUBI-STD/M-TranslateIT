---
name: release-packaging-development
description: TranslateIT specialist for Windows release/deployment: offline Setup/payload, private Python runtime, dependencies/models/voice assets, installed layout, third-party provenance/notices, Meeting audio-provider delivery, uninstall/reinstall and clean-machine proof. Do not redesign AI/audio/desktop behavior here.
---

# Release Packaging Development

## Current release shape

```text
working source
→ controlled release staging
→ TranslateIT-Setup.exe + TranslateIT-Payload.7z
→ installed TranslateIT on Windows
```

Normal users do not manually install Python, run pip, download core models, extract runtime folders or operate GPT-SoVITS tooling.

## Owns

- installer/package configuration;
- private Python runtime + locked runtime dependencies;
- required ASR/translation/GPT-SoVITS/built-in voice assets;
- deterministic installed resource layout;
- VB-CABLE/provider delivery boundary;
- fresh UserData initialization and uninstall/reinstall behavior;
- third-party licenses/notices/source provenance;
- clean-machine deployment proof.

## Rules

- Packaging difficulty does not justify a second runtime architecture.
- Repository/dev paths are not production resolution fallbacks.
- Model/runtime assets outside Git remain controlled release inputs.
- Installer generated ≠ installed runtime verified ≠ clean-machine verified.
- Never package historical docs, user recordings, private logs, developer secrets or test user data.
- Build-time tools do not become user runtime dependencies without product need.
- Built-in LibriSpeech/OpenSLR references are CC-BY-4.0 material; release provenance/attribution must preserve exact source/utterance/hash/license metadata. Do not call them public domain.
- Provider notice presence does not itself prove redistribution rights.

## Procedure

1. Establish the exact source revision, authorized release goal and immutable package inputs.
2. Identify the first wrong staging, signing, installer, attribution, asset or installed-path owner.
3. Preserve one packaged Python/AI/audio architecture; never add dev-path or missing-model runtime fallbacks.
4. Inspect third-party rights and model/voice/provider provenance before copying release assets.
5. Validate source/payload contracts and bounded artifact evidence using existing package entrypoints.
6. Separate artifact compilation, installation and clean-machine/native Windows evidence; report unavailable layers.
7. Treat signing, credentials, publication and promotion to main as separate approval gates; never infer them from source PASS.

## Proof hierarchy

Root `GITHUB_RULES.md` limits GitHub to **source/version control only**: no cloud compute, hosted CI, remote build, or cloud inference. Keep build/model/device proof `UNKNOWN / NOT EXECUTED` until the grouped local build-and-native acceptance phase; never request per-commit tests on the user's PC.

```text
configuration/source proof
→ artifact build proof
→ installed-runtime proof
→ clean-machine proof
```

Use only the level needed by the claim.

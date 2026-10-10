---
name: local-ai-runtime-development
description: TranslateIT specialist for the canonical local AI worker: ASR, Indonesian↔English translation, bounded outbound Meeting context, context-free incoming translation, GPT-SoVITS voice inference, built-in voice behavior, My Voice build/inference, model lifecycle and CUDA/CPU capability truth. Do not use for microphone/device routing, desktop presentation or packaging delivery.
---

# Local AI Runtime Development

## Current contract

```text
one canonical worker
ASR
→ one canonical ID ↔ EN translation pipeline
→ selected Meeting voice synthesis
```

There are no user-facing Realtime/Quality translation modes.

Meeting context:

```text
outbound → last 3 committed own-voice ID→EN pairs from same session
incoming → context-free
Text     → standalone, no Meeting-context reuse
```

Meeting voice:

```text
Built-in Male/Female → available without My Voice training
My Voice → optional trained GPT-SoVITS V2ProPlus actor after explicit approval
```

## Owns

- worker orchestration and request/response contract;
- ASR inference;
- bidirectional translation inference;
- outbound context injection and incoming context exclusion;
- model/provider load/readiness;
- GPT-SoVITS synthesis;
- built-in voice runtime contract;
- My Voice build/evaluation/inference runtime;
- CUDA/CPU capability truth and AI-stage performance evidence.

## Does not own

Physical mic/VAD/device routing; Meeting output endpoint; desktop navigation/readiness copy; installer/runtime asset delivery.

## Rules

- One canonical pipeline; do not retain a normal fallback engine/router to hide integration failure.
- CPU fallback handles named capability absence only; broad exception fallback is prohibited.
- Cloud fallback is never automatic.
- Source/model presence does not prove model load/output quality/performance.
- Translation must be complete or explicitly fail; no silent truncation.
- Context metadata must reach the actual outbound inference request before contextual translation is claimed.
- Incoming must never consume outbound/participant history.
- My Voice training is bounded and mutually exclusive with active Meeting inference.
- Built-in voice readiness and trained My Voice readiness share one selected-Meeting-voice product contract rather than two competing TTS owners.

## Tooling gate

Use `uv`, Ruff, pytest, benchmark/profiling tools or type checking only when they directly improve the current canonical runtime's reproducibility/correctness/proof. Tool adoption is not a reason to create another project skill or packaged end-user dependency.

## Procedure

1. Pin current canonical worker and expected ASR/translation/TTS or My Voice product contract.
2. Identify the first incorrect worker/provider/request owner and trace the exact caller and output envelope.
3. Preserve outbound last-three-committed-pairs context, context-free incoming, one translation pipeline and selected-voice authority.
4. Repair the smallest correct worker/provider owner; never create silent cloud/alternate-engine fallbacks.
5. Check relevant unit/protocol fixtures, model identity and affected frontend/Rust consumers without widening unrelated layers.
6. Distinguish source contract, model-load, CUDA/CPU performance and perceptual voice/translation quality proof.
7. Report unavailable model/device proof as residue, then STOP; a source change is not inference-quality acceptance.

## Proof

Root `GITHUB_RULES.md` controls ChatGPT/cloud-only execution and the **integrated test-ready candidate** gate. Do not assign the user's PC as an incremental test runner; name unexecuted native-only proof without claiming PASS.

GitHub/static proof can establish wiring/contracts/model identity. Actual model load, CUDA behavior, translation quality, generated speech quality, speaker similarity and practical latency require matching runtime/hardware evidence.

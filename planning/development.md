# Active development — My Voice SAFE_8GB

**State:** SOURCE_CANDIDATE / NOT EXECUTED. **Scope:** RTX 3070 8 GB, Windows, one GPT-SoVITS V2ProPlus training pipeline. This document records current intention, not runtime proof. Earlier implementation history stays in `Local` commits and the detailed reference `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md`.

## Canonical source ownership

- **Rust/Tauri:** sole Create/Improve resource authority, Meeting exclusion, run identity and completion receipts, native memory/disk admission, Windows build-process Job Object, final evaluation/approved-voice promotion and recovery archive.
- **Python:** one existing local training coordinator; `my_voice_gpu_policy.py` (preflight arithmetic), `my_voice_adaptive_safety.py` (pure NORMAL/WATCH/PRESSURE/CRITICAL trend rules), `my_voice_stage_supervisor.py` (owned child lifecycle and telemetry), `my_voice_cooperative.py` (scoped checkpoint-boundary request/ack), `my_voice_single_gpu_adapter.py` (pinned SoVITS/GPT single-GPU compatibility). No second runtime/provider.
- **Svelte:** presentation only; read existing Rust build snapshot and safely bound hardware advisory. No independent training state writer or invented percent/ETA.
- **Pinned upstream:** GPT-SoVITS V2ProPlus source/model revisions remain unchanged on disk; adapter source-hash mismatch fails closed. The English-only actor/evaluation contract and eight held-out reviews remain unchanged.

## Source-candidate behaviors

1. **Admission:** native Create and Improve refuse unknown/insufficient current CUDA headroom or native RAM/disk readings. Provisional safe profile starts with physical batch size 1 for SoVITS and GPT, sequential GPU stages, and a bounded PyTorch allocator fraction (not a global VRAM ceiling). CPU fallback is not silently enabled.
2. **Isolation:** Windows build and stage children request suspended launch -> Job Object assignment -> resume. KILL_ON_JOB_CLOSE and stage descendant accounting prevent knowingly continuing with orphaned work. Windows compatibility is unverified.
3. **Adaptation:** stable high usage => WATCH/continue; 3-sample downward trend in constrained GPU/RAM/disk => PRESSURE, create one run-scoped cooperative stop request; SoVITS/GPT adapter honors it only at an eligible configured checkpoint epoch. State changes are reported through the **existing** worker status writer, not every sample.
4. **Emergency:** consecutive critical or unknown resource readings => kill only the owned stage tree. Unexpected crashes, nonzero exit or missing/unsolicited cooperative ack never become successful training. A requested stop without checkpoint ack cannot silently advance to another stage.
5. **Integrity:** unique generation+dataset identity, monotonic worker phase, verified completion receipt after actual child success, isolated candidate evaluation and held-out SHA-256 checks. Training failure never automatically promotes a candidate or switches the Meeting actor.
6. **Recovery:** incomplete training checkpoint files remain **unverified candidates** and are copied only with measured free disk capacity. No automatic optimizer resume, cloud training, checkpoint deletion, release-ready declaration or hidden retry.

## Explicit limitations

- This is **source written to GitHub only**. No Rust/Node/Python suite, static checker, target-Windows compilation, CUDA/GPU run, generated voice listening, Meeting end-to-end test, packaging/install, fault injection or backup restore was executed during the requested source-only work. Source comparisons are not acceptance tests.
- 8 GB is a design target and 12 GB is optional headroom, not a mandatory minimum. Provisional VRAM/RAM/disk thresholds may still block viable work or fail to prevent OOM; actual safe operating envelopes remain unknown.
- Cooperative stop preserves only a possible checkpoint boundary; the model/optimizer/scheduler/AMP continuity is **not resume-certified**. UI `Stop Creating` remains the existing immediate cancellation route, not this new cooperative mechanism.
- Global free VRAM, PyTorch allocated/reserved and OS GPU indices are not yet independently reconciled to a stable physical device identity. GPU driver faults and sudden OOM can occur between sensor readings. Checkpoint writes in pinned upstream code are not guaranteed transactional.
- Initial stage process ownership, nested Job Object behavior, Lightning callback order, pinned source adapter runtime behavior and low-memory pipeline semantics all require future Windows evidence. Never claim zero-crash safety.

## Single next step (source-only)

Review/strengthen stable GPU identity across native CUDA and NVIDIA telemetry; then inspect checkpoint writer atomicity, heartbeat provenance and graceful manual-stop integration **without starting training or tests**. Keep all changes on `Local`; preserve the selected Meeting voice and immutable audio/archive lineage. Only move to the canonical `docs/knowledge/operations/target-windows-performance.md` integrated acceptance when the user explicitly authorizes validation.

---
id: document.knowledge.decisions.router
class: DOCUMENT
domain: knowledge
role: ROUTER
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Decision Register

Durable decisions store **why a current rule exists**. Current product law belongs in `docs/foundation/`; active status belongs in `next-action.md`; proof belongs in `current-validation.md`; historical detail belongs in Git history or `history-legacy.md`.

Historical rationale is retained as [historical evidence](./history-legacy.md), not current product authority.

## Current decisions

### D-001 — Local-only repository authority

`Local` is the sole active repository authority for development, governance, CI, proof, continuation, and release-source validation.

Other branches are outside the current development lifecycle and are not fallback source, proof, continuation, PR, or promotion targets unless the user explicitly changes this policy.

Reason: the team explicitly works only from `Local`; repository mechanics must not create a second active authority or silently promote state elsewhere.

### D-002 — One desktop product and one canonical local worker

Keep one Tauri/Rust desktop application and one normal Python worker path for ASR, translation, and voice inference. Parallel normal engines/fallback stacks require a new explicit product decision.

### D-003 — Reliable translation before feature breadth

Meeting/Text reliability, completeness, understandable translation, and safe delivery outrank additional modes/workspaces/automation.

### D-004 — One Meeting lifecycle authority

The Rust Meeting session/runtime owner controls Ready → Starting → Live → Stopping → Ended, stale-work rejection, and safe Stop/Close behavior.

### D-011 — Svelte 5 desktop architecture is current

The active frontend is Svelte 5 + Vite + TypeScript inside Tauri 2, with Tailwind CSS 4, semantic CSS tokens, selective Bits UI, and Lucide Svelte. This is current architecture, not a pending migration.

### D-019 — CUDA preferred; capability failures remain truthful

Use validated acceleration when available. CPU/capability fallback handles named supported conditions only; unknown failures are not masked as healthy degradation.

### D-020 — My Voice is one trained GPT-SoVITS V2ProPlus upgrade

My Voice uses authorized guided recordings, held-out evaluation, and explicit approval. Training is occasional; normal Meeting inference reuses the approved actor.

### D-021 — Standard VB-CABLE is the initial Meeting audio-provider direction

Provider delivery is a release boundary; detection/configuration/use remains Windows-audio runtime ownership.

### D-022 — Shared GitHub operating discipline

TranslateIT follows the same reusable governance discipline as PRD-Creator/BuildIT, adapted to its Local-only branch model:

```text
PIN
→ EXECUTION CONTEXT
→ GitHub-first partition
→ READ MINIMUM
→ DIAGNOSE first wrong owner
→ TOOL + TRANSFER GATE
→ WRITE ONCE
→ VERIFY + FAILURE POLICY
→ STOP
```

Domain-specific proof contexts are `REMOTE_GITHUB | LOCAL_CODE | TARGET_WINDOWS`.

### D-023 — Offline Setup + colocated payload

Controlled distribution uses one user-facing Setup plus colocated payload files. Users do not manually operate Python/pip/core-model setup.

### D-033 — Asymmetric Meeting context

Outbound translation may use the last three committed own-voice translation pairs from the same session. Incoming meeting audio remains context-free.

### D-034 — Current product direction locks

- one canonical translation pipeline;
- no Realtime/Quality user modes;
- Text paste/type only; no document translation;
- Indonesian ↔ English only;
- full English UI;
- personal/owned-machine distribution;
- Built-in Male/Female Meeting voices available without training;
- My Voice optional after explicit approval.

### D-035 — Built-in voice source provenance

Built-in references are LibriSpeech/OpenSLR-derived CC-BY-4.0 material with exact source/utterance/hash metadata. They are **not** described as public-domain material.

### D-036 — Stability-first realtime latency architecture

**Context:** current outbound correctness is strong, but synchronous playback can make later utterances wait behind earlier audio, repeated native output setup may add jitter, repeated Start proof may cost warm-start time, and GPT-SoVITS supports fragment return that could reduce first-audio delay.

**Decision:** keep one canonical worker and one Meeting lifecycle owner. Optimize in isolated, evidence-gated phases: (A) one-ahead bounded AI→playback decoupling while retaining full-WAV delivery, (B) persistent per-generation native output stream only if delivery remains material, (C) functional AI proof rebinding only under exact runtime/model/actor identity plus invalidation epoch, and (D) GPT-SoVITS `return_fragment=True` only after safer phases when TTS remains dominant and target quality parity is proven. `docs/foundation/04-realtime-latency-architecture.md` owns the detailed architecture contract.

**Why:** this removes avoidable waiting while keeping translation/TTS quality, output ordering, at-most-once delivery, bounded memory, Stop semantics, and truthful readiness intact.

**Tradeoffs / not chosen:** reject unbounded pipelines, parallel TTS engines, a second worker, automatic uncertain replay, broad mid-session output recovery, `streaming_mode=True`, lower-quality fixed chunks, user-facing Realtime/Quality modes, and weakening Start checks to file presence.

**Evidence / proof boundary:** REMOTE_GITHUB can prove source bounds/order/identity/cleanup contracts. TARGET_WINDOWS is still required for real latency, native callback/device stability, meeting-app reception, resource pressure, and voice-quality claims.

**Follow-up owner:** `meeting_session.rs` for lifecycle/handoff, `engine/audio/meeting_output.rs` for playback/output ownership, `helper_bridge.rs` for proof identity/rebinding, and the canonical WorkerRuntime voice owner only if fragment delivery becomes evidence-justified.

## Recording policy

Record a durable decision only when architecture/workflow/reasoning must survive sessions, multiple owners depend on it, a meaningful tradeoff exists, or an old method is explicitly superseded.

Minimum shape for substantial new entries:

```text
Context
Decision
Why
Tradeoffs / not chosen
Evidence / proof boundary
Follow-up owner
```

Do not create a decision entry for ordinary wording changes, straightforward bug fixes, or generated artifacts.

## Historical register

`history-legacy.md` preserves the former monolithic decision log. It is historical evidence and may contain superseded branch/product language.

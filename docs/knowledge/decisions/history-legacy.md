---
id: document.knowledge.decisions.history-legacy
class: DOCUMENT
domain: knowledge
role: REFERENCE
authority: HISTORICAL
lifecycle: ACTIVE
---

﻿# TranslateIT â€” Decision Log

Durable choices and reasons that must survive chat/session boundaries. Exact implementation detail belongs in current source/foundation owners; active status belongs in `next-action.md`; historical run IDs and superseded proof detail remain in Git history.

## D-001 â€” `Local` Is Development Authority

**Decision**  
`Local` owns current development. `Developing` remains the GitHub default branch and is the only retained historical/recovery branch; it is not a silent fallback source or write target.

**Reason**  
Current work must not drift to the default or historical branch when `Local` is the explicit development authority.

## D-002 â€” One Desktop Product And One Local Worker

**Decision**  
Keep one Rust/Tauri desktop application and one canonical Python worker path for normal ASR, translation, and TTS inference.

**Reason**  
Parallel launchers, workers, readiness services, or compatibility runtimes add failure paths without improving the approved translator.

## D-003 â€” Reliable Translation Core Supersedes Feature Breadth

**Decision**  
Prioritize reliable Meeting/Text translation over broad feature count. Initial top-level product is Meeting / Text / VoiceLab / Settings. VoiceLab was later explicitly reopened by D-020; other deferred feature-breadth exclusions remain unless a new product decision replaces them.

**Reason**  
Feature breadth previously moved ahead of proven translation quality and created stale parallel paths.

## D-004 â€” Meeting Has One Application-Level Authority

**Decision**  
`commands/meeting_session.rs` + `engine/runtime_state.rs` own the Meeting lifecycle:

```text
Ready â†’ Starting â†’ Live â†’ Stopping â†’ Ended
```

Navigation does not recreate/stop Meeting. Stop revokes output authority before cleanup. Optional incoming remains subordinate and may degrade without blocking safe required outbound.

**Reason**  
One authority prevents duplicate output, stale async promotion, and lifecycle races.

## D-005 â€” Direction-Based Local Translation

**Decision**  
Normal translation is selected by Indonesian â†” English direction through the canonical local translation owner. Current source text/utterance is the input; it is not silently truncated or promoted when known incomplete.

**Reason**  
Direction is the approved product contract; user-facing engine/mode selection is unnecessary.

## D-006 â€” One Setup Experience May Use Colocated Offline Payloads

**Decision**  
Initial controlled distribution may place large local runtime/model payloads beside one user-run Setup so installation remains fully offline and deterministic. This is permission for the packaging shape, not automatic approval of a specific final file layout after R3. Do not add first-run download, package manager, manual Python/model setup, or a checksum/registry framework merely to solve distribution.

**Reason**  
One setup experience matters more than forcing all bytes into one physical executable. Exact packaging still must respect current evidence and an explicit release-boundary decision.

## D-007 â€” Runtime Resources And Writable User Data Have Separate Owners

**Decision**  
Packaged runtime resources resolve from the application resource/runtime root; writable cache/log/user state resolves from app-local data. Repository probing is development-only.

**Reason**  
Installed builds cannot safely treat repository-relative resources as writable state.

## D-008 â€” Validation Must Be Proportional To The Claim

**Decision**  
Use a small repeatable source/preflight set plus real compile/runtime/device/package proof when the claim requires it. Do not grow test/report machinery merely because another check is possible.

**Reason**  
Proof quality comes from evidence matched to the exact claim, not verifier count.

## D-009 â€” One Active Frontend Entry And Bounded Diagnostics

**Decision**  
`src/main.ts` is the only normal frontend entry. Normal product surfaces use product-facing states; heavy technical inspection stays in setup/Advanced/Diagnostics boundaries.

**Reason**  
Parallel entrypoints and broad diagnostic polling keep retired architecture alive and waste resources.

## D-010 â€” Installed Worker Uses One Private Embedded Python Runtime

**Decision**  
Installed Windows builds run WorkerRuntime through one application-local private CPython runtime. System Python, repository `.venv`, `py`, and environment overrides are development conveniences only.

**Reason**  
A private interpreter preserves one controlled worker/runtime architecture without freezing or duplicating the worker.

## D-011 â€” Frontend Uses A Small Svelte 5 Architecture

**Decision**  
Use one plain Svelte 5 SPA inside Tauri 2 with Vite, TypeScript, Tailwind CSS 4, semantic tokens, selective Bits UI, and Lucide Svelte. Svelte owns presentation/application state, not duplicate runtime truth.

**Reason**  
Declarative components improve maintainability without requiring SvelteKit, a frontend router, Redux-like state, or another heavy framework.

## D-012 â€” Live Helper Recovery Is Transport-Only And Stage-Bounded

**Decision**  
Automatic Live helper recovery handles proven helper transport/lifecycle failures only. ASR and translation may receive at most one safe retry before output side effects; current synthesis is not blindly replayed after uncertain transport failure.

**Reason**  
Recovery should restore one canonical worker without masking model/content failures or creating duplicate spoken output.

## D-013 â€” Optional Incoming Prioritizes Freshness And Cannot Strand Shared Infrastructure

**Decision**  
Optional incoming may drop stale work under outbound contention. A proven incoming transport failure is not replayed; bounded same-worker recovery may restore shared infrastructure before scheduler ownership is released.

**Reason**  
Required outbound must not inherit a deliberately stopped helper, while old incoming comprehension must not become a backlog.

## D-014 â€” Active Runtime Sessions Freeze Conflicting Audio/Helper Mutation

**Decision**  
While a runtime session owns current resources, conflicting device preference changes, Mic Test/setup repair, and public/manual helper restart are deferred/rejected until canonical Stop.

**Reason**  
Persisted settings or helper lifecycle must not diverge from resources owned by an active Meeting/Mic Test.

## D-015 â€” Meeting Route Requires One Matched Pair Bound To The Session

**Decision**  
Meeting route readiness requires one recognized playback/recording virtual-audio pair. The prepared pair is bound to the Meeting generation and is not silently replaced mid-session.

**Reason**  
Independent virtual-looking endpoints can create false readiness and misleading user instructions.

## D-016 â€” Provider Preflight Has A Bounded Pre-Authority Safety Ceiling

**Decision**  
The Meeting route provider preflight is bounded before Meeting authority commits; the current source safety ceiling exists to contain hangs, not to define a latency SLA.

**Reason**  
A provider/import/device-enumeration hang must not leave Start stuck forever before application authority exists.

## D-017 â€” Windows Power Transitions Converge Through Canonical Meeting Stop

**Decision**  
Sleep/hibernate handling reuses the canonical Meeting Stop owner. Wake does not automatically resume/replay old voice output.

**Reason**  
Power transitions can invalidate capture/device/helper ownership; one Stop path preserves authority-first cleanup.

## D-018 â€” Clean Desktop Utility Is The Approved Visual Baseline

**Decision**  
Preserve a compact dark desktop-utility visual direction: clear task/state hierarchy, narrow sidebar, restrained card/border use, calm healthy states, stronger emphasis only for action/attention, and one dominant normal action.

**Reason**  
Rendered review showed this direction improves usability without changing product/runtime semantics.

## D-019 â€” One Windows CUDA Matrix And Capability-Only CPU Fallback

**Decision**  
Keep one locked WorkerRuntime environment. CUDA is preferred when proven available; CPU fallback is selected only for known capability absence, not as a broad exception fallback after arbitrary CUDA/model/runtime failure.

**Reason**  
A single reviewed dependency matrix is reproducible and prevents real failures from being disguised as healthy CPU degradation.

## D-020 â€” VoiceLab Uses One Trained GPT-SoVITS V2ProPlus Voice Actor

**Decision**  
VoiceLab creates one approved trained `MyVoice` actor using GPT-SoVITS V2ProPlus from guided English recordings, held-out evaluation, and explicit user approval. Daily Meeting inference reuses that actor through the existing canonical worker and does not retrain per startup/utterance.

**Reason**  
The product requirement is speaker fidelity with practical daily use, not provider breadth or instant-clone modes.

## D-021 â€” Initial Meeting Audio Provider Uses Standard VB-CABLE

**Decision**  
The initial controlled Windows provider direction is the standard VB-Audio VB-CABLE package. Runtime pair detection/delivery remains Rust/CPAL ownership; provider distribution/installation is a release boundary.

**Reason**  
One standard pair fits the existing route without introducing a mixer, provider registry, or custom driver.

## D-022 â€” BuildIT-Style GitHub Governance Owns Repository Execution

**Decision**  
TranslateIT adopts the relevant BuildIT operating discipline for ChatGPT/GitHub work:

```text
PIN
â†’ READ MINIMUM
â†’ DIAGNOSE
â†’ TOOL FIT
â†’ WRITE ONCE
â†’ VERIFY MINIMUM
â†’ STOP
```

`GITHUB_RULES.md` is the canonical owner for branch/ref, tool-fit, atomic delivery, history, CI, retry, and STOP discipline. `AGENTS.md` owns boot/mode/continuity routing. Active continuation stays compact in `next-action.md`; ownership maps do not carry milestone status.

The normal repository budget is one logical commit and one ref update per coherent task, relevant CI only, no CI-trigger/proof-only commits, no temporary one-use workflows, maximum two same-cause attempts with new evidence, and no adjacent cleanup unless required.

**Reason**  
Separating GitHub mechanics from product routing and enforcing a small static repository gate reduces branch mistakes, commit spam, evidence inflation, stale continuity, and CI-as-remote-shell behavior without changing TranslateIT product architecture.

**Boundary**  
This governance decision does not change Meeting, Text, VoiceLab, models, Windows audio, installer implementation, or the target-Windows acceptance boundary.

## D-023 â€” One Fully Offline Setup With Colocated External Payload Is Approved

**Decision**  
The release boundary is one user-facing automatic offline Setup plus colocated external payload file(s). Users launch `TranslateIT-Setup.exe`; Setup owns locating, validating, and extracting/installing its colocated payload automatically. Users are not asked to install Python, run pip, download core models, manually extract archives, or run a second installer.

R3.2 establishes `7z/LZMA2` as the current size-first compression candidate because the then-current exact resource set compressed materially smaller than ZIP while preserving byte-identical extracted content. This is evidence for payload representation, not permission to introduce an unrelated user-facing 7-Zip product/runtime.

**Reason**  
The classic single-EXE NSIS boundary is structurally unsuitable for the large offline payload, while the user experience requirement is one automatic offline setupâ€”not one physical file.

**Boundary**  
Do not turn this into a bootstrap download system, package manager, general artifact registry, manual extraction workflow, second setup, or model-quality reduction. Target-Windows installation/clean-machine proof remains separate acceptance evidence.

## D-024 â€” Canonical Translation Moves To M2M100 With Standalone Semantic Segmentation

**Decision**  
Replace the two Marian production translation models with one pinned `facebook/m2m100_418M` bidirectional model at revision `55c2e61bbf05dfb8d7abccdc3fae6fc8512fd636`. Keep one canonical local translation runtime; do not retain Marian as a normal fallback or add a provider router.

Standalone Text preserves blank-line paragraphs and translates conservative semantic/sentence units independently before ordered reassembly. A single oversized semantic unit may use the existing token-safe fallback. Every required unit must complete before any Text result is promoted. Meeting remains one finalized utterance at a time and receives its own later target acceptance.

The runtime uses the pinned model generation profile rather than overriding it with greedy `num_beams=1`. Translation completion must recognize a valid EOS followed only by model padding as complete; padding must not create a false incomplete result, while output with no verifiable EOS remains blocked.

**Reason**  
Target-Windows RTX 3070 evidence showed recurring Marian correctness/quality failures: multi-sentence omission, duplicated dates, numeric corruption (`2100`â†’`200`), technical-fact loss, and name/version corruption. Restoring Marian's model-default beams fixed one omission but did not make the model reliable.

The same target evaluation showed M2M100 materially stronger in both directions for dates, names, numbers, versions, IP addresses, URLs, CUDA terminology, and the original omission case. Whole-text M2M100 still omitted a second ENâ†’ID question in one fixture; semantic segmentation restored it. Across the final whole-vs-segmented fixture set, segmented M2M100 preserved all monitored literals in both directions with warm per-request totals roughly in the 0.25â€“0.52 s range on the tested RTX 3070. Natural-language wording findings remained unresolved quality observations.

**Boundary**  
D-024 records the current implementation direction, not permanent quality acceptance. Do not add generic date/number correction, glossary, back-translation, semantic verifier, cloud fallback, multi-model fallback, or user-facing translation modes merely to mask model defects.

## D-025 â€” Translation Quality Requires A General Benchmark Gate Before Further Runtime Change

**Decision**  
The current D-024 implementation remains the active translation path, but it is **not accepted as final translation quality** after target STEP 2C-B reproduced a meaning-changing EN â†’ ID modality error (`must not` â†’ `tidak harus`). No further production translation change may be made from one failing phrase, one known fixture, or one hand-written decoder experiment.

Before another model, decoder, segmentation, or output-handling change is implemented, the translation work must define and execute a general, reproducible quality gate containing all of the following:

```text
1. external/reference benchmark for both ID â†’ EN and EN â†’ ID
2. product-domain semantic stress suite covering grammar/meaning categories
3. unseen holdout material not used to choose/tune the implementation
4. MT quality metrics appropriate to reference translation
5. deterministic opaque-fact checks for names/numbers/dates/versions/IP/URL-like material where valid
6. human severity review for meaning-changing errors such as negation/modality/reference
7. naturalness/fluency review separate from correctness
8. target RTX 3070 cold-load, warm p50/p90 latency, and whole-device VRAM evidence
9. local/offline, Windows, licensing, and packaging constraints
```

The benchmark itself must not encode one exact expected phrasing as the product definition. Exact-output assertions remain limited to deterministic runtime contracts, not natural-language quality.

Candidate evaluation is bounded to the current baseline plus at most **two serious challengers per round**. A better candidate must replace the canonical model if adopted; do not create a normal production model router/fallback stack merely to win individual examples.

Phrase-specific corrections are explicitly rejected, including hard-coded rules such as `must not` â†’ `tidak boleh`, date repair maps, hand-written grammar rewriting, fixture-targeted dictionaries, or post-processors whose success is defined by examples already seen during debugging.

**Reason**  
The previous migration correctly fixed severe Marian omissions and factual corruption, but target acceptance exposed a process weakness: completeness and literal preservation were used as stronger proxies for overall translation quality than they can support. Semantic segmentation can guarantee that planned source units are attempted; it cannot guarantee correct modality, negation, tense, reference, or natural target-language wording. The `must not` failure proves that a translation may be structurally complete and fact-preserving while still being semantically wrong.

A broad benchmark + holdout gate reduces fixture overfitting and lets quality improvements be evaluated against the required latency/VRAM envelope instead of accumulating targeted patches.

**Boundary**  
Do not resume microphone, VoiceLab, Meeting, installer, or additional translation development until the Translation Quality Improvement Plan built around this gate is researched, critiqued, and explicitly approved. D-025 does not itself choose the next model, decoder profile, benchmark tool, metric, or quantization method.

## D-026 â€” LMT-60-1.7B Is The Approved Translation Evaluation Target

**Decision**  
Use `NiuTrans/LMT-60-1.7B` as the sole preferred challenger and intended canonical target for the next translation-quality round. The current M2M100-418M implementation remains the active production baseline until LMT-60-1.7B passes the frozen D-025 benchmark/holdout and target-Windows quality, latency, VRAM, licensing, offline, and packaging gates.

Round 1 does not evaluate LMT-0.6B, LMT-4B, LMT-8B, MADLAD-400 3B, or another model in parallel. Those may be reopened only if LMT-60-1.7B materially fails the approved gate and new evidence justifies a second bounded round. Do not add LMT as a runtime fallback/router beside M2M100; if adopted, it replaces the canonical translation model.

**Reason**  
The approved model-family audit prioritizes translation quality and low latency, with model size secondary. Published Indonesian LMT scaling results show a material quality gain from 0.6B to 1.7B, followed by substantially smaller gains from 1.7B to 4B/8B. The 1.7B payload is materially more practical for the target RTX 3070 8 GB than the 4B/8B variants, while avoiding the larger quality sacrifice of the 0.6B model. LMT is translation-specialized, supports Indonesian, and uses an Apache-2.0 license suitable for the current product evaluation direction.

**Boundary**  
This is approval to evaluate LMT-60-1.7B, not permission to pre-promote it into `model_manifest.json`, RuntimeAssets, or the production worker. D-025 remains controlling: first freeze the general benchmark contract, then compare M2M100 and LMT-60-1.7B under the exact same unseen/holdout, quality, naturalness, latency, and VRAM procedure. Production changes occur only after that evidence passes.

## D-027 â€” LMT Uses One Approved Runtime Configuration

**Decision**  
If LMT-60-1.7B proceeds through evaluation and production migration, use one approved execution configuration rather than a family of quality/speed profiles:

```text
PyTorch / compatible Transformers 4.x
CUDA on the target Windows RTX 3070
BF16 weights/compute path
AutoModelForCausalLM
upstream LMT translation prompt + chat template
num_beams=5
do_sample=False
use_cache=True with DynamicCache
native PyTorch SDPA attention
resident model after preload
model.eval() + torch.inference_mode()
```

Do not maintain CTranslate2, external FlashAttention2, `torch.compile`, static full-context cache, FP16, INT8/INT4, beam-reduced, speculative-decoding, or other alternate translation runtime profiles as normal options. There is no user-facing Realtime/Quality selection. If the single approved configuration fails semantic quality, practical target latency, or stable 8 GB VRAM operation, stop and reassess the model/runtime decision rather than accumulating additional production profiles.

**Reason**  
The product needs one understandable, reproducible translation behavior. Dynamic KV cache removes repeated autoregressive key/value computation while keeping the selected model weights, BF16 precision, prompt, and beam policy. Native PyTorch SDPA provides optimized CUDA attention dispatch without adding an external Windows attention package. Keeping one backend and one numeric/generation policy minimizes dependency and packaging risk and makes failures attributable instead of creating a matrix of partially equivalent configurations.

**Boundary**  
D-027 does not pre-approve production migration. D-025 still controls quality/holdout evidence and target-Windows latency/VRAM proof. M2M100 remains only the sequential pre-migration benchmark reference until LMT is accepted; after accepted migration it is retired rather than retained as fallback.

## D-028 â€” MiLMMT-46-1B-v1.0 BF16 Is The Selected Translation Migration Target

**Decision**  
Select `xiaomi-research/MiLMMT-46-1B-v1.0` at revision `4fc480b6c58dec29c159dcdf9fde0f6d5c354995` as the translation migration target for TranslateIT, using CUDA BF16, the official Xiaomi translation prompt, and deterministic generation. MiLMMT-46-4B-v1.0 is comparison evidence only and is not an active migration candidate.

D-028 supersedes D-026 and D-027 as the active model/runtime-selection direction. D-026/D-027 remain historical records of the earlier LMT evaluation phase; they do not reopen LMT or its runtime profile.

Production remains unchanged until MiLMMT-1B integration and target end-to-end proof pass. If adopted, MiLMMT-1B replaces the canonical production translator rather than becoming a router/fallback beside M2M100.

Before migration, one bounded same-model latency-optimization pass is allowed. It must preserve the same checkpoint, BF16 precision, official prompt, deterministic translation semantics, and no output repair. Execution-only variants may be adopted only when target-Windows evidence shows repeatable useful latency improvement without translation-quality regression. Exact output equality is preferred; any changed output requires semantic/factual/naturalness review before acceptance.

**Reason**  
The representative 24-case Meeting/Text evaluation completed all cases for MiLMMT-1B and MiLMMT-4B. The 4B model showed stronger aggregate quality, but the clean RTX 3070 rerun measured approximately 4516 ms p50 / 7036 ms p90 and ~4826 MiB framework allocation for 4B INT8, versus approximately 689 ms p50 / 1139 ms p90 and ~1907 MiB framework allocation for 1B BF16. The user explicitly selected MiLMMT-1B for the realtime product boundary after this clean comparison.

**Boundary**  
Do not introduce another translator automatically, reopen MiLMMT-4B as the active candidate, quantize the selected 1B model merely for speed, add phrase-specific repair, or create multiple permanent translation runtime profiles. If the bounded optimization pass yields no worthwhile safe gain, keep the clean MiLMMT-1B BF16 baseline and proceed to canonical integration and end-to-end ASR â†’ translation â†’ GPT-SoVITS proof.

## D-029 â€” MiLMMT-1B Uses Default SDPA + Default Cache; Latency Tuning Is Closed

**Decision**  
Use the selected MiLMMT-46-1B-v1.0 revision in BF16 with the normal PyTorch/Transformers execution path, native SDPA attention, the default/dynamic generation cache, a persistent resident model, and no benchmark-only `nvidia-smi` or explicit synchronization instrumentation in the normal per-request production hot path.

The bounded same-model latency experiment is closed. Do not adopt StaticCache, `torch.compile`, quantization, alternate inference backends, speculative decoding, or another model merely to chase additional latency reduction without a new explicit decision and new evidence.

The reviewed optimization report is:

```text
UserData/CacheData/TranslationQuality/MiLMMTRealtimeAB/milmmt_1b_latency_optimization_report.json
```

Observed production-like result:

```text
p50  674.51 ms
p90  1133.81 ms
24/24 authoritative outputs exact-match
attention backend = SDPA
```

This was only approximately 2.1% faster at p50 and 0.5% faster at p90 than the prior clean authority (`689.11 / 1139.34 ms`), so the gain is useful confirmation but not a reason to add runtime complexity.

StaticCache without compile measured approximately `723.97 ms p50 / 1169.84 ms p90`, was slower, and changed the deterministic output for `meeting.en_id.12.scope`. The dependent compile variant was therefore not pursued.

**Reason**  
The selected 1B model already uses SDPA under the tested PyTorch/Transformers runtime. The only simple safe gain was removing benchmark-only hot-path instrumentation. StaticCache failed both the speed and preferred exact-output-equivalence goals, so continuing cache/compile tuning would add complexity without product value.

**Boundary**  
The next translation task is not another optimization experiment. Before production migration, prove the selected MiLMMT-1B checkpoint inside the current frozen canonical WorkerRuntime because the validated evaluation environment used Transformers 4.57.6 while the canonical WorkerRuntime currently constrains Transformers to `>=4.44.0, <=4.50.0`. Do not modify the dependency lock speculatively. First run the repository-owned WorkerRuntime compatibility proof; if compatible, preserve the lock and proceed to canonical worker/model-manifest migration. If incompatible, diagnose the exact dependency gap and change only that boundary.

## D-030 â€” MiLMMT WorkerRuntime Convergence Is Canonical

**Decision**  
The compatibility precondition recorded at the end of D-029 is satisfied and superseded. The canonical production translation path is `xiaomi-research/MiLMMT-46-1B-v1.0` at revision `4fc480b6c58dec29c159dcdf9fde0f6d5c354995`, integrated through the single canonical WorkerRuntime. The reviewed WorkerRuntime dependency authority is Python 3.12.x + Torch 2.11.0/cu126 + Transformers 4.57.6 + Tokenizers 0.22.2.

Accelerate 1.14.0 remains part of the locked runtime graph and must survive R3 payload optimization because the canonical CUDA model-loading path uses Transformers `device_map`. M2M100, Marian, LMT, older Transformers constraints, and retired helper/compatibility paths are not active production alternatives; they may remain only as historical evidence or explicit negative guards.

**Reason**  
Repository-owned MiLMMT validation, canonical source integration, dependency convergence, and release-contract checks now agree on the same model revision and dependency boundary. Preserving Accelerate in the optimized offline runtime closes the packaging mismatch between the canonical CUDA load path and the R3 payload.

**Boundary**  
D-030 establishes repository-side canonical integration and packaging intent only. It does not prove the final Windows Setup artifact, installed private runtime, NVIDIA CUDA/BF16 practicality, microphone/VB-CABLE behavior, Meeting delivery, or clean-machine acceptance. Those require the corresponding Windows R3 artifact proof and later target-PC validation.

## D-031 - All-in-One R3 Acceptance Gate Retired for Granular Scenario Suite

**Decision**
Root `Run-Local-Test.ps1`, `scripts/run_local_test.ps1`, and `scripts/run_target_pc_acceptance.ps1` are removed. Acceptance is scenario-based per `docs/foundation/03-acceptance-scenarios.md`, executed one scenario at a time in criticality order (A core AI, B audio, C meeting, D desktop). Installer/distribution checks are demoted to deferred opt-in Group E scenarios that use `build_release.ps1` directly when distribution readiness is decided.

**Reason**
The single mega-gate mixed release building, installation, and roughly twelve independent claims into one opaque pass/fail. Two early local attempts failed in unrelated boundaries (missing staged inputs; PowerShell 5.1 encoding) before any product claim could be exercised, demonstrating poor scenario ownership and expensive feedback. Granular scenarios give each claim one owner, one precondition set, and one evidence artifact.

**Boundary**
This retires an acceptance harness, not the approved offline release shape: `TranslateIT-Setup.exe` + `TranslateIT-Payload.7z` remain the release representation and Group E still requires them. Existing green results (static validator suite, cargo tests, installer-free worker smoke on CUDA BF16) remain valid evidence for exactly the claims they tested.

## D-032 - Test Suite Rebalanced to Function-First

**Decision**
Automated coverage is reorganized around product function, per owner direction: (1) keep and extend GPU worker smoke plus pytest contract tests as the Level-0 function proof; (2) add pure-decision Rust units for the audio engine (VAD gate thresholds/hysteresis reasons, utterance overflow and oldest-eviction counters) reaching 53 cargo tests; (3) replace the two prose-marker validators (validate_startup_runtime_readiness.mjs, validate_frontend_build_preflight.mjs, ~830 lines of shape checks that had already rotted once) with a single functional command-parity gate proving registry.rs and the frontend bridge stay 1:1; (4) Meeting orchestration with devices stays a manual Group C scenario instead of forced mock plumbing.

**Reason**
Owner feedback: tests must describe application functions, stay few but efficient, and avoid overdevelopment. Marker validators answered "does the code still look like this" rather than "does the product behave", produced false confidence, and required repeated repair. Real inference tests and pure decision units answer function questions directly at far lower maintenance cost.

**Boundary**
This rebalance does not add device automation: microphone capture, VAD on live input, virtual-route delivery, and full Meeting flow remain manual scenarios B/C/D in docs/foundation/03-acceptance-scenarios.md. The gpu-probe pytest environment gap on CUDA hosts remains a separate known issue.

## D-033 - Meeting Context Asymmetry and Incoming Deferral Solutions

**Decision**
Two owner-approved designs close the last open ambiguities. (1) Outbound translation (the user's own voice) now carries a rolling context of the last three committed {Indonesian -> English} pairs from the same session, injected inside the official flat Xiaomi prompt format by repeating the language-labelled pair lines; the incoming lane stays permanently context-free because meeting audio has multiple speakers and cross-speaker gender bleed would harm quality. Measured cost on the target GPU: median generate() time 472.2 ms without context vs 475.6 ms with three pairs (+3.4 ms, p95 unchanged). (2) While required outbound work holds worker priority, finalized incoming segments are no longer discarded: they enter a bounded deferred queue (max 4 jobs, max age 20 s), drain FIFO once outbound is idle, and every overflow/expiry drop increments a visible counter instead of disappearing silently.

**Reason**
Owner required final solutions, not deferrals, for pronoun/persona ambiguity (#1) and silent incoming loss (#4). The asymmetric-context design keeps multi-speaker hazard out while proving benefit on the exact ambiguity class (dia -> he/she) in a live A/B benchmark on the staged runtime.

**Boundary**
Both solutions are dev-tree implemented with unit/contract coverage (cargo 54, pytest 36) plus the recorded latency benchmark. Live-meeting behavior, perceived quality, and end-to-end latency remain manual Group C observations whenever the owner schedules that session.

## D-034 - Product Direction Locks From Owner Review

**Decision**
Owner locked eight product directions. Built-in voices: ship two ready-to-use GPT-SoVITS voice packs (one male, one female) so Meeting works on day one without My Voice training; My Voice becomes an optional replacement selected by the user. Modes: collapse Realtime/Quality into a single canonical pipeline tuned for best quality at the lowest achievable latency; remove mode vocabulary from the product. Documents: document translation is removed entirely; Text stays manual paste-only. Languages: Indonesian<->English only, no multilingual roadmap. UI language: full English copy everywhere (current mixed Indonesian strings get swept). Usage: personal use, so code signing/auto-update stay irrelevant and Group E distribution shrinks to basic install sanity on owned machines.

**Reason**
Owner answered every open ambiguity directly during the post-audit review; recording them prevents drift and turns them into testable product law.

**Boundary**
Foundation documents and UI copy are not yet updated to mirror these locks; the built-in voice packs require a licensed reference-asset source and their own development brief. Until those land, current source remains the behavior truth where it already matches, and the locks govern all NEW work.

## D-035 - Built-in Voice Reference Assets Staged

**Decision**
Two public-domain reference samples for the future built-in GPT-SoVITS zero-shot voices are staged under EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/: MaleVoice (LibriSpeech speaker 3752, utterance 3752-4943-0003, 6.3 s) and FemaleVoice (speaker 6313, utterance 6313-66125-0007, 4.9 s), both 32 kHz mono WAV with per-file SHA-256 pins and exact spoken-text provenance recorded in SOURCES.json + REFERENCE_SOURCE.txt. Source corpus: LibriSpeech dev-clean (OpenSLR SLR12, CC-BY-4.0), selected objectively as the most prolific male/female readers (professional audiobook delivery, General American accent).

**Reason**
Owner approved option B (public-domain source) requiring easy-to-understand neutral-accent voices for the built-in male/female day-one Meeting voices locked in D-034.

**Boundary**
These are staged reference assets only: worker zero-shot support, Settings voice selection UI, readiness mapping, and staging/payload wiring for the built-in packs are a separate development-brief slice. Loudness (mean -21/-26 dB) may need normalization during that slice's GPU tuning.

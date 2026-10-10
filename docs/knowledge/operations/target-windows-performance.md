---
id: document.knowledge.operations.target-windows-performance
class: DOCUMENT
domain: knowledge
role: DOMAIN
authority: CANONICAL
lifecycle: ACTIVE
---

# Target Windows Performance Acceptance

Use this runbook once a **source-integrated candidate** has passed GitHub source/contract review and its locked build/package inputs are prepared.

The existing Settings → Diagnostics → Check Again action refreshes the worker/Meeting snapshot **and** the Runtime Health evidence panel on explicit user action; it is not a new monitoring daemon. Meeting performance reports the largest measured stage only after actual first playback timing exists; this is a candidate for investigation, not a proven root cause. The redacted support bundle includes numeric stage timings but no per-utterance timestamps, audio, transcripts, speaker references or session IDs. Collect the exact source SHA alongside any exported evidence. An unverified status must not be interpreted as Healthy/Clean.

This runbook groups the first actual local build, package, install and TARGET_WINDOWS acceptance into **one planned phase**, never per-change PC testing.

## Purpose

Measure the real installed/local Meeting path before changing safety, model residency, or transport architecture:

```text
physical microphone
→ finalized utterance
→ ASR
→ Indonesian → English translation
→ selected Meeting voice
→ TranslateIT Meeting Microphone
→ meeting application
```

The baseline must distinguish speech-boundary delay, CPU/audio preparation, AI stages, output delivery, and hardware pressure. Do not infer any target result from CI/source presence.

## Candidate admission — before any user-PC test request

This is a **read-only admission review**, not proof of native behavior or release approval. Require all of the following before inviting a single planned Windows acceptance session:

1. **Integrated scope:** the intended Meeting (outbound and optional incoming), Text, selected built-in/My Voice behavior, Settings, and application lifecycle paths are implemented and reconciled against `docs/foundation/03-acceptance-scenarios.md`. Do not call an isolated feature or partial source patch an install-ready product.
2. **Source/integration admission:** resolve known static/source-contract blockers on one pinned `Local` SHA. Cloud builds/CI are not used. Record compiler and live integration checks as `NOT EXECUTED` before the grouped local build stage; do not mislabel them as PASS.
3. **Build/package candidate:** prepare controlled Setup/payload sources, dependency locks, model/voice assets, notices and signing requirements. A working installer is **not** yet claimed. At the start of the integrated local session build/package once; only continue to installation after that step succeeds. Publishing/promoting remains separately authorized.
4. **One bounded plan:** pin source SHA and record the local toolchain/build commands, package inputs, native-only questions, safe diagnostics and STOP criteria. Run compiler/build checks first; on failure STOP and report source evidence. Only after build/package PASS proceed to device/Meeting/latency and existing acceptance scenarios.

If source admission is not met, continue GitHub repository work and keep execution `UNKNOWN / NOT EXECUTED`. No hosted fallback and no repeated local requests. Actual Windows build/package/device acceptance is required before declaring a finished application.

## One integrated candidate execution sequence

This is an **execution plan**, not a request to use the user's PC now. Keep it in the existing runbook; do not create a separate release harness or hosted job. Follow `docs/foundation/03-acceptance-scenarios.md` as the sole scenario authority.

1. **Pin one immutable candidate.** Record the 40-character `Local` SHA and require a clean tracked checkout. Set `TRANSLATEIT_EXPECTED_SHA` to that externally pinned SHA **before** invoking the canonical local Tauri compile check; do not derive the expected value from whatever happens to be checked out. Record the Node 22 / Rust 1.88+ / Python 3.12.10 / uv 0.12+ toolchain identities and confirm `package-lock.json`, `Cargo.lock`, and `uv.lock` are unchanged. On mismatch, STOP.
2. **Source-level admission first.** From `EngineData/Frontend/RustApp`, use `npm ci`, `npm run validate:source-contracts`, `npm run typecheck`, and `npm run test:frontend-runtime` against that same SHA. Using the locked Python worker environment, run its existing pytest suite and the existing translation, ASR, and TTS quality evaluator self-tests; verify no corpus/model identity substitution or generated tracked-file edits. Source tests do not constitute model-quality or device proof. On first failure, STOP.
3. **One consistent Rust/frontend build.** Run `npm run check:tauri-rust-local` with the pinned `TRANSLATEIT_EXPECTED_SHA`. Its existing helper must compile a fresh frontend rather than accept a stale `dist`, check a clean Git source identity, and use `cargo check --locked`. Record executable result; STOP if either language fails.
4. **Model and product proof before distribution.** On the same installed/staged canonical model revisions, exercise A1/A3/A4/A5/A6/A7 plus D1/D2/D3/D4: Text in both directions, incomplete/over-length responses, natural/formal terminology, Quick Translate and alternate wording, persisted settings, and redacted Diagnostics. Capture matched real ASR and MiLMMT outputs for the existing quality evaluators. Never label an unexecuted or fixture-only quality check as an acoustic/linguistic PASS.
5. **Controlled Setup + Payload only when admitted.** The existing `scripts/build_release.ps1` requires actual `-QualityReadinessEvidence`, a staged private Python/voice/model/driver payload, and updater signing credentials. This is a **hard release gate**, not optional decoration. Do not create fake quality receipts, bypass signing, use another release script or claim an installer if those inputs are missing. Build the existing exact-SHA offline R3 pair only after prerequisites pass; publishing remains separately authorized. STOP on missing provenance, license, artifact hash, driver prerequisite, or signature.
6. **One installed Windows acceptance session.** Only with the built artifact and actual target device, carry out B1–B5 and C0–C5 with outbound built-in voice first, then optional incoming and A9 My Voice as scope requires. Include D5/E3–E6, device conflict/Stop/restart behavior, the recorded stage timings and hardware pressure below. STOP at the first reproducible blocker; report its owning stage and preserve prior working behavior instead of adjusting multiple engines.

Pass is **revision-scoped**: source, model, quality receipt, Setup/payload hashes, installed runtime and native measurements must refer to the same admitted candidate. A passing source check is not a passing installed application. This plan adds no normal user-PC testing steps to the ChatGPT/GitHub development loop.

## Preconditions

- Use the exact `Local` SHA being evaluated and record it before starting.
- Start with a built-in Meeting voice so My Voice training is not a prerequisite.
- Use the actual physical microphone intended for normal use.
- Configure the exact TranslateIT Meeting Microphone / supported virtual route used by the meeting application.
- Keep optional incoming Meeting Sound **off for the first outbound baseline**.
- Close unrelated GPU/CPU-heavy applications where practical, but keep the actual meeting application running for the end-to-end pass.
- Do not tune VAD, remove safety checks, unload models, or replace WAV transport before the first baseline is recorded.

## Record the environment

Record these once with the baseline:

| Item | Value |
|---|---|
| `Local` SHA | |
| Windows version/build | |
| CPU | |
| Physical RAM | |
| GPU | |
| Dedicated VRAM | |
| GPU driver | |
| Microphone + source format | |
| Meeting application | |
| Meeting microphone endpoint | |
| Selected TranslateIT voice | |
| Incoming Meeting Sound enabled? | No for baseline |

Do not record private conversation text, raw recordings, credentials, or unredacted private paths as evidence.

## Test order

### 1. A2 / A3 — acceleration and ASR truth

Open Diagnostics and confirm the executed runtime reports the actual selected device/precision. Run the normal required readiness path and confirm ASR loads and processes a valid finalized input.

Record:

- selected ASR device and compute type;
- selected translation device and precision;
- whether CPU fallback is active;
- any CUDA/runtime blocker.

A file existing or CUDA being installed is not execution proof.

### 2. B1 / B2 / B5 — microphone, finalization and route

Verify the selected/default microphone functionally produces capture frames. Start Session Listening and speak several short, normal Indonesian sentences with natural pauses.

Confirm:

- capture frames continue arriving;
- natural pauses produce finalized utterances;
- include a few natural one- or two-word replies alongside ordinary sentences, without assuming speaking duration from the number of words;
- on any lost reply, distinguish a VAD non-finalization, a finalized frame rejected by the writer, and a successfully written WAV with an empty or failed ASR result. Record the existing blocker and stage, not the private audio or spoken content;
- no repeated callback error appears;
- `overflow_dropped_utterance_count` and `evicted_pending_utterance_count` stay at zero under normal speaking pace;
- the exact Meeting Microphone route remains ready and selectable by the meeting application.

### 3. C0 — first Start with built-in voice

From a stopped state, press **Start Translation** once and measure wall-clock **Start → Live**. Do not double-click Start.

Record whether Start is cold (models not yet resident) or warm (same app process after successful runtime use). Repeat one warm Start only after a clean Stop so cold/warm behavior is distinguishable.

### 4. C1 — outbound end-to-end timing

With Translation Live, speak 10–20 representative utterances. Include short phrases and normal meeting-length sentences; do not use an artificial stress script as the only sample.

Use the current Diagnostics timing surfaced from Meeting outbound status and record at least several representative turns:

| Timing | Meaning | Result |
|---|---|---|
| `speech_boundary_ms` | detected end-silence contribution | |
| `finalization_ms` | finalized speech → queued target frame | |
| `queue_ms` | wait before serialized outbound processing | |
| `audio_prepare_ms` | temporary ASR audio preparation | |
| `asr_ms` | finalized audio → transcript | |
| `translation_ms` | transcript → completed translation | |
| `tts_ms` | translated text → generated voice WAV | |
| `delivery_ms` | output delivery start → first playback | |
| `outbound_latency_ms` | finalized utterance → first translated playback | |

Also record:

- callback error count;
- overflow-dropped utterance count;
- evicted-pending utterance count;
- any failed/stale/interrupted turn;
- whether the meeting application actually receives the translated voice.

The official user-relevant latency metric remains `outbound_latency_ms`; stage timing exists to identify its owner.

**Short-utterance admission decision (same B2/C1 session, not another test cycle):** the producer's VAD duration gate and the downstream WAV writer's frame-duration gate are independent today. The producer computes accepted speech duration separately from the retained frame (which can contain pre-roll and trailing silence). If a natural brief reply is lost, use the existing `finalized_utterance_writer:segment_too_short` blocker to identify an already-finalized utterance vetoed at the writer, rather than mislabeling it a VAD miss. If WAV admission succeeds but no transcript emerges, assign the issue to ASR instead. Record any unavailable per-stage evidence as `UNKNOWN`; do not infer model quality or fabricate sample timing. Preserve the current thresholds until a matching baseline distinguishes deliberate short-audio rejection from unintended speech loss.

### 5. Hardware pressure

Observe the TranslateIT/Rust process and its Python worker during these phases:

```text
app idle
→ Start / functional readiness
→ Live listening with no speech
→ ASR
→ translation
→ TTS
→ delivery
```

Use Windows Task Manager and, when available for NVIDIA, `nvidia-smi` or an equivalent trusted local monitor. Capture **steady and peak** values rather than one averaged number:

| Phase | Rust CPU | Python CPU | Rust RAM | Python RAM | GPU util | VRAM |
|---|---:|---:|---:|---:|---:|---:|
| Idle | | | | | | |
| Start | | | | | | |
| Live listening | | | | | | |
| ASR | | | | | | |
| Translation | | | | | | |
| TTS | | | | | | |
| Delivery | | | | | | |

### 6. Optional incoming isolation

After the outbound-only baseline is healthy, enable the intended Meeting Sound source and repeat several outbound utterances while English incoming speech is present.

Confirm:

- required outbound remains responsive and takes scheduler priority;
- incoming may defer/degrade without breaking outbound;
- TranslateIT's own English TTS is not re-consumed as incoming translation;
- stale incoming backlog does not grow unbounded.

### 7. C4 — Stop lifecycle

Press Stop while idle, then once while an utterance is being processed.

Confirm output authority is revoked immediately, no stale translated audio is emitted afterward, microphone/Meeting Sound resources release, and the app reaches a truthful stopped state.

### 8. C5 — repeated-session stability

Run at least five normal Start → speak → Stop cycles in the same app process. Record each Start → Live duration and whether the Python worker stayed warm or had to restart after an in-flight hard cancellation.

A repeated-session issue is actionable only when its owner is visible: Start preflight, helper restart/cold load, audio-route opening, capture opening, or cleanup.

### 9. A9 — My Voice quality and approval (integrated, optional to first built-in baseline)

Run this only on a controlled installed Windows candidate when My Voice is in the acceptance scope. Keep the physical speaker, microphone, input format, environment and exact `Local` SHA recorded. Use authorized guided English recordings only. Quick Preview is a pretrained **early listening aid**, not a trained actor or Meeting voice.

1. **Reference and Quick Preview:** accept at least one usable 3–10 second guided take; generate and listen to one temporary preview. Confirm stop/cancel during preparation, rejection of unusable takes, no unexpected Meeting selection, and no leftover partial WAV after failure. If it cannot synthesize or clean up safely, STOP A9; do not claim preview quality PASS from a button or file alone.
2. **Trained-actor cycle:** collect the existing minimum accepted speech and required recording coverage, build the bounded GPT-SoVITS candidates, and inspect the selected candidate's existing eight held-out outputs (IDs 1001–1008). Confirm no gross `artifact_flags`, review intelligibility transcripts/WER and speaker-similarity evidence, and **listen to all eight** before deciding whether to approve.
3. **Perceptual review:** for those same eight English phrases, note pronunciation of names, dates, numbers and negation; voice identity; clipping, dropout, unexpected silence; and stability across short and long utterances. Record explicit `acceptable / needs work` judgments with concrete observations, not a fabricated numerical quality threshold or model-confidence claim.
4. **Controlled comparison:** compare two trained candidates, builds, or versions **only when their output sentences, source-speaker identity, and recorded conditions are comparable**. The one-sentence pretrained Quick Preview and eight held-out trained samples differ in text, so they are not a valid direct before/after quality score. When there is no matching earlier audio/evidence, record `no comparative baseline` and do not claim a Voicebox-inspired acoustic improvement.
5. **Meeting and persistence:** explicitly approve only after the user's listening review; confirm the selected My Voice works through the actual Meeting Microphone, survives app restart, and is still selected. A failed/rejected new build must not replace the previously approved Meeting voice. Check Stop and resource conflicts between Meeting and training.

Existing Rust evaluation and Python candidate evidence are the measurement owners; do not add a second benchmark database, export private recordings to GitHub, or ship a new evaluation model. Retain detailed private samples/evidence only in the controlled acceptance environment; record only non-sensitive observations and outcome. If A9 fails, report the first reproducible owner (recording, Quick Preview, dataset, training, selection, approval or Meeting playback) and STOP further voice optimization until diagnosed.

## Decision rules after the baseline

Use the measured owner instead of applying speculative architecture changes:

- **Start functional probe:** optimize/reuse it only if Start → Live is materially dominated by repeated functional verification on an otherwise unchanged healthy worker. Any reuse must remain bound to worker generation, selected actor identity, and current prerequisites; do not replace functional truth with file presence.
- **Model residency:** keep ASR/MiLMMT/GPT-SoVITS warm when RAM/VRAM pressure is acceptable. Consider selective residency only if measured pressure is the practical limiter; do not trade normal per-utterance latency for theoretical memory savings.
- **Temporary WAV transport:** keep the simpler file boundary when `audio_prepare_ms`/delivery preparation is negligible relative to AI stages. Consider memory/shared transport only when the measured file boundary is material and the replacement can preserve cancellation, privacy, and bounded memory.
- **Audio callback/VAD:** revisit callback/buffer work when callback errors, capture instability, abnormal CPU while only listening, or drop/eviction counters reproduce independently of heavy AI inference.
- **Polling/UI:** optimize only if profiling shows UI/IPC is a measurable contributor; it is not a proxy for Meeting latency.

## Completion

This run is complete when one exact SHA has:

1. real microphone/finalization/route evidence;
2. real outbound stage timing;
3. real CPU/RAM/GPU/VRAM observations;
4. one clean Stop and repeated-session evidence;
5. a named first bottleneck, or evidence that no additional source optimization is currently justified.

Return only the measured first bottleneck to development. Do not reopen every previously considered optimization at once.

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

This runbook groups source build and TARGET_WINDOWS acceptance into **one planned phase**, never per-change PC testing. Developer-mode functional acceptance can precede release packaging: `DEV.cmd run` starts the existing local Tauri app without release signing, but still requires real local AI/runtime assets and devices to prove Meeting behavior. **Signed package/install acceptance is a separate conditional E2–E6 boundary**, not a prerequisite for a developer UI/Meeting baseline. No developer run is evidence of installed-product behavior.

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
3. **Developer build candidate:** ensure locked toolchains, local worker/model readiness inputs, and an exact clean `Local` source identity are prepared. A development app is not an installed release. **Only if release/install claims (E2–E6) are in scope**, separately prepare controlled Setup/payload sources, notices and signing requirements. Never treat signing material as a prerequisite merely to inspect the developer UI.
4. **One bounded plan:** pin source SHA and record the local toolchain/build commands, required development model/device inputs, native-only questions, safe diagnostics and STOP criteria. Run compiler/build checks first; on failure STOP and report source evidence. After source build PASS, native developer-mode functional tests may proceed; package/install tests proceed only after a genuine signed package PASS.

If source admission is not met, continue GitHub repository work and keep execution `UNKNOWN / NOT EXECUTED`. No hosted fallback and no repeated local requests. Actual Windows runtime behavior is required for functional PASS; genuine controlled package/install evidence is additionally required before declaring an installed release finished.

## U4 — integrated desktop UI/UX acceptance (U1–U3)

**Execution context:** source inspection may establish only `REMOTE_GITHUB` admission; this section records no actual run results. Conduct the UI checks **within** the single planned locked-SHA Windows session below, after its existing compilation/source admission succeeds. Do not dispatch hosted CI, create a second UI verification framework, or ask for incremental user-PC tests. The existing [acceptance scenarios](../../foundation/03-acceptance-scenarios.md) remain authoritative for D1–D5, A9, B1/B5 and Meeting C0/C4/C5. Keep Q4 packaging separately gated by the existing candidate admission decisions.

| Area and existing scenario | Exact interaction to observe | Failure/STOP signal |
| --- | --- | --- |
| U1 Text · D5 | Translate Indonesian→English and English→Indonesian; copy Translation/Source/Bilingual; open and close More options; Clear and Undo Clear; edit draft, navigate away/back, and change settings during a pending translation | Any lost draft without explicit action, stale result presented as current, missing secondary action, clipped/focus-inaccessible Copy or Translate |
| U2 First Setup · D1/D3/D5 | Change microphone without saving: Continue stays blocked; save/check new selection; revisit an unavailable device; verify 4-step resume and deferred setup after relaunch | Continue enabled for an unsaved/unverified selected microphone, silent device fallback, checkpoint loss, unavailable recovery |
| U2 Meeting · D3/B5/C0/C4/D9/D10 | At Ready one Start action; active Live shows chronological You/right and Person 1/left with speaker unverified, and only Stop; verify accurate status and navigation to Settings → Voice & Audio; open browser Meet with separated receiver, confirm incoming captions do not overclaim speaker, after Stop manually export a unique per-session folder; inspect short/long transcript and scroll-follow. No presets, Quick Translate or Text captions. | False readiness, hidden Stop, unexpected route/device change, unverified speaker identity, silent dropped speech, transcript auto-save, old session merge, or user-private data in diagnostics |
| U3 built-in and My Voice · A9/D5 | Preview Built-in Male/Female including missing-audio recovery; select and restore an approved personal voice after using a built-in; in controlled disposable test data verify one valid interrupted active-actor backup can be recovered by the next selection, while invalid, multiple or ambiguous backups remain untouched and blocked; record, Replay/Retry/Accept, and retain samples. Simulate an unavailable bridge response during Start/Stop/get recording status: no synthetic idle, no navigation unlock until a verified owner is clear, and Retry status/Retry Stop remains accessible. Expand/collapse Quick Preview, cancel when active, listen to every evaluation sample and explicitly approve before use; while Meeting is Live, verify approval is disabled and native approval rejects a direct attempt without changing the current actor, then Stop Meeting and retry approval | Missing voice restoration, preview button locked, a failed bridge call presented as confirmed stopped, active audio owner bypassed by navigation, inaccessible recovery, lost sample, approval bypass or unintended switch |
| U3 Floating Caption · D4/D5 | Change size/width/contrast in Settings; compare public-sample preview to actual overlay at 100%, 125%, 150%, 200% display scaling and multi-monitor changes; Hide/Show, click-through escape, keyboard/focus and long captions | Private text restored on Show/Hide, overlay inaccessible/off-screen, text clipped without recovery, incorrect claim that preview proves native layout |
| U3 Translation Settings · D1/D2/D5 | Expand terminology/feedback; add/remove final term while disclosure stays open by user choice; navigate away/back, verify saved entries; confirm no feedback export/clear without user action | Disclosure unexpectedly collapses on term-list updates, lost term, unintended export/clear, private feedback leakage |
| Cross-page accessibility · D5 | Keyboard-only tab order, labelled controls, screen-reader announcements, reduced motion/high contrast, disabled/loading/error states at supported 1280×760 minimum and high-DPI effective viewport | Unreachable critical action, trapped focus, unreadable warning, lack of usable scrolling, click-through without recovery |

**Evidence & safety:** record the pinned 40-character SHA, toolchain/build results, Windows/display dimensions and DPI, scenario ID, pass/fail/unknown, and a reproducible non-private symptom. Do not record or commit user speech, microphone recordings, private translated text, approved My Voice assets or unredacted user paths. A screenshot with personal data must not enter GitHub. An `UNKNOWN` for an unobservable application setting remains unknown; do not infer it from Tauri device presence.

**Stop rule:** if the pinned source fails typechecking, frontend tests, bundle contract or Tauri/Rust compilation, STOP before native UI evidence. If native UI behavior fails, capture the **first wrong owner**, do not bypass readiness/privacy checks or weaken the gate. No PASS or completion claim until actual executable/rendered/Windows evidence is recorded.

## One integrated candidate execution sequence

This is an **execution plan**, not a request to use the user's PC now. Keep it in the existing runbook; do not create a separate release harness or hosted job. Follow `docs/foundation/03-acceptance-scenarios.md` as the sole scenario authority.

1. **Pin one immutable candidate.** Record the 40-character `Local` SHA and require a clean tracked checkout. Set `TRANSLATEIT_EXPECTED_SHA` to that externally pinned SHA **before** invoking the canonical local Tauri compile check; do not derive the expected value from whatever happens to be checked out. Record the Node 22 / Rust 1.88+ / Python 3.12.10 / uv 0.12+ toolchain identities and confirm `package-lock.json`, `Cargo.lock`, and `uv.lock` are unchanged. On mismatch, STOP.
2. **Source-level admission first.** From `EngineData/Frontend/RustApp`, use `npm ci`, `npm run validate:source-contracts`, `npm run typecheck`, and `npm run test:frontend-runtime` against that same SHA. Using the locked Python worker environment, run its existing pytest suite and the existing translation, ASR, and TTS quality evaluator self-tests; verify no corpus/model identity substitution or generated tracked-file edits. Source tests do not constitute model-quality or device proof. On first failure, STOP.
3. **One consistent Rust/frontend build.** Run the existing `DEV.cmd build` from the repository root (or its canonical `npm run check:tauri-rust-local` command) with `TRANSLATEIT_EXPECTED_SHA` set to the externally pinned SHA. Its existing helper rebuilds the frontend, checks clean source identity, and runs `cargo check --locked`. Record the executable result; STOP if either language fails.
4. **One developer-mode local functional baseline before distribution.** With real local worker/models and audio already available, launch the existing Tauri development runtime via `DEV.cmd run` after the source build gate. Exercise A1/A3/A4/A5/A6/A7, D1/D2/D3/D4 and the applicable B/C scenarios in this one Windows session: Text in both directions, incomplete/over-length responses, persisted audio settings, redacted Diagnostics, microphone capture, real Meet receiver delivery, incoming speech, manual per-session transcript export and repeated Start/Stop. Do not reinstate retired terminology, style, Meeting Presets, alternative wording or Quick Translate. Capture matched real ASR and MiLMMT outputs for the existing quality evaluators. Missing model/device evidence remains `UNKNOWN`; developer-mode PASS is never installed-app PASS.
5. **Controlled Setup + Payload only for release/install scope.** The existing `DEV.cmd package "<quality-readiness-report.json>"` delegates to `scripts/build_release.ps1`, which requires genuine `-QualityReadinessEvidence`, a staged private Python/voice/model/driver payload, and updater signing credentials. This is a **hard release gate**, not an optional decoration. Do not create fake quality receipts, bypass signing, use another release script or claim an installer if those inputs are missing. Build the existing exact-SHA offline R3 pair only when the requested release/install scope and prerequisites justify it; publishing remains separately authorized. STOP on missing provenance, license, artifact hash, driver prerequisite, or signature.
6. **Installed Windows acceptance only when release scope is admitted.** Only with a real signed build artifact and actual target device, repeat the necessary B/C/D checks through the **installed** app and carry out E3–E6. The earlier developer-mode evidence can diagnose failures, but cannot replace installed voice, driver, update, data-preservation or clean-machine proof. STOP at the first reproducible blocker; report its owning stage and preserve prior working behavior instead of adjusting multiple engines.

Pass is **revision-scoped**: source and model identity must match the observed developer-mode run; for release claims, quality receipt, Setup/payload hashes, installed runtime and native measurements must additionally match the admitted release. A passing source check or Tauri dev run is not a passing installed application. This plan adds no normal user-PC testing steps to the ChatGPT/GitHub development loop.

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
| `outbound_latency_ms` | finalized utterance → first local translated playback | |
| `estimated_onset_to_first_audio_ms` | estimated onset → first local audio (VAD buffered span may include pre-roll) | |
| `estimated_end_to_first_audio_ms` | estimated speech end → first local audio (includes trailing VAD silence) | |
| `asr_scheduler_wait_ms` / `translation_scheduler_wait_ms` / `tts_scheduler_wait_ms` | serialized helper admission delay for each successful request | |
| `output_stream_build_ms` | native CPAL stream construction, not full output preparation | |
| `output_stream_start_to_first_audio_ms` | stream start attempt → estimated first-audio callback timing | |

**Interpretation:** ASR / translation / TTS stage totals already include their scheduler admission wait. Never add the scheduler waits again to a total. The new output-stream build timing isolates one potential source of technical latency; it does not prove a persistent stream would be faster.

Also record:

- callback error count;
- overflow-dropped utterance count;
- evicted-pending utterance count;
- any failed/stale/interrupted turn;
- whether the meeting application actually receives the translated voice.
- whether `output_stream_build_ms` is nonzero and material relative to total delay; do not replace CPAL streams without an A/B result.
- whether input and output timing fields remain missing (rather than zero) when capture or playback fails.

**Latency boundaries:** `outbound_latency_ms` measures *utterance finalization → first native playback*, not time from speech onset and not audio receipt by browser participants. New `estimated_onset_to_first_audio_ms` and `estimated_end_to_first_audio_ms` are approximate because they use VAD speech-span/trailing-silence durations, including pre-roll; do not present them as precise microphone hardware timestamps. Source-level timings and browser reception still require matching Windows execution.

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

**Actual device for this phase:** one RTX 3070 (8 GB dedicated VRAM), Intel i7-14700-class CPU (20 cores / 28 threads), 32 GB RAM, no usable iGPU. Keep the actual calling browser (Meet or other) open during tests. Do not infer remaining VRAM from model parameter counts or PyTorch alone.

Use Windows Task Manager and NVIDIA `nvidia-smi` where supported. A voluntary, local-only 1-second GPU sampler can be run from a separate terminal while the meeting is active:

```powershell
nvidia-smi --query-gpu=timestamp,memory.total,memory.used,utilization.gpu,temperature.gpu --format=csv -l 1
```

Stop with Ctrl+C. Record only aggregate values in the private test receipt and include the GPU driver and exact Git SHA. Windows WDDM may limit process-specific accounting; cross-check Task Manager. This does **not** record meeting text, voice, speaker identity or a precise breakdown of CUDA allocator ownership.

Capture **steady and peak** values rather than one averaged number:

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
- stale incoming backlog does not grow unbounded;
- when incoming ASR/translation defers during outbound, the existing incoming consumer retries (or expires) the bounded work without requiring new incoming speech; record the observed result, not a synthetic PASS;
- if suppression fails or Stop occurs with a deferred item, no late completion refills the cleared queue or re-enables the disabled incoming lane; confirm the optional-lane failure never breaks required outbound.

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

## Q3 Session-latency distribution interpretation

An explicit redacted support export may include the numeric-only
`latency_distribution_ms` from the active Meeting's retained, completed outbound
turns (within the existing 240-turn cap). P50/P95 use nearest-rank selection
and always include sample count. Failed, interrupted, incoming, pending and
unmeasured turns are excluded. A null field means no eligible completed
samples, **not** zero latency. Small samples and a rolling retention window
are exploratory evidence, not a representative performance SLA.

No raw text, speech, speaker identity or per-turn timestamps are exported
for this distribution. During future TARGET_WINDOWS acceptance, associate
the sample window with the exact source/model/voice identity, drop/queue
counters, device route and resource pressure. Do not optimize from
source-only metrics or infer a successful native outcome.

## Completion

This run is complete when one exact SHA has:

1. real microphone/finalization/route evidence;
2. real outbound stage timing;
3. real CPU/RAM/GPU/VRAM observations;
4. one clean Stop and repeated-session evidence;
5. a named first bottleneck, or evidence that no additional source optimization is currently justified.

Return only the measured first bottleneck to development. Do not reopen every previously considered optimization at once.

## Audio endpoint ambiguity and capture-progress acceptance (2026-10-11 candidate)

This is a future TARGET_WINDOWS acceptance procedure, not a completed test receipt. It verifies the fail-closed endpoint-name selector and native-frame progress check introduced on Local.

1. With a uniquely named physical mic, Meeting Sound output and one installed VB-CABLE pair, verify explicit preferences, functional Mic Test, actual Zoom/Meet microphone delivery, Stop and repeated Start without raw-mic or physical-speaker fallback.
2. Reproduce duplicate-name Windows endpoints where hardware permits. Pinned capture, optional incoming and required translated playback must refuse ambiguous matches. If duplicate names cannot be reproduced, mark NOT EXECUTED, not PASS.
3. Keep the mic powered and silent for 30 seconds: `frames_received` must advance and the guard must not falsely flag a stall. If possible, simulate a native stream that stays nominally active but stops producing callback frames. Sustained no-progress across health observations must report `device_loss:microphone_callback_stalled` after 24 seconds.
4. Stop, new Meeting generation and new capture-start identity must reset liveness observations. Loss of Meeting Sound remains optional, without automatic outbound mic swap or TTS replay.
5. Include cases in the single exact-SHA integrated source/toolchain/package/long-session matrix, measuring handle, memory and dropout growth rather than treating unit contracts as native proof.

This revision does not supply Windows MMDevice endpoint IDs, process-specific loopback or persistent playback. Those require separate dependency/API and Windows evidence gates. Maintain the existing VB-CABLE policy and no silent audio fallback.

## Route isolation and name-collision parity — source candidate 2026-10-11

This section is an acceptance procedure, not target-Windows evidence. It covers new explicit separation between the spoken microphone, virtual TTS input/meeting recording output, and incoming meeting output loopback.

1. Select a uniquely named physical microphone as TranslateIT's spoken mic. Set the calling app's microphone INPUT to CABLE Output (VB-Audio Virtual Cable), and the calling app's speaker OUTPUT to the physical headset. TranslateIT Meeting Sound must select the same physical headset only for optional incoming captions. Verify raw Indonesian voice is not sent directly to the calling app.
2. Intentionally select CABLE Output as the TranslateIT spoken microphone, first pinned and then as Windows Default. Required Meeting Start must reject the feedback topology.
3. Intentionally select CABLE Input as optional Meeting Sound, first pinned and then as Windows Default. Incoming must degrade with an explicit blocker; translated outbound speech continues.
4. With two Windows endpoints sharing the same visible name, check that the Settings option is marked ambiguous and disabled; with duplicate virtual endpoints, Meeting route discovery must not claim ready. Repeat for duplicate physical microphone/output names as available.
5. Confirm normal physical speaker output and virtual microphone are distinct. Endpoint-wide WASAPI incoming may capture unrelated PC audio; do not claim process-specific isolation or actual Zoom/Meet reception from a source-only validation.

No OS or meeting application checks were executed here. Test the complete path, repeated session generations, physical device loss and long-session resources in the existing grouped exact-SHA Windows acceptance.

## Supported virtual provider / no silent fallback (2026-10-11 source candidate)

Confirm the native Windows audio-route discovery allows **only the standard** VB-CABLE playback/recording matched pair (CABLE Input / CABLE Output, VB-Audio Virtual Cable). Test a host with Voicemeeter, VB-CABLE A+B and/or other virtual cables **without the standard pair**: Meeting preflight must report missing virtual route rather than silently selecting those devices. With the standard pair installed alongside other providers, the standard pair alone must be selected. Keep packaging under the standard VB-CABLE license/provenance policy; do not add a second provider/driver. A functional CPAL output callback and actual meeting-app audio reception remain separate native checks, not consequences of static name matching.

# Next Action

## Continuation Handoff

This file is the canonical continuation note for a new chat/session.

**Authoritative branch:** `Local`  
**Current authoritative HEAD when this handoff was written:** `5d74ac11a09d8c5cdc7f71197f57f25ffac1b272`  
**Latest Code Health at this HEAD:** PASS

Before doing new work in a future chat:
1. Re-read this file.
2. Re-read `docs/knowledge/current-validation.md`, `docs/knowledge/development-discipline.md`, and `docs/knowledge/source-ownership.md`.
3. Re-check the actual `Local` HEAD and latest CI before assuming this SHA is still current.
4. Continue on `Local` only. Do not integrate to another branch unless explicitly requested.

---

## Product Direction

TranslateIT is a desktop-first local translation product built around:

- Tauri v2 + Rust
- Svelte 5 + Tailwind CSS 4
- local Faster-Whisper ASR
- local MiLMMT translation
- GPT-SoVITS / My Voice
- bounded Meeting audio lifecycle
- explicit fail-closed readiness
- low latency / low waste / no hidden background services

Primary product principle:

> real-time local translation + translated voice + readable captions

Do **not** broaden this into a generic meeting productivity suite.

Do not add:
- AI summaries
- action items
- meeting bots
- cloud transcript history
- calendar collaboration
- screen translation
- Auto Language Detection
- extra translation providers
- speculative cloud fallback

Global Quick Translate engine/API exists, but **no default keybind / global shortcut should be installed yet**.

---

## Completed High-End Feature Roadmap

The previously agreed 12-feature roadmap is implemented source-side:

1. **Spoken Terms / ASR Dictionary** — implemented
2. **Meeting Auto Detection** — implemented
3. **Recent Caption History** — implemented
4. **Pause Captions** — implemented
5. **Quick Translation Direction** — implemented
6. **Natural / Formal Translation Style** — implemented
7. **Audio Quality Guard** — implemented
8. **Noise Suppression** — implemented
9. **Global Quick Translate engine/API** — implemented, dormant, **no keybind**
10. **Optional Transcript Export** — implemented, explicit only, no autosave
11. **Caption Customization** — implemented, bounded readability controls only
12. **Voice / Prosody Preservation** — implemented as bounded native speaking-pace preservation, not emotion guessing

Important explicit exclusions remain:
- Auto Language Detection: **do not implement**
- Screen Translation: **do not implement**

---

## Reliability Package — Completed

The reliability work that was planned after the 12-feature roadmap has also already been implemented on `Local`.

### Crash-Safe Startup Recovery
Implemented starting with:
- `862fce3d11 feat(reliability): add crash-safe startup recovery`

Goals:
- detect/reconcile stale runtime/session markers
- fail closed after unclean shutdown
- clear stale transient resources
- never silently resume translation
- remain safe with multiple TranslateIT instances

Follow-up hardening included:
- multi-instance-safe runtime markers
- recovery contract alignment
- source ownership cleanup

### Runtime Watchdog
Implemented:
- `0380d1cb06 feat(reliability): add bounded runtime watchdog`

Behavior:
- detects lack of progress rather than merely process death
- bounded / explicit degraded state
- no aggressive silent auto-restart
- recovery remains user-visible and fail closed

### Device-Loss / Hot-Swap Guard
Implemented:
- `82f8f75647 feat(reliability): add fail-closed device loss guard`

Hardening included:
- avoiding false-positive device-loss detection
- correct iterator/device consumption behavior
- incident surface integration

Policy:
- do not silently switch to another microphone/output
- explicit device failure state
- recovery requires revalidation

### Redacted Diagnostic Support Export
Implemented:
- `2071c25faa feat(reliability): add redacted diagnostic support export`

Existing privacy helpers remain authoritative:
- redact local paths
- redact email
- redact bearer/API/secrets
- diagnostics must not contain transcript/audio/voice-reference content

### Bounded Incident History
Implemented:
- `c698efe91b feat(reliability): add bounded incident history`

Hardening:
- incident I/O moved outside status locks
- live guard failures surfaced
- bounded history only

### Long-Session Pressure Guard
Implemented:
- `56d0be661d feat(reliability): add long-session pressure guard`
- `ed583879c3 feat(reliability): watch long-session pressure`

Hardening included:
- counters scoped per Meeting
- centralized live-monitor ownership
- metadata-only health checks
- source-budget cleanup

The latest related cleanup commit:
- `5d74ac11a0 chore(runtime): leave source budget margin`

---

## Zero-Waste / Architecture Status

Current direction is intentionally conservative:

- no always-running updater service
- updater remains one-shot at startup
- no permanent Meeting poll when no Meeting is active
- Meeting detection polling suspends while Meeting owns a session
- Audio Quality polling suspends when not useful
- shared conditional polling owner exists instead of duplicated timers
- bounded queues remain authoritative
- no unbounded transcript history
- no automatic transcript disk writes
- no second translation engine for Quick Translate
- no second audio capture for Audio Quality Guard
- Noise Suppression works on finalized speech before ASR, not via a neural sidecar
- My Voice pace preservation uses GPT-SoVITS native `speed_factor`, bounded conservatively
- diagnostics/incident state are bounded and privacy-redacted

Repository hygiene was audited:
- no committed `node_modules`
- no committed Rust `target`
- no committed build/dist artifacts
- no Python cache/venv/coverage artifacts

Do not weaken source-size budgets merely to land new functionality. Split ownership instead.

---

## Settings / State Contracts

Runtime settings schema is currently:

`schema_version = 11`

Important canonical settings include:
- source language
- target language
- translation style: Natural / Formal
- Meeting incoming listening direction
- terminology / Preferred Words
- Spoken Terms
- audio device IDs
- Noise Suppression: Auto / Off

Floating-caption preferences are separate presentation state and include bounded:
- visibility
- text size
- reading width
- contrast

Legacy caption preferences must continue migrating safely to Standard width + Standard contrast.

---

## Transcript / Privacy Boundary

Meeting committed transcript remains bounded and transient.

Normal lifecycle:

```text
active committed turns
→ Stop
→ at most one ended transcript snapshot retained in memory
→ optional explicit Markdown/TXT export
```

Rules:
- no automatic transcript persistence
- no cloud transcript history
- no database transcript history
- next Meeting/full cleanup clears the retained snapshot
- export must warn if bounded live history already dropped earlier turns

---

## Current Validation Boundary

REMOTE_GITHUB/source work can prove:
- compile
- Clippy correctness
- unit tests
- frontend typecheck/build/runtime tests
- bridge contracts
- source-size budgets
- bounded queue/state logic
- stale-generation rejection
- lifecycle cleanup ownership
- redaction/privacy contracts
- source architecture and failure-state semantics

Do **not** claim native/product proof from source alone.

The user is still intentionally postponing local testing.

Native Windows acceptance remains required later for:
- real microphone behavior
- virtual microphone routing to Zoom / Teams / Meet
- Meeting app/window detection on installed Windows
- transparent floating WebView behavior
- mixed-DPI / multi-monitor placement
- monitor unplug/replug
- audio-device unplug/replug
- sleep/wake
- real Noise Suppression effect / WER
- real Audio Quality thresholds
- ASR → translation → My Voice latency
- voice similarity and audible pace preservation
- transcript export path in packaged install
- signed updater installed-build flow
- clean-machine install
- actual idle/live CPU/RAM/GPU/VRAM
- long-session hardware stability

---

## What NOT To Do Next

Do not continue adding random translator features merely because local testing is postponed.

Avoid:
- AEC/DSP implementation without hardware evidence
- latency architecture Phase A/B/C/D without TARGET_WINDOWS measurements
- new language expansion without an explicit QA/model plan
- emotion/prosody guessing
- speaker diarization unless it becomes a clear product requirement
- global shortcut setup until explicitly requested
- UI cosmetic expansion without a real usability problem

The repo already contains `docs/foundation/04-realtime-latency-architecture.md`.
Follow its measurement-gated rollout order instead of speculatively implementing all latency phases.

---

## Next Work If Continuing Remotely

The reliability package is now implemented. Therefore the next chat should **not restart that package from zero**.

If the user still does not want local testing, first perform a **fresh audit of the current HEAD** and look only for falsifiable source-level defects such as:

1. CI regression on current HEAD.
2. Dead/stale/unreachable code introduced by the reliability package.
3. Duplicate ownership or duplicate polling/monitoring.
4. Lock contention / filesystem I/O inside runtime status locks.
5. Unbounded logs, incidents, queues, transcript state, temp files, or health counters.
6. Privacy leaks in support bundle / incident diagnostics.
7. Recovery markers that could conflict across multiple app instances.
8. Source-size regressions.
9. Bridge/API contract drift.
10. Incorrect long-session pressure semantics.

Only implement changes when a concrete defect is found.

If no material remote defect remains, **stop remote feature expansion** and preserve the repo for future TARGET_WINDOWS native acceptance.

---

## Suggested New-Chat Starting Prompt

Use something equivalent to:

> Read `docs/knowledge/next-action.md`, `current-validation.md`, `development-discipline.md`, and `source-ownership.md` first. Work only on branch `Local`. Do not local-test or rebuild/package yet. Re-check current HEAD and CI. The 12-feature roadmap and reliability package (crash recovery, watchdog, device-loss guard, diagnostic export, incident history, long-session pressure guard) are already implemented. Audit the current source for remaining high-value reliability/zero-waste defects only; do not repeat finished work or add speculative features.

---

## Native-Test Plan — Preserve For Later

When the user is ready for local Windows acceptance:

1. Run `docs/knowledge/operations/target-windows-performance.md`.
2. Establish outbound-only baseline first.
3. Record:
   - speech boundary
   - finalization
   - queue
   - ASR
   - translation
   - TTS
   - delivery
   - total latency
   - drops/backpressure
   - CPU/RAM/GPU/VRAM
   - meeting-app reception
4. Validate:
   - repeated Start/Stop
   - device unplug/replug
   - sleep/wake
   - long sessions
   - captions + mixed DPI
   - My Voice pace
   - transcript export
   - clean-machine install
   - updater end-to-end
5. Fix only the first measured bottleneck.

Until that point preserve:
- one canonical local translation pipeline
- bounded queues
- at-most-once Meeting output
- terminology determinism
- generation authority
- fail-closed readiness
- privacy-redacted diagnostics
- explicit user control
- app-only startup updater boundary

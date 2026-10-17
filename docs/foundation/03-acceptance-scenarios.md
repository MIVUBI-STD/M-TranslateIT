---
id: document.foundation.acceptance-scenarios
class: DOCUMENT
domain: foundation
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT — Acceptance Scenarios

This file owns **what must be verified before a claim is allowed** and the order/proof context. It does **not** store run outcomes. Current proof interpretation belongs in `docs/knowledge/current-validation.md`; private/local artifacts remain outside tracked source.

## Principles

- One scenario proves one claim.
- Run the smallest relevant scenario(s); stable promotion may deliberately run a broader source gate.
- Stop at the first failure and diagnose the first wrong owner.
- Label proof by actual context: `REMOTE_GITHUB`, `LOCAL_CODE`, `TARGET_WINDOWS`.
- Source/CI proof never substitutes for physical device, real GPU, installed-runtime or clean-machine claims.
- Do not add marker/mock tests merely to claim runtime success.

## Group A — Local AI runtime

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| A1 | Worker contract starts/resolves | LOCAL_CODE or capable CI | canonical worker contract succeeds; blockers empty |
| A2 | CUDA execution truth | TARGET_WINDOWS or matching GPU context | actual selected device/precision reported from executed inference |
| A3 | ASR preload/inference | matching runtime | canonical ASR loads and processes a valid finalized input |
| A4 | Translation ID→EN | matching runtime | complete canonical result or explicit bounded failure; no silent truncation |
| A5 | Translation EN→ID | matching runtime | complete canonical result or explicit bounded failure |
| A6 | Over-length safety | source/runtime test | rejected before silent truncation |
| A7 | Incomplete generation safety | source/runtime test | known incomplete output rejected with explicit cause |
| A8 | Built-in Meeting voice | TARGET_WINDOWS when audio quality/runtime is claimed | selected Built-in Male/Female synthesizes through canonical Meeting voice path |
| A9 | My Voice build→evaluate→approve→reuse | TARGET_WINDOWS | authorized workflow completes; approved actor survives restart/reuse |

## Group B — Windows audio

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| B1 | Physical microphone discovery/probe | TARGET_WINDOWS | selected/default intent is truthful and real capture frames are observed |
| B2 | Session Listening finalization | TARGET_WINDOWS | natural speech produces finalized utterances continuously until Stop; brief accepted speech is traced through the finalized-audio writer rather than treated as a VAD-only success |
| B3 | VAD/drop observability | TARGET_WINDOWS | bounded drop/overflow conditions remain observable rather than silent |
| B4 | Device mutation during session | TARGET_WINDOWS | conflicting selection/setup mutation is rejected/deferred while owned |
| B5 | Meeting route truth | TARGET_WINDOWS | matched route is detected/configured; discovery is not mislabeled as meeting delivery proof |

For B2, distinguish **VAD never finalized** from **VAD finalized but the WAV writer returned `finalized_utterance_writer:segment_too_short`**, and from **WAV admitted but ASR returned an empty transcript**. These are different owner boundaries. Brief natural speech must be included in the single integrated native session, but no VAD threshold or writer safety limit is changed merely because a synthetic fixture or an unmeasured duration appears to conflict.

## Group C — Meeting end-to-end

Requires relevant A/B capability first.

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| C0 | Day-one start with built-in voice | TARGET_WINDOWS | choose Built-in Male/Female; required readiness can reach Start without My Voice training |
| C1 | Outbound end-to-end | TARGET_WINDOWS | Start→capture→ASR→ID→EN→selected voice→Meeting Microphone; stage timing recorded when latency is claimed |
| C1A | Original passthrough without AI | source + TARGET_WINDOWS | Start Original with worker/models unavailable, record physical mic → unchanged selected virtual device; no ASR/MT/TTS requests |
| C1B | Source switching within one session | source + TARGET_WINDOWS | Original→TranslateIT→Original changes only the source, never calling-app input; no overlap, stale translated playback, unintended raw leakage or second capture owner |
| C1C | Original start/stop without voice model | source + TARGET_WINDOWS | Original ignores selected voice readiness but respects microphone, route, session ownership and Stop cleanup; failed native stream does not claim Live |
| C2 | Outbound context isolation | TARGET_WINDOWS or matching runtime fixture | previous translated pairs never enter the model prompt; the current finalized utterance is the only translation source |
| C3 | Incoming context isolation | TARGET_WINDOWS or matching runtime fixture | incoming EN→ID remains context-free and incoming failure does not break outbound |
| C4 | Stop lifecycle | TARGET_WINDOWS | output authority revoked, work/capture cleaned, no dangling handles/stale output |
| C5 | Repeated session stability | TARGET_WINDOWS | repeated start/stop remains stable and bounded |
| C6 | Multi-speaker honesty | TARGET_WINDOWS | browser output can be captured when routed correctly; mixed audio stays one unverified incoming lane, with no false diarization or speaker assignment |
| C8 | Source-first transcript and bounded failure | source/Rust plus TARGET_WINDOWS | ASR source appears before translation; translation failure/deferred expiry preserves it with unavailable status; stale completion after Stop cannot alter text or output |
| C7 | Hardware-aware performance baseline | TARGET_WINDOWS | with RTX 3070, i7-14700-class CPU, 32 GB RAM and browser active, record post-finalization delay, approximate speech onset/end latency, scheduler wait, stream-build cost, full GPU/WDDM peak VRAM and P50/P95; do not claim meeting-app receipt from local callback |

## Group D — Desktop product surfaces

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| D1 | First Setup persistence | LOCAL_CODE/TARGET_WINDOWS as claim requires | setup checkpoints/preferences survive relaunch correctly |
| D2 | Settings sanitize/atomic write | source/local | invalid state is sanitized; writes remain atomic |
| D3 | Readiness truth | source + TARGET_WINDOWS for live claim | Meeting Start depends on required route/runtime plus **a selected built-in or approved My Voice**, not My Voice training specifically |
| D4 | Diagnostics privacy | source/local | no private conversation/raw audio/unredacted user path is exposed by default |
| D5 | Rendered UI hierarchy | rendered local/target | one Meeting primary action, simple Text, categorized Settings, contextual export feedback; remains legible at supported sizes; source alone is not visual PASS |
| D6 | Text Swap data preservation | source + rendered local | swapping source direction never discards untranslated input; stale inference cannot overwrite newer user edits |
| D7 | Deferred Meeting recovery | source + TARGET_WINDOWS | deferred setup exposes one actionable recovery path, and Start/Stop ownership remains authoritative |
| D8 | Professional HTML Preview/export reading truth | source + rendered local/TARGET_WINDOWS | saved HTML remains offline/script-free, with Bilingual/Bahasa Indonesia/English modes; a still-running Meeting preview identifies pending translation as in progress and does not claim storage interruption merely because no finish receipt exists; actual archive errors remain warnings; stopped sessions normalize pending translation to unavailable; full export is stopped-session only; print labels current reading mode without inventing speaker/delivery status |
| D9 | Chat-style transcript | source + rendered local | You/right and combined Meeting audio/left in Live and Last Meeting; original and translated content visible; recorded time accurate; no participant-ID inference |
| D10 | Persistent saved Meeting isolation | source + TARGET_WINDOWS file check | Both Original and TranslateIT Start create a distinct local recognized-turn archive before microphone capture and reject Live on archive start failure; no AI readiness is required for Original. The Saved Transcripts surface discloses automatic text retention, surviving Stop, restart, new sessions and manual HTML export until explicit saved-session Delete. Delete clears the chosen archive and matching in-memory review, never previously completed HTML exports. No raw mic audio is implied |
| D11 | Long-session transcript archive and HTML export | source + TARGET_WINDOWS file check | Live/review remains bounded; record-queue budgets are not disk quotas and a matching finalization-entry count does not prove that every utterance was captured; Full HTML export requires both inactive Meeting authority and absence of a still-owned background writer, otherwise Retry Stop; a stopped unverified archive remains exportable with an explicit completeness warning. Export validates each entry against one inventory, retains source until explicit Delete and never publishes failed .partial HTML; on a subsequent explicit Export, only generated unpublished Transcript_<timestamp>_<nonce>.html.partial files are removed, leaving finished HTML and saved sources untouched; failed temporary cleanup is disclosed. Verify partial cleanup, active writer, disk failure, large Unicode sessions and browser output on the target host before any runtime claim |
| D12 | Single Quick Access naming and launch | source + rendered TARGET_WINDOWS | One hidden-by-default `quick-access` window opens from app header, in-app Ctrl+K, and Windows tray (if supported); old Command Palette absent; all entrypoints open the same popup |
| D13 | Quick Access meeting actions | source + TARGET_WINDOWS with real mic/route | Original/TranslateIT switches only after native acknowledgment; captions show/hide uses the existing overlay; Stop requires explicit confirmation; stale status/events cannot misstate the source; failures stay visible |
| D14 | Quick Access desktop comfort | rendered TARGET_WINDOWS, including secondary monitor | Popup opens within current monitor work area without covering default bottom-center captions, Escape/blur/success hide only the popup, and application exit destroys popup/captions without orphan process |
| D15 | One native Virtual Microphone Mute | source + TARGET_WINDOWS receiver | Muting either Original or translated TTS outputs immediate silence to one virtual endpoint; no muted audio queues for Unmute; source selection is preserved; Floating Captions remain independently available |
| D16 | Synchronized pre-Start source choice | source + TARGET_WINDOWS | Choosing Original/TranslateIT in either Meeting or Quick Access immediately updates both through the same native preference; neither page falsely implies a second audio owner; Start uses the selected source |
| D17 | Caption action and visibility truth | source + TARGET_WINDOWS | Quick Access reads actual native caption-window visibility and delegates Show/Hide to main's existing caption runtime, preserving the active private caption content and state across windows |
| D18 | Saved transcript UI responsiveness and zero-entry deletion | source + rendered TARGET_WINDOWS | saved history/read/HTML preview/export/delete dispatch filesystem work via Tauri blocking workers; empty sessions remain listed with Delete enabled and Read/Preview/Export disabled; a long archive scan and export do not freeze Meeting UI |

## Group E — Distribution

Run when distribution/install claims are in scope.

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| E1 | Release source contract | REMOTE_GITHUB/LOCAL_CODE | controlled Setup/payload inputs, pins, notices and package config validate |
| E2 | Setup + payload build | LOCAL_CODE/capable CI | artifact pair builds and records exact source/artifact identity |
| E3 | Installed runtime | TARGET_WINDOWS | installed private runtime/assets resolve and canonical worker functions |
| E4 | Audio-provider install behavior | TARGET_WINDOWS | provider consent/restart/setup behavior matches policy |
| E5 | Uninstall/reinstall data policy | TARGET_WINDOWS | app runtime is repaired/replaced as intended while user-owned persistent data is preserved |
| E6 | Clean-machine acceptance | TARGET_WINDOWS clean target | fresh install launches and required product path works without developer setup |

## Stable source promotion

`Stable Release Gate` is a repository/source promotion gate. Even when fully green, it does not by itself turn Groups B/C/E target-only claims into PASS.

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
| C2 | Outbound rolling context | TARGET_WINDOWS or matching runtime fixture | only last three committed own-voice pairs affect outbound; current utterance remains primary input |
| C3 | Incoming context isolation | TARGET_WINDOWS or matching runtime fixture | incoming EN→ID remains context-free and incoming failure does not break outbound |
| C4 | Stop lifecycle | TARGET_WINDOWS | output authority revoked, work/capture cleaned, no dangling handles/stale output |
| C5 | Repeated session stability | TARGET_WINDOWS | repeated start/stop remains stable and bounded |

## Group D — Desktop product surfaces

| ID | Claim | Minimum context | Pass criteria |
|---|---|---|---|
| D1 | First Setup persistence | LOCAL_CODE/TARGET_WINDOWS as claim requires | setup checkpoints/preferences survive relaunch correctly |
| D2 | Settings sanitize/atomic write | source/local | invalid state is sanitized; writes remain atomic |
| D3 | Readiness truth | source + TARGET_WINDOWS for live claim | Meeting Start depends on required route/runtime plus **a selected built-in or approved My Voice**, not My Voice training specifically |
| D4 | Diagnostics privacy | source/local | no private conversation/raw audio/unredacted user path is exposed by default |
| D5 | Rendered UI hierarchy | rendered local/target | current state/action remains legible at supported window sizes; source alone is not visual PASS |

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

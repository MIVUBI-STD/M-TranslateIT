---
id: document.knowledge.proposals.my-voice-ui-implementation
class: DOCUMENT
domain: knowledge
role: GUIDE
authority: REFERENCE
lifecycle: ACTIVE
---

# My Voice — Simple Voice Workspace implementation specification

**Decision:** Approved UI structure, implementation not started. This is a bounded UI implementation reference, not an alternate product law, active-work status ledger, or permission to change the engine.
**Source basis:** MIVUBI-STD/M-TranslateIT, branch Local, af56652c1a8123fbbcd621be1d370e23d84fe47f (2026-10-13).
**Product contract:** docs/foundation/02-product-requirements.md, especially PR-004, PR-013, PR-101–103, PR-110–119, PR-160–166. Later Local source and product law take precedence over this proposal.
**Owner:** Existing My Voice Svelte components. Do not create a new backend state manager, voice engine, or independent UI truth.

## 1. Scope and acceptance

**Goal:** Make My Voice understandable to a first-time user without hiding the safety/review actions. Keep the existing Svelte 5 / Tailwind 4 / semantic token stack.

**In scope:**
- Visually organize one page into "Voice for meetings" and "Create My Voice".
- Provide one compact selected-voice summary with a disclosure for built-in Male, built-in Female, and already-saved personal voice.
- Present a three-stage creation journey: Record → Create → Review. "Prepare microphone" and "Review take" belong within Record; neither is a separate page or step.
- Make one primary action, explicit recovery guidance, and progress that reflects the actual backend contract.
- Preserve Quick Preview as an optional, clearly isolated creation-time action.

**Out of scope:** GPT-SoVITS changes, new voices/providers, Designed Voice, importing audio, training/profile controls, multiple personal voice libraries, visualizers/equalizers, neural enhancement, a new wizard router, global theme redesign, persistent draft steps, changes to meeting resource ownership, or new app dependencies.

**Acceptance conditions:**
1. Normal user sees a selected Meeting voice and one clear primary action for the current My Voice task; 128 recording lines are not exposed as a giant mandatory checklist.
2. Existing recording, microphone checks, preview, retries, build, eight held-out previews, approval, and voice switching remain callable and fail closed under the same guards.
3. Recordings and existing approved voice remain safe on cancel, build failure, or a new incomplete candidate; hidden panes do not lose authoritative activity or review state.
4. Mobile-like/narrow desktop content stacks cleanly, focus is visible, all actions keyboard-accessible, and warning text stays next to the affected action.
5. No UI-only state claims voice is "good", "professional", "fully trained", or ready for Meeting merely from elapsed recording time or a functional microphone callback.

## 2. Information architecture and layout

Global app navigation stays Meeting | Text | My Voice | Settings.

My Voice page:
A. Header — "My Voice"; helper "Choose a voice for meetings or create your own."
B. Voice for meetings — one current-selection summary row (voice label + verified selection state) and "Change voice" disclosure. Expanded choices: Built-in Male and Built-in Female, each with existing audio preview + Select; show "My Voice" only if a verified saved personal voice is available. Reuse current selection confirmation and Meeting lock logic.
C. Create My Voice — single workspace with compact progress labels "Record / Create / Review". Show the relevant area, not three simultaneously full-height panels. Keep pending review and recovery actions visible immediately.
D. Optional "Quick Preview" disclosure within the creation workspace, not a new top-level page and never a selectable Meeting voice.

Desktop content >= 920px wide: main task area + narrow supporting summary (approximately 280px). Below 920px content width: one column with progress beneath the current action. These are layout targets to validate after rendering, not new global design tokens. No fixed 300px sidebar at narrow widths. Content is bounded by existing app width and padding tokens; no horizontal scrolling at supported windows.

No full-page stepper route; stage labels are progress orientation, not disabled clickable promises. Avoid redundant section headings like "Step 2" repeated inside multiple cards.

## 3. Single source of UI truth

Do not invent training completion percentage, quality score, or local selected-voice state.

Authoritative inputs currently exist:
- GuidedRecordingState: recording_line_id, pending_review, lines[].accepted.
- GuidedTakeReview: duration_ms, quality_blocker, review_advice.
- MyVoiceBuildStatus: active, preview_active, phase, accepted_duration_ms, minimum_duration_ms, missing_coverage, can_build, evaluation_ready, evaluation_review_id, evaluation_samples, selected_voice_kind, selected_builtin_voice_id, saved_personal_voice_available, approved_voice_ready.
- Existing props: voiceChangesLocked plus unknown/bridge-recovery states; existing microphone check is device-callback verification only.

Keep MyVoiceBuild.svelte mounted during stage changes so polling, playback cleanup, candidate review identity, and in-memory listened flags do not disappear on unmount. Parent may receive a read-only status callback from MyVoiceBuild.applyStatus for presentation; avoid a second independent status polling loop or new persistence. Preserve exact evaluation SHA/review-id binding.

Stage derivation precedence (presentation only):
1. Unknown recording/build status: retain last VERIFIED state and show one relevant "Retry status" action; do not infer idle or safely stopped.
2. Active recording or pending take review: Record, with Stop or Replay / Record again / Keep recording.
3. Active trained build: Create with its actual preparing/training/evaluating/cancelling state; Stop creating remains available.
4. Active Quick Preview: show its isolated generation/cancel affordance clearly; never present it as the trained build.
5. Valid evaluation_ready after completed training: Review, keeping eight evaluation samples and approval checkbox.
6. can_build: Create is available, while "Record more" returns to Record explicitly.
7. Otherwise: Record with the exact next requirement from the backend.

A deliberate "Record more" UI affordance is permitted when safe. If an evaluated candidate exists, explain that changing accepted takes invalidates its existing review and requires recreating the candidate. A temporary choice of visible pane is not training authority.

## 4. Voice for meetings

Collapsed summary: selected label and a truthful state from verified backend only. If selection is unverified: "Could not verify selected voice" and retry, never assume Built-in Male or personal.

Expand "Change voice" to reveal just two built-in choices plus saved personal voice when available; no second independent "Choose Voice" card. Built-in previews keep using packaged WAV; they are not proof of synthesized Meeting output. Selection retains the current confirmation for replacing an active chosen voice. When Meeting/recording/build resource locks apply, keep controls disabled with "Finish the current Meeting or recording before switching voices." Preserve existing safe storage/archive and recovery paths.

When no personal voice exists, do not show a fake selectable personal option; the creation area explains how to make one.

## 5. Record area — exact states and copy

A concise microphone row appears above the line to read:
- Button: "Check microphone"
- Checking: "Checking microphone…"
- Callback verified: "Microphone is connected. Record a sample to check its sound."
- Failure: "Couldn't use the selected microphone. Open Settings > Voice & Audio."
This does NOT say "Audio quality passed", "Quiet room verified", or a measured noise floor. No extra microphone capture pipeline or browser getUserMedia.

One authorization control retains the backend's explicit "my voice or authorized voice" confirmation. Short guidance: "Use a quiet room. Read the text exactly as shown in your natural voice."

Line panel: "Line N", full current text at high legibility; one dominant action:
- Idle: Record; secondary Skip and Replay accepted where applicable.
- Recording: Stop; visible "Recording…" status.
- Pending review: Replay, Record again, Keep recording. Do not enable Keep recording for quality_blocker. If review_advice exists, show concise "Check this recording" and the relevant guidance in-place. No unproven numerical quality badge.
- Hard blocker: "Recording needs another take" + the existing specific reason; Record again primary.
- Accepted line: "Saved" with Record again / Replay accepted as secondary actions.
- Unknown capture status: show existing "Retry status" and do not replace native capture state with an empty fallback.

Review warnings do not become new training blockers. The accepted canonical WAV and native-rate copy remain unchanged.

128 guided lines are OPTIONAL choices, not 128 required completions. Default view surfaces the current selected line and accepted count. Replace the always-visible 128-line scroll pane with a "Browse lines" disclosure (or sheet within the existing page) grouped by source-defined ranges:
1–24 Conversational, 25–30 Questions & intonation, 31–64 Natural sentences, 65–96 Names, numbers & technical terms, 97–128 Longer explanations.
Display saved/unrecorded per line, optionally jump to the next not-yet-accepted line. Preserve the existing Skip and manual selection behavior. While recording or pending review, switching to another line is blocked to preserve ownership. No invented "N of 128 remaining" completion requirement.

## 6. Recording progress, Create, and Review

Progress summary reads verified accepted duration against minimum duration (60 seconds in current source) and accepted line count. At 60 seconds, progress may show full MINIMUM duration, but must NOT display "Ready to create" unless build.can_build is verified. Show the exact missing coverage group or missing 3–10 second reference requirement when returned by backend. If backend status is unavailable, show "Status unavailable", not a cached green Ready.

Create pane when build.can_build is true: "Your recordings meet the minimum requirements." Keep explicit authorization checkbox before Create My Voice. Training states: "Preparing recordings", "Creating My Voice", "Preparing samples for review", "Stopping creation"; no fabricated percentages or time remaining. Cancel keeps accepted takes and the prior selected voice safe.

Review pane: show all eight held-out samples from evaluation_samples (not samples claimed by static code alone). Each is counted as listened only after playback ends as current source does. Keep the candidate review-id and SHA verification. Check confirmation is not prechecked; "Use My Voice" requires every actual sample listened, explicit quality confirmation, verified evaluation, and no Meeting resource lock. Provide "Record more" / "Create another candidate" as secondary actions, with a clear warning about invalidating the old candidate when takes change.

Approved voice status should distinguish "My Voice selected" from "Saved My Voice available" and temporary archive/recovery warnings. A failed approval never implies voice changed.

Quick Preview remains nested under a collapsed "Optional quick preview" disclosure; keep authorization; show stop/cancel if active even if otherwise collapsed. Label explicitly "Not for meetings". Never rebrand Quick Preview as an approved trained voice.

## 7. Design and accessibility

Reuse existing ti-page, ti-panel, ti-button, ti-button-secondary, ti-button-danger, semantic colors, --ti-radius-* and spacing utilities. No new component library or global token changes.

Typography targets: page title consistent with existing ti-page-title; section title ~18–20px; body 14px; helper 12–13px with sufficient contrast. One primary control per task state. Warnings are amber only when review is needed; no permanent green warning container and no invented success badge. Reuse panels; avoid nested borders around every paragraph.

Use semantic buttons (not clickable divs). Clear headings and aria-expanded/aria-controls for disclosures. Focus ring must be visible. aria-live polite for progress/state changes, role=alert for failures needing intervention. Hidden stage controls must be neither tabbable nor announced. Do not steal focus during polling; after Stop transition focus to review heading or Replay in a controlled way. Prevent audio overlap, preserve object-URL cleanup, honor reduced-motion preferences. Do not continuously announce training polling every 1.4 seconds if visible meaning has not changed.

## 8. Mapping to existing Svelte owners

- src/pages/MyVoice.svelte: page order, Record workspace, microphone row, line browsing disclosure, responsive layout, recording/review advice, focus.
- src/components/my-voice/BuiltinVoicePicker.svelte: collapsed selected-voice summary and safe "Change voice" disclosure. Preserve all selection callbacks and confirmation.
- src/components/my-voice/MyVoiceBuild.svelte: single build-status owner, status-to-parent presentation callback if necessary, Record/Create/Review stage presentation, verified progress/coverage guidance, training/review controls and lifecycle/polling.
- src/components/my-voice/QuickVoicePreview.svelte: secondary disclosure and never-Meeting labeling; preserve active cancellation visibility.
- Existing src/styles/app.css and tokens.css: align with existing styles; add scoped responsive/layout rules only if needed.
- Do not edit Python worker, model manifest, Tauri command names, voice actor storage, audio capture, or the existing public bridge API unless a source-proven gap demands a separately scoped decision.

## 9. State and regression coverage to preserve

Checks should include:
- Initial/no recordings; Built-in Male/Female selected, saved personal available, selection unverified.
- Functional mic check: ready, checking, failed; explicitly distinguish connectivity from acoustic quality.
- Recording Stop / failed Stop / ambiguous bridge response; review pending, hard blocker, advisory hint, retry, accepted recording and stale selection.
- Accepted duration >= minimum but missing coverage or usable reference; no false readiness.
- Active build, cancellation, failed build; previous approved voice remains selected.
- Candidate evaluation available and every held-out audio played before approval; playback/hash failure, stale review identity, active Meeting lock.
- Quick Preview remains temporary and cannot be promoted; cancellation survives hiding other content.
- Focus order, keyboard, small window width, screen-reader announcements, reduced motion and CSS overflow.

Protect existing scripts/tests/voice_quick_preview_isolation_contract.test.ts, scripts/tests/voice_build_runtime_sync_contract.test.ts and existing bridge/runtime contracts. Update tests for deliberate markup changes rather than deleting valid lifecycle assertions. Actual Svelte compile, rendered responsive appearance, audio/performance and Windows acceptance remain NOT EXECUTED until the integrated authorized local phase.

## 10. Implementation sequence (reference only, not automatic authorization)

1. Restructure existing BuiltinVoicePicker and parent MyVoice page layout using only current props and callbacks.
2. Keep MyVoiceBuild mounted while surfacing the relevant stage and moving progress to a single concise location; never create duplicate independent polling/state.
3. Collapse line list and optional Quick Preview, refine the state-specific English copy, and improve responsive/keyboard behavior.
4. Update affected source regression tests; static review the exact Local diff and defer compiled/rendered/native claims honestly.

**Stop condition:** all currently supported My Voice actions and recovery paths remain representable with simpler visible hierarchy, with no new engine or backend behavioral change. Source deliverable is a candidate until executed/reviewed at the matching SHA.

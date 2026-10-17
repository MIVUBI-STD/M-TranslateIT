<script lang="ts">
  import "../styles/quick-access.css";
  import { onMount } from "svelte";
  import { AudioLines, Captions, ExternalLink, Languages, Mic, Play, Square, X } from "@lucide/svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { applicationRuntimeApi, type ApplicationSnapshot } from "../app/bridge/applicationRuntimeApi";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import {
    hideQuickAccess, openTranslateIt, captionWindowIsVisible, changeCaptionVisibility,
  } from "../app/runtime/quickAccessRuntime";
  import { APPLICATION_MEETING_OWNER_ID } from "../app/shared/types";

  type Source = "original" | "translateit";
  let snapshot = $state<ApplicationSnapshot | null>(null);
  let choice = $state<Source>("original");
  let captionsVisible = $state<boolean | null>(null);
  let busy = $state(false);
  let confirmingStop = $state(false);
  let errorMessage = $state("");
  let resultMessage = $state("");
  let disposed = false;
  let actionRevision = 0;
  let closeTask: ReturnType<typeof setTimeout> | null = null;

  const meeting = $derived(snapshot?.meeting ?? null);
  const owned = $derived(meeting?.owner_id === APPLICATION_MEETING_OWNER_ID);
  const live = $derived(Boolean(owned && meeting?.authority_active && meeting?.lifecycle === "live"));
  const stopping = $derived(meeting?.lifecycle === "starting" || meeting?.lifecycle === "stopping");
  const foreignOwner = $derived(Boolean(meeting?.has_session && !owned));
  // Foreign-owned runtime resources must be handled by their own feature.
  const needsStop = $derived(Boolean(meeting?.lifecycle === "cleanup_incomplete"
    && (!meeting?.has_session || owned)));
  const canSelectSource = $derived(Boolean(meeting && !stopping && !needsStop
    && !foreignOwner && (live || (!meeting.has_session && meeting.lifecycle === "idle"))));
  const canStart = $derived(Boolean(snapshot && snapshot.revision > 0
    && meeting && !meeting.has_session && meeting.lifecycle === "idle"
    && !snapshot.resources.audio_locked
    && meeting.preflight.microphone_ready && meeting.preflight.meeting_route_ready
    && (choice === "original" || meeting.preflight.start_eligible)));
  const canStop = $derived(Boolean(meeting && !stopping && !foreignOwner
    && meeting.lifecycle !== "unavailable"
    && meeting.runtime_claim !== "frontend_bridge_unavailable"
    && ((meeting.has_session && owned) || needsStop)));
  const sourceLabel = $derived(live
    ? meeting?.audio_source === "original" ? "Original Voice" : "TranslateIT Voice"
    : "No active source");
  const statusLabel = $derived(
    !snapshot || snapshot.revision <= 0 ? "Checking microphone..."
    : live && meeting?.muted ? "Microphone Muted"
    : needsStop ? "Recovery required"
    : stopping ? "Meeting is changing state"
    : foreignOwner ? "Microphone in use"
    : meeting?.lifecycle === "unavailable" ? "Status unavailable"
    : live ? "Microphone Live"
    : "Microphone stopped"
  );

  function syncFromNative(next: ApplicationSnapshot): void {
    // Native application snapshots carry a monotonic observation revision.
    // A delayed poll or event cannot overwrite a newer confirmed source state.
    if (snapshot && next.revision < snapshot.revision) return;
    snapshot = next;
    if (!next.meeting.has_session && next.meeting.lifecycle !== "cleanup_incomplete") {
      confirmingStop = false;
    }
    if ((next.meeting.lifecycle === "live"
        && next.meeting.owner_id === APPLICATION_MEETING_OWNER_ID)
      || (next.meeting.lifecycle === "idle" && !next.meeting.has_session)) {
      choice = next.meeting.audio_source === "original" ? "original" : "translateit";
    }
  }

  async function refresh(): Promise<void> {
    const [current, visible] = await Promise.all([
      applicationRuntimeApi.getSnapshot(), captionWindowIsVisible(),
    ]);
    if (!disposed && !busy) {
      syncFromNative(current);
      captionsVisible = visible;
    }
  }

  function cancelAutoClose(): void {
    actionRevision += 1;
    if (closeTask !== null) { clearTimeout(closeTask); closeTask = null; }
  }

  function closeAfterSuccess(): void {
    // A background action can finish after the user moved back to the call.
    // Never arm an old timer capable of closing a later popup invocation.
    if (!document.hasFocus()) return;
    const token = ++actionRevision;
    if (closeTask !== null) clearTimeout(closeTask);
    closeTask = setTimeout(() => {
      closeTask = null;
      if (!disposed && token === actionRevision && !busy) void hideQuickAccess();
    }, 850);
  }

  async function hide(): Promise<void> {
    cancelAutoClose();
    confirmingStop = false;
    await hideQuickAccess();
  }

  async function selectSource(requested: Source): Promise<void> {
    if (busy || !canSelectSource || !meeting || choice === requested) return;
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    resultMessage = "Switching audio source...";
    try {
      const result = live
        ? await runtimeApi.selectMeetingAudioSource(requested)
        : meeting.lifecycle === "idle" && !meeting.has_session
          ? await runtimeApi.selectMeetingPreferredAudioSource(requested)
          : null;
      const native = await applicationRuntimeApi.getSnapshot();
      if (!disposed) syncFromNative(native);
      const actual = snapshot?.meeting ?? native.meeting;
      if (!result?.ok || actual.audio_source !== requested
        || (live ? actual.lifecycle !== "live" : actual.lifecycle !== "idle")) {
        errorMessage = result?.message ?? "Could not confirm the selected voice source.";
        resultMessage = "";
      } else {
        resultMessage = requested === "original" ? "Original Voice selected." : "TranslateIT Voice selected.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Voice source could not be verified. Check the current Meeting status.";
      resultMessage = "";
    } finally { busy = false; }
  }

  async function meetingAction(): Promise<void> {
    if (busy || stopping || foreignOwner || (!canStart && !canStop) || !meeting) return;
    const stop = canStop;
    if (stop && !confirmingStop) { confirmingStop = true; return; }
    confirmingStop = false;
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    resultMessage = stop ? "Stopping microphone..." : "Starting microphone...";
    try {
      const result = await applicationRuntimeApi.dispatchIntent(
        stop ? "stop_meeting" : choice === "original" ? "start_original_meeting" : "start_meeting",
      );
      if (!disposed) syncFromNative(result.snapshot);
      const actual = snapshot?.meeting ?? result.snapshot.meeting;
      if (!result.ok
        || (stop && (actual.has_session || actual.lifecycle !== "idle"))
        || (!stop && (actual.lifecycle !== "live"
          || actual.audio_source !== choice))) {
        errorMessage = result.message || "Meeting state was not confirmed. Try again.";
        resultMessage = "";
      } else {
        resultMessage = stop ? "Microphone stopped." : "Microphone is live.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "The Meeting action could not be confirmed. Check its status before retrying.";
      resultMessage = "";
    } finally { busy = false; }
  }

  async function toggleMute(): Promise<void> {
    if (busy || !live || !meeting) return;
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    const desired = !meeting.muted;
    resultMessage = desired ? "Muting microphone..." : "Unmuting microphone...";
    try {
      const response = await runtimeApi.setMeetingOutputMuted(desired);
      const native = await applicationRuntimeApi.getSnapshot();
      if (!disposed) syncFromNative(native);
      const actual = snapshot?.meeting ?? native.meeting;
      if (!response?.ok || actual.muted !== desired
        || actual.lifecycle !== "live") {
        errorMessage = response?.message ?? "Mute status could not be confirmed.";
        resultMessage = "";
      } else {
        resultMessage = desired ? "Microphone muted." : "Microphone unmuted.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Mute could not be verified. Check Meeting status.";
      resultMessage = "";
    } finally { busy = false; }
  }

  async function toggleCaptions(): Promise<void> {
    if (busy) return;
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    const desired = captionsVisible === false;
    try {
      const result = await changeCaptionVisibility(desired);
      const actual = await captionWindowIsVisible();
      if (!disposed) captionsVisible = actual;
      if (!result.ok || actual !== desired) {
        errorMessage = result.message || "Floating Captions visibility could not be confirmed.";
        resultMessage = "";
      } else {
        resultMessage = actual ? "Floating Captions shown." : "Floating Captions hidden.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Floating Captions could not be verified. Open Settings for recovery.";
      resultMessage = "";
    } finally { busy = false; }
  }

  async function openMain(): Promise<void> {
    try { await openTranslateIt(); }
    catch { errorMessage = "TranslateIT window could not be opened."; }
  }

  onMount(() => {
    disposed = false;
    const currentWindow = getCurrentWindow();
    let unlistenRuntime: (() => void) | null = null;
    const connect = async () => {
      try {
        const unlisten = await applicationRuntimeApi.subscribe((event) => {
          if (!disposed && !busy && event.snapshot.revision > 0) syncFromNative(event.snapshot);
        });
        if (disposed) unlisten();
        else unlistenRuntime = unlisten;
      } catch { /* Focus refresh remains authoritative. */ }
    };
    const onFocus = () => {
      cancelAutoClose();
      confirmingStop = false;
      if (!busy) resultMessage = "";
      // Preserve an unacknowledged error across close/reopen. New actions
      // explicitly clear it, so the user never loses a failed-switch reason.
      void refresh();
    };
    const onBlur = () => {
      // Never hold a floating panel on top of a call while AI is preparing.
      // Hiding does not cancel or alter the authoritative Meeting action.
      void hide();
    };
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); void hide(); }
    };
    let closeUnlisten: (() => void) | null = null;
    void currentWindow.onCloseRequested((event) => {
      event.preventDefault();
      void hide();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else closeUnlisten = unlisten;
    }).catch(() => {});
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    window.addEventListener("keydown", onKeydown);
    const polling = window.setInterval(() => {
      if (!disposed && !busy && document.hasFocus()) void refresh();
    }, 5000);
    void connect();
    void refresh();
    return () => {
      disposed = true;
      cancelAutoClose();
      window.clearInterval(polling);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("keydown", onKeydown);
      unlistenRuntime?.();
      closeUnlisten?.();
    };
  });
</script>

<div class="qa-shell">
  <section class="qa-panel" aria-label="TranslateIT Quick Access">
    <header class="flex shrink-0 items-center justify-between gap-3 px-4 py-3" data-tauri-drag-region>
      <div class="flex items-center gap-2" data-tauri-drag-region>
        <AudioLines size={18} aria-hidden="true" />
        <strong class="text-[13px] font-semibold">Quick Access</strong>
      </div>
      <button type="button" class="grid size-8 place-items-center rounded-lg text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"
        aria-label="Hide Quick Access" title="Close (Esc)" onclick={() => void hide()}><X size={16}/></button>
    </header>

    <div class="qa-section">
      <div class="flex items-center gap-2">
        <span class={"size-2 rounded-full " + (needsStop ? "bg-[var(--ti-warning)]" : live && !busy ? "bg-[var(--ti-success)]" : "bg-[var(--ti-text-soft)]")} aria-hidden="true"></span>
        <div class="min-w-0 flex-1">
          <p class="m-0 text-[13px] font-semibold" role="status" aria-live="polite">{statusLabel}</p>
          <p class="m-0 mt-1 text-[11px] text-[var(--ti-text-muted)]">{sourceLabel} · Virtual Meeting Microphone</p>
        </div>
      </div>
    </div>

    <div class="qa-section">
      <p class="m-0 mb-3 text-[10px] font-semibold tracking-wide text-[var(--ti-text-muted)]">VOICE SOURCE</p>
      <div class="grid grid-cols-2 gap-2">
        <button type="button" class="qa-choice" aria-pressed={choice === "original"}
          disabled={busy || !canSelectSource || !snapshot || snapshot.revision <= 0}
          onclick={() => void selectSource("original")}>
          <Mic size={18} aria-hidden="true" />
          <span>Original Voice</span>
        </button>
        <button type="button" class="qa-choice" aria-pressed={choice === "translateit"}
          disabled={busy || !canSelectSource || !snapshot || snapshot.revision <= 0}
          onclick={() => void selectSource("translateit")}>
          <Languages size={18} aria-hidden="true" />
          <span>TranslateIT Voice</span>
        </button>
      </div>
      <p class="qa-feedback mt-2">{choice === "original" ? "Direct microphone · No AI" : "English translation · AI required"}</p>
    </div>

    <div class="qa-section flex items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <Mic size={17} aria-hidden="true" />
        <div>
          <p class="m-0 text-[12px] font-semibold">Microphone Mute</p>
          <p class="qa-feedback">{meeting?.muted ? "Virtual output muted" : live ? "Virtual output unmuted" : "Available during Meeting"}</p>
        </div>
      </div>
      <button type="button" class="qa-caption-toggle" role="switch"
        aria-checked={Boolean(meeting?.muted)}
        aria-label={meeting?.muted ? "Unmute microphone" : "Mute microphone"}
        disabled={busy || !live}
        onclick={() => void toggleMute()}>
        {meeting?.muted ? "Unmute" : "Mute"}
      </button>
    </div>

    <div class="qa-section flex items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <Captions size={17} aria-hidden="true" />
        <div>
          <p class="m-0 text-[12px] font-semibold">Floating Captions</p>
          <p class="qa-feedback">{captionsVisible === null ? "Status unavailable" : captionsVisible ? "Visible" : "Hidden"}</p>
        </div>
      </div>
      <button type="button" class="qa-caption-toggle" role="switch" aria-checked={captionsVisible === true}
        aria-label={captionsVisible ? "Hide Floating Captions" : "Show Floating Captions"} disabled={busy || captionsVisible === null}
        onclick={() => void toggleCaptions()}>
        {captionsVisible ? "Hide" : "Show"}
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-2" aria-live="polite">
      {#if errorMessage}
        <p class="qa-feedback qa-error" role="alert">{errorMessage}</p>
      {:else if resultMessage}
        <p class="qa-feedback">{resultMessage}</p>
      {:else if needsStop}
        <p class="qa-feedback qa-error">Audio needs cleanup. Use Retry Stop to recover.</p>
      {:else if !live && !canStart && !foreignOwner && snapshot?.revision}
        <p class="qa-feedback">{choice === "original"
          ? "Connect a physical microphone and the virtual audio route."
          : "Check the microphone, virtual route, and selected translation voice."}</p>
      {/if}
    </div>

    <footer class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--ti-border)] px-4 py-3">
      <button type="button" class="inline-flex items-center gap-1 text-[11px] text-[var(--ti-text-muted)] hover:text-[var(--ti-text)]"
        onclick={() => void openMain()}><ExternalLink size={13} aria-hidden="true" /> Open TranslateIT</button>
      {#if confirmingStop}
        <div class="flex items-center gap-2">
          <button type="button" class="ti-button ti-button-secondary min-h-9 px-2 text-xs"
            onclick={() => { confirmingStop = false; }}>Cancel</button>
          <button type="button" class="ti-button ti-button-danger min-h-9 px-2 text-xs"
            disabled={busy} onclick={() => void meetingAction()}>{needsStop ? "Confirm Retry Stop" : "Confirm Stop"}</button>
        </div>
      {:else}
        <button type="button" class={"ti-button min-h-9 px-3 text-xs " + (canStop ? "ti-button-danger" : "")}
          disabled={busy || (!canStart && !canStop)}
          onclick={() => void meetingAction()}>
          {#if canStop}<Square size={13} aria-hidden="true" /> {needsStop ? "Retry Stop" : "Stop"}
          {:else}<Play size={13} aria-hidden="true" /> Start{/if}
        </button>
      {/if}
    </footer>
  </section>
</div>

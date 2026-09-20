<script lang="ts">
  import { ArrowRight, AudioLines, Languages, Mic, Radio } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    runtimeApi,
    type MeetingCommittedTurnsSnapshot,
    type MeetingSessionStatus,
    type VirtualMicRouteContractStatus,
  } from "../app/bridge/runtimeApi";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { setupStateNeedsResume } from "../app/runtime/setupFlow";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";

  type Tone = "neutral" | "good" | "warning" | "danger";

  let {
    snapshot,
    meetingStatus,
    meetingTurns,
    actionBusy = false,
    onMeetingAction,
    onRefresh,
    onFixSetup,
    onOpenMyVoice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    meetingStatus: MeetingSessionStatus | null;
    meetingTurns: MeetingCommittedTurnsSnapshot | null;
    actionBusy?: boolean;
    onMeetingAction: () => void | Promise<void>;
    onRefresh: () => void | Promise<void>;
    onFixSetup: () => void | Promise<void>;
    onOpenMyVoice: () => void;
  } = $props();

  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let resumeBusy = $state(false);
  let resumeError = $state("");

  const readiness = $derived(snapshot.readiness);
  const meeting = $derived(snapshot.meeting);
  const meetingVoiceReady = $derived(readiness.approvedVoiceReady);
  const setupDeferred = $derived(setupStateNeedsResume(snapshot.settings.meeting_setup_state));
  const runtimeUnavailable = $derived(readiness.level === "unavailable" || meeting.label === "Unavailable");
  const checking = $derived(readiness.level === "checking" && !meeting.hasSession);
  const microphone = $derived(
    String(snapshot.inputStatus?.selected_device_name ?? snapshot.settings.audio.input_device_id ?? "").trim() || "Windows Default",
  );
  const meetingSound = $derived(String(snapshot.settings.audio.output_device_id ?? "").trim() || "Windows Default");
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim() || "Meeting microphone not configured",
  );
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.hasSession && (meeting.live || meeting.busy)));

  function statusTone(ready: boolean, pending = false, unavailable = false): Tone {
    if (unavailable) return "danger";
    if (ready) return "good";
    if (pending) return "neutral";
    return "warning";
  }

  const microphoneUnavailable = $derived(readiness.microphoneStatus === "Unavailable");
  const microphoneTone = $derived(statusTone(readiness.microphoneReady, checking, microphoneUnavailable));
  const routeTone = $derived(statusTone(readiness.meetingRouteReady, checking, runtimeUnavailable));
  const meetingTone = $derived(
    runtimeUnavailable
      ? "danger"
      : meeting.busy || checking
        ? "neutral"
        : meeting.live || readiness.meetingReady
          ? "good"
          : "warning",
  );

  const primaryLabel = $derived(
    actionBusy
      ? meeting.canStop ? "Stopping..." : "Starting..."
      : meeting.live ? "Stop Translation"
      : meeting.busy ? meeting.label === "Stopping" ? "Stopping..." : "Starting..."
      : "Start Translation",
  );
  const primaryDisabled = $derived(actionBusy || meeting.busy || (!meeting.canStart && !meeting.canStop));

  const readyMessage = $derived(
    resumeError
      ? resumeError
      : setupDeferred
        ? "Meeting setup is paused. Resume setup to finish the required items."
        : runtimeUnavailable
          ? "TranslateIT can't reach the local translator right now. Retry the status check."
          : checking || meetingVoiceReady === null
            ? "Checking your microphone, Meeting voice, and meeting output..."
            : meetingVoiceReady === false
              ? "Choose a built-in Meeting voice or create My Voice before starting."
              : readiness.meetingReady
                ? "Ready to translate. Start when your meeting is open."
                : meeting.canStart
                  ? "Start Translation will run a final local translation check before going live."
                  : "Finish the setup items below before starting translation.",
  );

  async function refreshRouteStatus(): Promise<void> {
    try {
      routeStatus = await runtimeApi.getVirtualMicRouteStatus();
    } catch {
      routeStatus = null;
    }
  }

  async function refreshMeetingSetup(): Promise<void> {
    resumeError = "";
    await onRefresh();
    await refreshRouteStatus();
  }

  async function resumeSetup(): Promise<void> {
    if (resumeBusy) return;
    resumeBusy = true;
    resumeError = "";
    const candidate = {
      ...snapshot.settings,
      meeting_setup_state: "new",
      audio: { ...snapshot.settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) {
        resumeError = "Couldn't resume Meeting setup. Try again.";
        return;
      }
      await onRefresh();
    } catch {
      resumeError = "Couldn't resume Meeting setup. Try again.";
    } finally {
      resumeBusy = false;
    }
  }

  onMount(() => {
    void refreshRouteStatus();
  });
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Meeting translation</h2>
      <p class="ti-page-copy">
        {activityVisible
          ? "Speak normally. Finished phrases are translated and spoken into your meeting."
          : "Speak Indonesian. TranslateIT sends English voice to your meeting."}
      </p>
    </div>
    {#if meeting.busy || runtimeUnavailable || setupDeferred || !readiness.meetingReady}
      <StatusBadge
        label={meeting.busy ? meeting.label : runtimeUnavailable ? "Unavailable" : checking ? "Checking" : "Setup Needed"}
        tone={meetingTone}
      />
    {/if}
  </header>

  <article class="ti-panel overflow-hidden">
    {#if activityVisible && meetingStatus}
      <div class="p-5">
        <MeetingActivity status={meetingStatus} turns={meetingTurns} />
      </div>
    {:else}
      <div class="flex items-center justify-between gap-6 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
        <div class="flex min-w-0 items-center gap-4">
          <div class="min-w-0">
            <span class="ti-field-label">You speak</span>
            <strong class="mt-1 block text-[15px] font-semibold">Indonesian</strong>
          </div>
          <div class="grid size-8 shrink-0 place-items-center rounded-full border border-[var(--ti-border)] bg-[var(--ti-surface)] text-[var(--ti-text-soft)]" aria-hidden="true">
            <ArrowRight size={15} />
          </div>
          <div class="min-w-0">
            <span class="ti-field-label">Meeting hears</span>
            <strong class="mt-1 block text-[15px] font-semibold">English voice</strong>
          </div>
        </div>
      </div>

      {#if readiness.meetingReady && meetingVoiceReady && !setupDeferred && !runtimeUnavailable}
        <section class="grid grid-cols-[1fr_auto] items-center gap-5 px-5 py-5">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <StatusBadge label="Ready" tone="good" />
              <span class="text-[12px] text-[var(--ti-text-soft)]">Indonesian → English voice</span>
            </div>
            <p class="mb-0 mt-2 text-[13px] leading-5 text-[var(--ti-text-muted)]">
              Speak into {microphone}. Your meeting should use {meetingMicrophoneDevice}.
            </p>
          </div>
          <button type="button" class="ti-button ti-button-secondary min-h-9" onclick={onOpenMyVoice}>Meeting Voice</button>
        </section>
      {:else}
      <div class="grid grid-cols-3 divide-x divide-[var(--ti-border)]">
        <section class="min-w-0 p-5">
          <div class="flex items-center gap-2 text-[var(--ti-text-muted)]">
            <Mic size={15} strokeWidth={1.8} />
            <span class="ti-field-label">Your microphone</span>
          </div>
          <strong class="mt-2 block break-words text-[13px] font-semibold leading-5">{microphone}</strong>
          <p class="mb-0 mt-1.5 text-[11.5px] leading-[1.55] text-[var(--ti-text-soft)]">The microphone you speak into.</p>
          {#if !readiness.microphoneReady}
            <div class="mt-3">
              <StatusBadge
                label={microphoneUnavailable ? "Unavailable" : checking ? "Checking" : "Setup Needed"}
                tone={microphoneTone}
              />
            </div>
          {/if}
        </section>

        <section class="min-w-0 p-5">
          <div class="flex items-center gap-2 text-[var(--ti-text-muted)]">
            <AudioLines size={15} strokeWidth={1.8} />
            <span class="ti-field-label">Meeting voice</span>
          </div>
          <strong class="mt-2 block text-[13px] font-semibold leading-5">{meetingVoiceReady ? "Selected" : meetingVoiceReady === null ? "Checking..." : "Not selected"}</strong>
          <p class="mb-0 mt-1.5 text-[11.5px] leading-[1.55] text-[var(--ti-text-soft)]">
            {meetingVoiceReady ? "Your selected English Meeting voice." : "Choose Built-in Male/Female or create My Voice before starting."}
          </p>
          {#if !meetingVoiceReady}
            <div class="mt-3 flex flex-wrap items-center gap-2">
              <StatusBadge label={meetingVoiceReady === null ? "Checking" : "Setup Needed"} tone={meetingVoiceReady === null ? "neutral" : "warning"} />
              {#if meetingVoiceReady === false && !setupDeferred}
                <button type="button" class="ti-button ti-button-secondary min-h-8 px-2.5 text-xs" onclick={onOpenMyVoice}>Choose Voice</button>
              {/if}
            </div>
          {/if}
        </section>

        <section class="min-w-0 p-5">
          <div class="flex items-center gap-2 text-[var(--ti-text-muted)]">
            <Radio size={15} strokeWidth={1.8} />
            <span class="ti-field-label">Meeting microphone</span>
          </div>
          <strong class="mt-2 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
          <p class="mb-0 mt-1.5 text-[11.5px] leading-[1.55] text-[var(--ti-text-soft)]">
            {readiness.meetingRouteReady
              ? "Available on Windows. Make sure your meeting app is using this exact microphone."
              : "Meeting microphone setup is required before you start."}
          </p>
          {#if !readiness.meetingRouteReady}
            <div class="mt-3">
              <StatusBadge
                label={runtimeUnavailable ? "Unavailable" : checking ? "Checking" : "Setup Needed"}
                tone={routeTone}
              />
            </div>
          {:else}
            <div class="mt-3"><StatusBadge label="Available" tone="good" /></div>
          {/if}
        </section>
      </div>
      {/if}

      <section class="flex items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
        <div class="min-w-0">
          <div class="flex items-center gap-2 text-[var(--ti-text-muted)]">
            <Languages size={15} strokeWidth={1.8} />
            <span class="ti-field-label">Incoming translation</span>
          </div>
          <strong class="mt-2 block text-[13px] font-semibold leading-5">English → Indonesian text</strong>
          <p class="mb-0 mt-1.5 text-[11.5px] leading-[1.55] text-[var(--ti-text-soft)]">Optional · listens to {meetingSound} · change in Settings</p>
        </div>
        <StatusBadge label="Optional" tone="neutral" />
      </section>
    {/if}

    <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
      {#if !activityVisible}
        <div class="flex min-w-0 items-center gap-2.5 text-[12.5px] text-[var(--ti-text-muted)]" aria-live="polite">
          <span class={`size-1.5 shrink-0 rounded-full ${runtimeUnavailable ? "bg-[var(--ti-danger)]" : readiness.meetingReady && !setupDeferred ? "bg-[var(--ti-success)]" : checking ? "bg-[var(--ti-text-soft)]" : "bg-[var(--ti-warning)]"}`} aria-hidden="true"></span>
          <span>{readyMessage}</span>
        </div>
      {:else}
        <div class="text-[12.5px] text-[var(--ti-text-muted)]">Meeting translation remains active until you stop it.</div>
      {/if}

      <div class="ti-action-row ml-auto">
        {#if setupDeferred && !meeting.live && !meeting.busy}
          <button type="button" class="ti-button min-w-40" disabled={resumeBusy} onclick={() => void resumeSetup()}>{resumeBusy ? "Opening Setup..." : "Resume Setup"}</button>
        {:else}
          {#if !meeting.live && !meeting.busy && !readiness.meetingReady}
            <button type="button" class="ti-button ti-button-secondary" onclick={() => void refreshMeetingSetup()}>{runtimeUnavailable ? "Retry Status" : "Refresh Status"}</button>
            {#if !runtimeUnavailable}
              <button type="button" class="ti-button ti-button-secondary" onclick={onFixSetup}>Repair Setup</button>
            {/if}
          {/if}
          {#if meetingVoiceReady === false && !meeting.live && !meeting.busy}
            <button type="button" class="ti-button min-w-40" onclick={onOpenMyVoice}>Choose Meeting Voice</button>
          {:else}
            <button type="button" class={`ti-button min-w-40 ${meeting.canStop ? "ti-button-danger" : ""}`} disabled={primaryDisabled || meetingVoiceReady === null} onclick={onMeetingAction}>{primaryLabel}</button>
          {/if}
        {/if}
      </div>
    </footer>
  </article>
</section>
<script lang="ts">
  import { ArrowRight, AudioLines, Languages, Mic, Radio, Video } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    runtimeApi,
    type MeetingCommittedTurnsSnapshot,
    type MeetingSessionStatus,
    type MeetingAppDetection,
    type AudioQualityReport,
    type VirtualMicRouteContractStatus,
  } from "../app/bridge/runtimeApi";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { languageName } from "../app/shared/state";
  import type { RuntimeSettings } from "../app/shared/types";
  import { setupStateNeedsResume } from "../app/runtime/setupFlow";
  import { startRuntimePoll } from "../app/runtime/pollRuntime";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingTranscriptExport from "../components/meeting/MeetingTranscriptExport.svelte";
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
    onRefresh: (preferredNotice?: string, knownSettings?: RuntimeSettings) => void | Promise<void>;
    onFixSetup: () => void | Promise<void>;
    onOpenMyVoice: () => void;
  } = $props();

  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let meetingDetection = $state<MeetingAppDetection | null>(null);
  let audioQuality = $state<AudioQualityReport | null>(null);
  let detectionInFlight = false;
  let audioQualityInFlight = false;
  let resumeBusy = $state(false);
  let directionSaving = $state(false);
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
    String(routeStatus?.selected_input_device ?? "").trim() || "TranslateIT microphone not ready",
  );
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.hasSession && (meeting.live || meeting.busy)));
  const outboundSourceName = $derived(languageName(snapshot.settings.source_language));
  const outboundTargetName = $derived(languageName(snapshot.settings.target_language));
  const listenSourceName = $derived(languageName(snapshot.settings.meeting_listen_source_language));
  const listenTargetName = $derived(languageName(snapshot.settings.meeting_listen_target_language));

  function statusTone(ready: boolean, pending = false, unavailable = false): Tone {
    if (unavailable) return "danger";
    if (ready) return "good";
    if (pending) return "neutral";
    return "warning";
  }

  const microphoneUnavailable = $derived(readiness.microphoneStatus === "Unavailable");
  const microphoneTone = $derived(statusTone(readiness.microphoneReady, checking, microphoneUnavailable));
  const routeTone = $derived(statusTone(readiness.meetingRouteReady, checking, runtimeUnavailable));
  const audioQualityTone = $derived<Tone>(
    audioQuality?.quality === "good" ? "good"
      : audioQuality?.quality === "clipping" ? "danger"
      : audioQuality?.available ? "warning"
      : "neutral",
  );
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
        ? "Setup is not finished yet. Continue setup before starting."
        : runtimeUnavailable
          ? "TranslateIT isn't ready right now. Check again in a moment."
          : checking || meetingVoiceReady === null
            ? "Checking your microphone, selected voice, and meeting connection..."
            : meetingVoiceReady === false
              ? "Choose the voice other people will hear before starting."
              : readiness.meetingReady
                ? "Ready. Open your meeting, then start translation."
                : meeting.canStart
                  ? "Everything looks ready. Start Translation will do one final check."
                  : "Finish the items below before starting.",
  );

  async function swapListenDirection(): Promise<void> {
    if (directionSaving || meeting.live || meeting.busy) return;
    directionSaving = true;
    const candidate: RuntimeSettings = {
      ...snapshot.settings,
      meeting_listen_source_language: snapshot.settings.meeting_listen_target_language,
      meeting_listen_target_language: snapshot.settings.meeting_listen_source_language,
      audio: { ...snapshot.settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) throw new Error(result.message || "Meeting listening direction could not be saved.");
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onRefresh(
        `Meeting captions: ${languageName(saved.meeting_listen_source_language)} → ${languageName(saved.meeting_listen_target_language)}`,
        saved,
      );
    } catch {
      await onRefresh("Couldn't change the Meeting listening direction. Try again.");
    } finally {
      directionSaving = false;
    }
  }

  async function refreshAudioQuality(): Promise<void> {
    if (audioQualityInFlight) return;
    audioQualityInFlight = true;
    try {
      audioQuality = await runtimeApi.getAudioQuality();
    } catch {
      audioQuality = null;
    } finally {
      audioQualityInFlight = false;
    }
  }

  async function refreshMeetingDetection(): Promise<void> {
    if (detectionInFlight || meeting.hasSession) return;
    detectionInFlight = true;
    try {
      meetingDetection = await runtimeApi.detectMeetingApp();
    } catch {
      meetingDetection = null;
    } finally {
      detectionInFlight = false;
    }
  }

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

  $effect(() => meeting.hasSession ? undefined : startRuntimePoll(refreshMeetingDetection, 5000));

  $effect(() => {
    if (activityVisible) {
      audioQuality = null;
      return;
    }
    return startRuntimePoll(refreshAudioQuality, 2500);
  });
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Meeting</h2>
      <p class="ti-page-copy">
        {activityVisible
          ? "Speak normally. Finished phrases are translated and spoken into your meeting."
          : "Speak Indonesian. Other people in the call hear the English translation."}
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
            <strong class="mt-1 block text-[15px] font-semibold">{outboundSourceName}</strong>
          </div>
          <div class="grid size-8 shrink-0 place-items-center rounded-full border border-[var(--ti-border)] bg-[var(--ti-surface)] text-[var(--ti-text-soft)]" aria-hidden="true">
            <ArrowRight size={15} />
          </div>
          <div class="min-w-0">
            <span class="ti-field-label">Others hear</span>
            <strong class="mt-1 block text-[15px] font-semibold">{outboundTargetName} voice</strong>
          </div>
        </div>
      </div>

      {#if meetingDetection?.detected}
        <section class="flex items-center justify-between gap-4 border-b border-[var(--ti-border)] bg-[var(--ti-surface)] px-5 py-3.5" aria-live="polite">
          <div class="flex min-w-0 items-center gap-3">
            <div class="grid size-8 shrink-0 place-items-center rounded-[9px] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] text-[var(--ti-text-muted)]" aria-hidden="true">
              <Video size={15} strokeWidth={1.8} />
            </div>
            <div class="min-w-0">
              <strong class="block truncate text-[12.5px] font-semibold">{meetingDetection.provider} detected</strong>
              <p class="mb-0 mt-0.5 truncate text-[11px] text-[var(--ti-text-soft)]">
                {meetingDetection.confidence === "high" ? "Meeting window detected. Translation will only start when you choose Start Translation." : "Meeting app is open. Translation will only start when you choose Start Translation."}
              </p>
            </div>
          </div>
          <StatusBadge label="Detected" tone="good" />
        </section>
      {/if}

      {#if readiness.meetingReady && meetingVoiceReady && !setupDeferred && !runtimeUnavailable}
        <section class="grid grid-cols-3 divide-x divide-[var(--ti-border)]">
          <div class="min-w-0 p-5">
            <span class="ti-field-label">Microphone</span>
            <strong class="mt-1.5 block truncate text-[13px] font-semibold" title={microphone}>{microphone}</strong>
          </div>
          <button type="button" class="min-w-0 p-5 text-left transition-colors hover:bg-[var(--ti-surface-soft)]" onclick={onOpenMyVoice}>
            <span class="ti-field-label">Meeting voice</span>
            <strong class="mt-1.5 block text-[13px] font-semibold">Selected</strong>
          </button>
          <div class="min-w-0 p-5">
            <span class="ti-field-label">TranslateIT microphone</span>
            <strong class="mt-1.5 block truncate text-[13px] font-semibold" title={meetingMicrophoneDevice}>{meetingMicrophoneDevice}</strong>
          </div>
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
          <div class="mt-3 flex flex-wrap items-center gap-2">
            <StatusBadge label={audioQuality?.label ?? "Checking audio"} tone={audioQualityTone} />
            {#if audioQuality?.available}
              <span class="text-[10.5px] text-[var(--ti-text-soft)]" title={audioQuality.note}>
                RMS {audioQuality.rms.toFixed(3)} · peak {audioQuality.peak.toFixed(3)}
              </span>
            {/if}
          </div>
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
            <span class="ti-field-label">TranslateIT microphone</span>
          </div>
          <strong class="mt-2 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
          <p class="mb-0 mt-1.5 text-[11.5px] leading-[1.55] text-[var(--ti-text-soft)]">
            {readiness.meetingRouteReady
              ? "Select this microphone in your meeting app."
              : "TranslateIT microphone setup is required before you start."}
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

    {/if}

    <section class="flex items-center justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-3.5">
      <div class="flex min-w-0 items-center gap-3">
        <Languages size={15} strokeWidth={1.8} class="shrink-0 text-[var(--ti-text-muted)]" />
        <div class="min-w-0">
          <strong class="block text-[12.5px] font-semibold">Translate what you hear: {listenSourceName} → {listenTargetName}</strong>
          <p class="mb-0 mt-0.5 truncate text-[11px] text-[var(--ti-text-soft)]">Optional · listens to {meetingSound}</p>
        </div>
      </div>
      <button
        type="button"
        class="ti-button ti-button-secondary min-h-8 px-3 text-xs"
        disabled={directionSaving || meeting.live || meeting.busy}
        title={meeting.live || meeting.busy ? "Stop Meeting translation before changing direction." : "Swap listening direction"}
        onclick={() => void swapListenDirection()}
      >
        {directionSaving ? "Saving..." : "Swap"}
      </button>
    </section>

    <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
      {#if !activityVisible}
        <div class="flex min-w-0 items-center gap-2.5 text-[12.5px] text-[var(--ti-text-muted)]" aria-live="polite">
          <span class={`size-1.5 shrink-0 rounded-full ${runtimeUnavailable ? "bg-[var(--ti-danger)]" : readiness.meetingReady && !setupDeferred ? "bg-[var(--ti-success)]" : checking ? "bg-[var(--ti-text-soft)]" : "bg-[var(--ti-warning)]"}`} aria-hidden="true"></span>
          <span>{readyMessage}</span>
        </div>
      {:else}
        <div class="text-[12.5px] text-[var(--ti-text-muted)]">Translation stays on until you choose Stop Translation.</div>
      {/if}

      <div class="ti-action-row ml-auto">
        <MeetingTranscriptExport hasSession={meeting.hasSession} onNotice={(message) => void onRefresh(message)} />
        {#if setupDeferred && !meeting.live && !meeting.busy}
          <button type="button" class="ti-button min-w-40" disabled={resumeBusy} onclick={() => void resumeSetup()}>{resumeBusy ? "Opening..." : "Resume Setup"}</button>
        {:else}
          {#if !meeting.live && !meeting.busy && !readiness.meetingReady}
            <button type="button" class="ti-button ti-button-secondary" onclick={() => void refreshMeetingSetup()}>{runtimeUnavailable ? "Retry Status" : "Refresh Status"}</button>
            {#if !runtimeUnavailable}
              <button type="button" class="ti-button ti-button-secondary" onclick={onFixSetup}>Fix Setup</button>
            {/if}
          {/if}
          {#if meetingVoiceReady === false && !meeting.live && !meeting.busy}
            <button type="button" class="ti-button min-w-40" onclick={onOpenMyVoice}>Choose Voice</button>
          {:else}
            <button type="button" class={`ti-button min-w-40 ${meeting.canStop ? "ti-button-danger" : ""}`} disabled={primaryDisabled || meetingVoiceReady === null} onclick={onMeetingAction}>{primaryLabel}</button>
          {/if}
        {/if}
      </div>
    </footer>
  </article>
</section>
<script lang="ts">
  import { ArrowRight, AudioLines, Mic } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import type { UtilityDestination } from "../app/runtime/productCommandRegistry";
  import {
    runtimeApi,
    type MeetingCommittedTurnsSnapshot,
    type MeetingSessionStatus,
    type MeetingAppDetection,
    type VirtualMicRouteContractStatus,
  } from "../app/bridge/runtimeApi";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { languageName } from "../app/shared/state";
  import type { RuntimeSettings } from "../app/shared/types";
  import { setupStateNeedsResume } from "../app/runtime/setupFlow";
  import { meetingControlState } from "../app/bridge/productMeetingState";
  import { classifyVirtualMicRoute } from "../app/bridge/virtualMicRoutePresentation";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingPresetPanel from "../components/meeting/MeetingPresetPanel.svelte";
  import MeetingSessionReview from "../components/meeting/MeetingSessionReview.svelte";
  import MeetingTranscriptExport from "../components/meeting/MeetingTranscriptExport.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";

  type Tone = "neutral" | "good" | "warning" | "danger";

  let {
    utilityTarget = null,
    utilityRequest = 0,
    snapshot,
    meetingStatus,
    meetingTurns,
    actionBusy = false,
    onMeetingAction,
    onRefresh,
    onFixSetup,
    onOpenMyVoice,
  }: {
    utilityTarget?: UtilityDestination | null;
    utilityRequest?: number;
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
  let detectionInFlight = false;
  let resumeBusy = $state(false);
  let directionSaving = $state(false);
  let resumeError = $state("");
  let optionsOpen = $state(false);

  const readiness = $derived(snapshot.readiness);
  const meeting = $derived(snapshot.meeting);
  const primaryControl = $derived(meetingControlState(
    meeting,
    snapshot.readiness.approvedVoiceReady,
    snapshot.settings.meeting_setup_state,
    actionBusy,
  ));
  const meetingVoiceReady = $derived(readiness.approvedVoiceReady);
  const setupDeferred = $derived(setupStateNeedsResume(snapshot.settings.meeting_setup_state));
  const runtimeUnavailable = $derived(readiness.level === "unavailable" || meeting.label === "Unavailable");
  const checking = $derived(readiness.level === "checking" && !meeting.hasSession);
  const microphone = $derived(
    String(snapshot.inputStatus?.selected_device_name ?? snapshot.settings.audio.input_device_id ?? "").trim() || "Windows Default",
  );
  const meetingSound = $derived(String(snapshot.settings.audio.output_device_id ?? "").trim() || "Windows Default");
  const routeCheck = $derived(classifyVirtualMicRoute(routeStatus));
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim()
      || (routeCheck === "ready" ? "Device name unavailable" : routeCheck === "checking" ? "Checking..." : "Not available in Windows"),
  );
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.live));
  // Meeting outbound is always Indonesian speech to English voice. Swapping
  // Text's shared source/target preferences must not alter the Meeting label.
  const outboundSourceName = "Indonesian";
  const outboundTargetName = "English";
  const listenSourceName = $derived(languageName(snapshot.settings.meeting_listen_source_language));
  const listenTargetName = $derived(languageName(snapshot.settings.meeting_listen_target_language));
  const meetingDetectionAdvisory = "Meeting detection is advisory. Translation will only start when you choose Start Translation.";

  const meetingTone = $derived<Tone>(
    meeting.lifecycle === "cleanup_incomplete" || meeting.hasSession && !meeting.live && !meeting.busy ? "warning"
      : runtimeUnavailable ? "danger"
      : meeting.busy || checking ? "neutral"
      : meeting.live || readiness.meetingReady ? "good"
      : "warning",
  );
  const healthyReady = $derived(
    !meeting.hasSession && !meeting.canStop && !setupDeferred
    && !runtimeUnavailable && readiness.meetingReady && meetingVoiceReady === true,
  );
  const readyMessage = $derived(
    resumeError || (
      meeting.canStop || meeting.hasSession ? meeting.message
      : setupDeferred ? "Meeting setup is not finished. Resume setup before starting a call."
      : runtimeUnavailable ? "Meeting status is unavailable. Check status before starting."
      : checking || meetingVoiceReady === null ? "Checking microphone, voice and translation readiness..."
      : !readiness.microphoneReady ? "Microphone unavailable. Check your device in Settings."
      : !readiness.meetingRouteReady ? "TranslateIT microphone is not available in Windows. Repair setup before starting."
      : meetingVoiceReady === false ? "Choose an English Meeting voice before starting."
      : readiness.meetingReady ? ""
      : "Meeting needs a readiness check. Check again or repair setup."
    ),
  );

  async function swapListenDirection(): Promise<void> {
    if (directionSaving || meeting.hasSession || meeting.canStop) return;
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

  $effect(() => {
    const target = utilityTarget;
    const request = utilityRequest;
    if (target !== "meeting-presets" && target !== "meeting-transcript") return;
    optionsOpen = true;
    void tick().then(() => {
      if (utilityTarget !== target || utilityRequest !== request) return;
      const element = document.getElementById(target === "meeting-presets" ? "utility-meeting-presets" : "utility-meeting-transcript");
      element?.scrollIntoView({ block: "center" });
      element?.focus({ preventScroll: true });
    });
  });

  onMount(() => {
    void refreshRouteStatus();
    void refreshMeetingDetection();
  });
</script>

<section class="ti-page ti-page-wide" style="margin-inline: 0; max-width: 1080px;">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Meeting</h2>
      <p class="ti-page-copy">{outboundSourceName} speech → {outboundTargetName} voice</p>
    </div>
    {#if !activityVisible && (meeting.busy || meeting.canStop || meeting.hasSession || runtimeUnavailable || setupDeferred || !readiness.meetingReady)}
      <StatusBadge
        label={meeting.busy || meeting.canStop || meeting.hasSession ? meeting.label : runtimeUnavailable ? "Unavailable" : checking ? "Checking" : "Setup Needed"}
        tone={meetingTone}
      />
    {/if}
  </header>

  <article class="ti-panel overflow-hidden">
    {#if activityVisible && meetingStatus}
      <div class="px-6 py-5">
        <MeetingActivity status={meetingStatus} turns={meetingTurns} />
      </div>
    {:else}
      {#if readyMessage}
        <p class="m-0 border-b border-[var(--ti-border)] px-6 py-4 text-sm leading-6 text-[var(--ti-text-muted)]" aria-live="polite">
          {readyMessage}
        </p>
      {/if}
      <div class="grid gap-0 lg:grid-cols-2">
        <div class="min-w-0 px-6 py-5 lg:border-r border-[var(--ti-border)]">
          <div class="flex items-center gap-2 text-[var(--ti-text-muted)]">
            <Mic size={17} aria-hidden="true" />
            <span class="text-sm font-medium">Microphone</span>
          </div>
          <strong class="mt-2 block break-words text-base font-semibold">{microphone}</strong>
        </div>
        <button type="button" class="min-w-0 border-t border-[var(--ti-border)] px-6 py-5 text-left hover:bg-[var(--ti-surface-soft)] lg:border-t-0"
          disabled={meeting.hasSession || meeting.canStop || meeting.busy} onclick={onOpenMyVoice}>
          <span class="flex items-center gap-2 text-sm font-medium text-[var(--ti-text-muted)]">
            <AudioLines size={17} aria-hidden="true" /> Meeting voice
          </span>
          <span class="mt-2 flex flex-wrap items-center justify-between gap-2">
            <strong class="text-base font-semibold">{meetingVoiceReady === true ? "Selected" : meetingVoiceReady === null ? "Checking..." : "Choose a voice"}</strong>
            <span class="text-sm text-[var(--ti-text-muted)]">Change voice <ArrowRight size={14} class="inline" aria-hidden="true" /></span>
          </span>
        </button>
      </div>
    {/if}

    <details bind:open={optionsOpen} class="border-t border-[var(--ti-border)]">
      <summary class="cursor-pointer px-6 py-3.5 text-sm font-medium text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)] hover:text-[var(--ti-text)]">
        Meeting options
        <span class="ml-2 text-xs font-normal text-[var(--ti-text-soft)]">Device routing, captions, presets and export</span>
      </summary>
      <div class="border-t border-[var(--ti-border)]">
        <section class="flex flex-wrap items-start justify-between gap-3 px-6 py-4">
          <div class="min-w-0">
            <h3 class="m-0 text-sm font-semibold">Set up your calling app</h3>
            {#if routeCheck === "unavailable"}
              <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">Couldn't check the TranslateIT microphone in Windows. Check Settings before configuring your calling app.</p>
            {:else if routeCheck === "ready" && routeStatus?.selected_input_device}
              <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">
                Select <strong class="font-medium text-[var(--ti-text)]">{meetingMicrophoneDevice}</strong> as the microphone input in your calling app.
              </p>
              <p class="mb-0 mt-1 text-xs text-[var(--ti-text-soft)]">Available in Windows; TranslateIT cannot verify your calling app's input selection.</p>
            {:else if routeCheck === "ready" || routeCheck === "checking"}
              <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">Check the TranslateIT microphone's device name in Settings before selecting it in your calling app.</p>
            {:else}
              <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">The TranslateIT microphone isn't ready. Open Settings and repair the setup before starting a call.</p>
            {/if}
          </div>
        </section>

        <section class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--ti-border)] px-6 py-4">
          <div class="min-w-0">
            <h3 class="m-0 text-sm font-semibold">Translate what you hear: {listenSourceName} → {listenTargetName}</h3>
            <p class="mb-0 mt-1 text-sm text-[var(--ti-text-muted)]">Optional incoming captions · Meeting sound: {meetingSound}</p>
          </div>
          <button
            type="button"
            class="ti-button ti-button-secondary min-h-9"
            disabled={directionSaving || meeting.hasSession || meeting.canStop}
            title={meeting.hasSession || meeting.canStop ? "Finish stopping Meeting translation before changing direction." : "Swap listening direction"}
            onclick={() => void swapListenDirection()}
          >{directionSaving ? "Saving..." : "Swap"}</button>
        </section>

        <div id="utility-meeting-presets" tabindex="-1" class="border-t border-[var(--ti-border)]">
          <MeetingPresetPanel
            detection={meetingDetection}
            settings={snapshot.settings}
            locked={meeting.hasSession || meeting.canStop}
            advisoryCopy={meetingDetectionAdvisory}
            {onRefresh}
            onNotice={(message) => void onRefresh(message)}
          />
        </div>
        <div id="utility-meeting-transcript" tabindex="-1" class="flex flex-wrap items-center justify-end gap-3 border-t border-[var(--ti-border)] px-6 py-4">
          <MeetingTranscriptExport hasSession={meeting.hasSession} onNotice={(message) => void onRefresh(message)} />
        </div>
      </div>
    </details>

    <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] px-6 py-4">
      <p class="m-0 min-w-0 flex-1 text-sm text-[var(--ti-text-muted)]">
        {activityVisible ? "Translation is on until you stop it." : healthyReady ? "Ready to translate." : ""}
      </p>
      <div class="ti-action-row ml-auto">
        {#if setupDeferred && !meeting.hasSession && !meeting.canStop && !meeting.busy}
          <button type="button" class="ti-button" disabled={resumeBusy} onclick={() => void resumeSetup()}>
            {resumeBusy ? "Opening..." : "Resume Setup"}
          </button>
        {:else}
          {#if !meeting.hasSession && !meeting.canStop && !meeting.busy && !readiness.meetingReady}
            <button type="button" class="ti-button ti-button-secondary" onclick={() => void refreshMeetingSetup()}>
              {runtimeUnavailable ? "Retry Status" : "Check Again"}
            </button>
            {#if !runtimeUnavailable && !checking}
              <button type="button" class="ti-button ti-button-secondary" onclick={onFixSetup}>Repair Setup</button>
            {/if}
          {/if}
          {#if meetingVoiceReady === false && !meeting.hasSession && !meeting.canStop && !meeting.busy}
            <button type="button" class="ti-button" onclick={onOpenMyVoice}>Choose Voice</button>
          {:else if primaryControl.action !== null || meeting.busy || actionBusy}
            <button
              type="button"
              class={`ti-button min-w-40 ${primaryControl.action === "stop" ? "ti-button-danger" : ""}`}
              disabled={!primaryControl.enabled}
              onclick={onMeetingAction}
            >{primaryControl.label}</button>
          {/if}
        {/if}
      </div>
    </footer>
  </article>

  <MeetingSessionReview sessionActive={meeting.hasSession} onNotice={(message) => void onRefresh(message)} />
</section>

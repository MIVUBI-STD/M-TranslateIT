<script lang="ts">
  import { onMount } from "svelte";
  import { classifyVirtualMicRoute } from "../app/bridge/virtualMicRoutePresentation";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../app/bridge/runtimeApi";
  import {
    runtimeProductFacade,
    type ProductAudioDeviceKind,
    type ProductRuntimeSnapshot,
    type ProductSetupAction,
  } from "../app/bridge/runtimeProductFacade";
  import { deviceId } from "../app/shared/state";
  import type { AudioDeviceListReport, RuntimeSettings } from "../app/shared/types";
  import MeetingNoiseSuppression from "../components/settings/MeetingNoiseSuppression.svelte";
  import FloatingCaptionPreferences from "../components/settings/FloatingCaptionPreferences.svelte";
  import TranslationFeedbackReview from "../components/settings/TranslationFeedbackReview.svelte";
  import SettingsDiagnostics from "../components/settings/SettingsDiagnostics.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";
  import StatusRow from "../components/ui/StatusRow.svelte";

  type SettingsTab = "meeting" | "translation" | "advanced";

  let {
    snapshot,
    settings,
    setupBusy = false,
    micTestBusy = false,
    onSettingsChange,
    onRefresh,
    onSetupAction,
    onFixSetup,
    onMicTest,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    settings: RuntimeSettings;
    setupBusy?: boolean;
    micTestBusy?: boolean;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onRefresh: (message?: string) => void | Promise<void>;
    onSetupAction: (action: ProductSetupAction) => void | Promise<void>;
    onFixSetup: () => void | Promise<void>;
    onMicTest: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let tab = $state<SettingsTab>("meeting");
  let diagnosticsOpen = $state(false);
  let diagnosticsLoading = $state(false);
  let diagnosticsRefreshRevision = $state(0);
  let devices = $state<AudioDeviceListReport | null>(null);
  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let devicesLoading = $state(false);
  let deviceSaving = $state(false);
  let deviceMessage = $state("Loading audio devices...");

  const meetingResourcesLocked = $derived(snapshot.resources.audio_locked);
  const micTestOwnsResources = $derived(snapshot.resources.owner_kind === "mic_test");
  const micTestBlockedByMeeting = $derived(snapshot.resources.audio_locked && !micTestOwnsResources);
  const meetingResourceLockMessage = $derived(
    snapshot.resources.owner_kind === "meeting"
      ? "Stop Translation before changing meeting audio or running Repair Setup."
      : snapshot.resources.owner_kind === "mic_test"
        ? "Stop Mic Test before changing meeting audio or running Repair Setup."
        : snapshot.resources.owner_kind === "voice_recording"
          ? "Stop My Voice recording before changing meeting audio or running Repair Setup."
          : "Finish the current audio action before changing meeting audio or running Repair Setup.",
  );
  const routeCheck = $derived(classifyVirtualMicRoute(routeStatus));
  const virtualMicrophoneReady = $derived(routeCheck === "ready");
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim()
      || (routeCheck === "unavailable" ? "Status unavailable"
        : routeCheck === "checking" ? "Checking..."
        : routeCheck === "ready" ? "Device name unavailable" : "Not available in Windows"),
  );


  function currentDevice(kind: ProductAudioDeviceKind): string {
    return String(kind === "microphone" ? settings.audio.input_device_id ?? "" : settings.audio.output_device_id ?? "");
  }

  function deviceMissing(kind: ProductAudioDeviceKind): boolean {
    const selected = currentDevice(kind).trim();
    if (!selected || !devices?.ok) return false;
    const list = kind === "microphone" ? devices.input_devices : devices.output_devices;
    return !list.some((device) => deviceId(device) === selected);
  }

  async function loadDevices(): Promise<void> {
    if (devicesLoading) return;
    devicesLoading = true;
    deviceMessage = "Loading audio devices...";
    try {
      devices = await runtimeProductFacade.loadProductAudioDevices();
      deviceMessage = devices.ok ? "Choose a device. TranslateIT checks it before saving." : "Audio devices are unavailable right now. Try again.";
    } catch {
      devices = null;
      deviceMessage = "Audio devices are unavailable right now. Try again.";
    } finally {
      devicesLoading = false;
    }
  }

  async function loadRouteStatus(): Promise<void> {
    try {
      routeStatus = await runtimeApi.getVirtualMicRouteStatus();
    } catch {
      routeStatus = null;
    }
  }

  async function changeDevice(kind: ProductAudioDeviceKind, value: string): Promise<void> {
    if (deviceSaving) return;
    if (meetingResourcesLocked) {
      deviceMessage = meetingResourceLockMessage;
      onNotice(meetingResourceLockMessage);
      return;
    }

    const candidate = value.trim() || null;
    const previous = kind === "microphone" ? settings.audio.input_device_id : settings.audio.output_device_id;
    if ((previous ?? null) === candidate) return;

    deviceSaving = true;
    deviceMessage = "Checking device...";
    try {
      const result = await runtimeProductFacade.selectProductAudioDevice(kind, candidate, settings);
      deviceMessage = result.message;
      onNotice(result.message);
      if (!result.ok) return;
      await onSettingsChange(result.settings);
      await onRefresh(result.message);
      await loadDevices();
      await loadRouteStatus();
    } catch {
      deviceMessage = "The device wasn't changed. Try again or check Diagnostics.";
      onNotice(deviceMessage);
    } finally {
      deviceSaving = false;
    }
  }

  async function runSetupRepair(): Promise<void> {
    if (meetingResourcesLocked) {
      deviceMessage = meetingResourceLockMessage;
      onNotice(meetingResourceLockMessage);
      return;
    }
    await onFixSetup();
    await loadRouteStatus();
  }

  function selectValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  async function refreshDiagnostics(): Promise<void> {
    if (diagnosticsLoading) return;
    diagnosticsOpen = true;
    diagnosticsLoading = true;
    // The mounted reliability panel observes this revision on every explicit
    // Check Again, rather than keeping only its initial onMount snapshot.
    diagnosticsRefreshRevision += 1;
    onNotice("Refreshing Diagnostics...");
    try {
      await onRefresh("Diagnostics refreshed.");
      await loadRouteStatus();
    } finally {
      diagnosticsLoading = false;
    }
  }

  onMount(() => {
    void loadDevices();
    void loadRouteStatus();
  });
</script>

<section class="min-h-0 overflow-y-auto">
  <div class="ti-page" style="max-width: 1220px;">
    <header class="ti-page-header">
      <div>
        <h2 class="ti-page-title">Settings</h2>
        <p class="ti-page-copy">Audio and meeting preferences.</p>
      </div>
    </header>
    <div class="grid grid-cols-1 gap-5 lg:grid-cols-[190px_minmax(0,1fr)]">
      <nav class="flex flex-col gap-1 self-start rounded-[var(--ti-radius-md)] bg-[var(--ti-surface-soft)] p-3" aria-label="Settings categories">
        <p class="px-2 text-[10px] font-semibold uppercase tracking-wide text-[var(--ti-text-soft)]">App Settings</p>
        <button type="button" class={"rounded-[8px] px-3 py-2.5 text-left text-[13px] " + (tab === "meeting" ? "bg-[var(--ti-surface-raised)] font-semibold" : "hover:bg-[var(--ti-surface-raised)]")}
          aria-current={tab === "meeting" ? "page" : undefined}
          onclick={() => { tab = "meeting"; diagnosticsOpen = false; }}>Voice &amp; Audio</button>
        <button type="button" class={"rounded-[8px] px-3 py-2.5 text-left text-[13px] " + (tab === "translation" ? "bg-[var(--ti-surface-raised)] font-semibold" : "hover:bg-[var(--ti-surface-raised)]")}
          aria-current={tab === "translation" ? "page" : undefined}
          onclick={() => { tab = "translation"; diagnosticsOpen = false; }}>Meeting Captions</button>
        <p class="mt-3 px-2 text-[10px] font-semibold uppercase tracking-wide text-[var(--ti-text-soft)]">Support</p>
        <button type="button" class={"rounded-[8px] px-3 py-2.5 text-left text-[13px] " + (tab === "advanced" ? "bg-[var(--ti-surface-raised)] font-semibold" : "hover:bg-[var(--ti-surface-raised)]")}
          aria-current={tab === "advanced" ? "page" : undefined}
          onclick={() => { tab = "advanced"; diagnosticsOpen = false; }}>Help</button>
      </nav>
      <div class="min-w-0">
    {#if tab === "meeting"}
      <article  class="ti-panel overflow-hidden">
        <header class="flex items-start justify-between gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <div>
            <h3 class="m-0 text-[15px] font-semibold tracking-[-0.015em]">Voice &amp; Audio</h3>
            <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Choose devices here once. Meeting uses your saved preferences automatically.</p>
          </div>
          {#if meetingResourcesLocked}
            <StatusBadge label="In Use" tone="neutral" />
          {:else if !snapshot.readiness.meetingReady}
            <StatusBadge label={snapshot.readiness.level === "unavailable" ? "Unavailable" : "Setup Needed"} tone={snapshot.readiness.level === "unavailable" ? "danger" : "warning"} />
          {/if}
        </header>

        <div class="grid grid-cols-2 gap-5 p-5">
          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Microphone</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving} value={currentDevice("microphone")} onchange={(event) => void changeDevice("microphone", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.input_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_input_names.includes(device.name) || routeStatus?.selected_input_device === device.name}>{device.name}{devices?.ambiguous_input_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_input_device === device.name ? " · virtual output (not a spoken mic)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("microphone")}
                <option value={currentDevice("microphone")}>{currentDevice("microphone")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Your actual microphone, not CABLE Output. This feeds speech recognition, not the calling app directly.</small>
          </label>

          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Sound from your meeting</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving} value={currentDevice("meeting-sound")} onchange={(event) => void changeDevice("meeting-sound", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.output_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_output_names.includes(device.name) || routeStatus?.selected_output_device === device.name}>{device.name}{devices?.ambiguous_output_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_output_device === device.name ? " · virtual TTS output (not meeting sound)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("meeting-sound")}
                <option value={currentDevice("meeting-sound")}>{currentDevice("meeting-sound")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[11.5px] leading-5 text-[var(--ti-text-soft)]">For optional incoming captions, choose the same physical speaker/headset output selected in your calling app. TranslateIT does not change its speaker setting.</small>
          </label>
        </div>

        {#if (devices?.ambiguous_input_names.length ?? 0) > 0 || (devices?.ambiguous_output_names.length ?? 0) > 0}
          <p class="mx-5 mb-4 mt-0 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">
            Multiple Windows audio endpoints have identical names. Those selections are disabled to prevent accidental routing to the wrong device.
          </p>
        {/if}

        <MeetingNoiseSuppression
          {settings}
          locked={meetingResourcesLocked}
          onSettingsChange={onSettingsChange}
          onNotice={onNotice}
        />

        <div class="flex items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
          <div class="min-w-0">
            <span class="ti-field-label">TranslateIT microphone</span>
            <strong class="mt-1.5 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
            <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
              {routeCheck === "unavailable" ? "Couldn't check the virtual microphone. Check Again before changing your call settings."
                : routeCheck === "checking" ? "Checking the virtual microphone in Windows..."
                : virtualMicrophoneReady
                  ? "Choose this as the calling app's microphone INPUT. Its speaker OUTPUT should remain your physical headset or speakers."
                  : "TranslateIT's virtual microphone route is not ready. Check VB-CABLE or run Repair Setup before Meeting translation."}
            </p>
          </div>
          {#if !virtualMicrophoneReady}
            <StatusBadge
              label={routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
              tone={routeCheck === "setup_needed" ? "warning" : "neutral"}
            />
          {/if}
        </div>

        <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <p class="m-0 min-w-0 flex-1 text-[12px] leading-5 text-[var(--ti-text-muted)]" aria-live="polite">{meetingResourcesLocked ? meetingResourceLockMessage : deviceMessage}</p>
          <div class="ti-action-row shrink-0">
            {#if routeCheck === "unavailable"}
              <button type="button" class="ti-button ti-button-secondary" disabled={devicesLoading || deviceSaving} onclick={() => void loadRouteStatus()}>Check Again</button>
            {/if}
            <button type="button" class="ti-button ti-button-secondary" disabled={micTestBlockedByMeeting || micTestBusy || setupBusy || deviceSaving} onclick={() => void onMicTest()}>{micTestBusy ? "Working..." : snapshot.readiness.recording || micTestOwnsResources ? "Stop Mic Test" : "Mic Test"}</button>
            <button type="button" class="ti-button ti-button-secondary" disabled={meetingResourcesLocked || setupBusy || deviceSaving} onclick={() => void runSetupRepair()}>{setupBusy ? "Fixing..." : "Repair Setup"}</button>
          </div>
        </footer>
      </article>
    {:else if tab === "translation"}
      <div >
        <FloatingCaptionPreferences {onNotice} />
      </div>
    {:else if !diagnosticsOpen}
      <article class="ti-panel overflow-hidden">
        <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <h3 class="m-0 text-[15px] font-semibold">Is everything ready?</h3>
          <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Most users can stop here. Open technical details only if something is not working.</p>
        </header>

        <div class="divide-y divide-[var(--ti-border)]">
          <StatusRow
            label="Text translation"
            value="Indonesian ↔ English"
            detail="Written Indonesian ↔ English translation."
            status={snapshot.readiness.textReady ? "Ready" : snapshot.readiness.textStatus}
            tone={snapshot.readiness.textReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
          />
          <StatusRow
            label="Meeting translation"
            value="Indonesian voice → English voice"
            detail="Checks the microphone, voice, and connection needed for calls."
            status={snapshot.readiness.meetingReady ? "Ready" : snapshot.readiness.meetingStatus}
            tone={snapshot.readiness.meetingReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
          />
        </div>

        <footer class="flex justify-end border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={diagnosticsLoading} onclick={() => void refreshDiagnostics()}>{diagnosticsLoading ? "Opening..." : "Open Diagnostics"}</button>
        </footer>
      </article>
      <details class="ti-panel mt-4">
        <summary class="cursor-pointer px-5 py-4 text-sm font-medium">Past translation issue reports</summary>
        <div class="border-t border-[var(--ti-border)] p-4">
          <TranslationFeedbackReview {onNotice} />
        </div>
      </details>
    {:else}
      <div >
      <SettingsDiagnostics
        {snapshot}
        {setupBusy}
        {diagnosticsLoading}
        refreshRevision={diagnosticsRefreshRevision}
        onRefreshDiagnostics={refreshDiagnostics}
        {onSetupAction}
        onBack={() => { diagnosticsOpen = false; }}
      />
      </div>
    {/if}
      </div>
    </div>
  </div>
</section>

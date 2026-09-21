<script lang="ts">
  import { onMount } from "svelte";
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
  import TranslationPreferences from "../components/settings/TranslationPreferences.svelte";
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
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim() || "TranslateIT microphone not ready",
  );


  function currentDevice(kind: ProductAudioDeviceKind): string {
    return String(kind === "microphone" ? settings.audio.input_device_id ?? "" : settings.audio.output_device_id ?? "");
  }

  function deviceMissing(kind: ProductAudioDeviceKind): boolean {
    const selected = currentDevice(kind).trim();
    if (!selected || !devices) return false;
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
  <div class="ti-page">
    <header class="ti-page-header">
      <div>
        <h2 class="ti-page-title">Settings</h2>
        <p class="ti-page-copy">Microphone, translation preferences, and help.</p>
      </div>

      <nav class="flex gap-1 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] p-1" aria-label="Settings sections">
        <button
          type="button"
          class={`min-h-8 rounded-[8px] px-3.5 text-[12.5px] font-semibold transition-colors ${tab === "meeting" ? "bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "text-[var(--ti-text-muted)] hover:text-[var(--ti-text)]"}`}
          aria-current={tab === "meeting" ? "page" : undefined}
          onclick={() => { tab = "meeting"; diagnosticsOpen = false; }}
        >Meeting</button>
        <button
          type="button"
          class={`min-h-8 rounded-[8px] px-3.5 text-[12.5px] font-semibold transition-colors ${tab === "translation" ? "bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "text-[var(--ti-text-muted)] hover:text-[var(--ti-text)]"}`}
          aria-current={tab === "translation" ? "page" : undefined}
          onclick={() => { tab = "translation"; diagnosticsOpen = false; }}
        >Translation</button>
        <button
          type="button"
          class={`min-h-8 rounded-[8px] px-3.5 text-[12.5px] font-semibold transition-colors ${tab === "advanced" ? "bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "text-[var(--ti-text-muted)] hover:text-[var(--ti-text)]"}`}
          aria-current={tab === "advanced" ? "page" : undefined}
          onclick={() => { tab = "advanced"; diagnosticsOpen = false; }}
        >Help</button>
      </nav>
    </header>

    {#if tab === "meeting"}
      <article class="ti-panel overflow-hidden">
        <header class="flex items-start justify-between gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <div>
            <h3 class="m-0 text-[15px] font-semibold tracking-[-0.015em]">Meeting audio</h3>
            <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Choose the microphone you speak into and the sound device TranslateIT listens to.</p>
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
                <option value={deviceId(device)}>{device.name}{device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("microphone")}
                <option value={currentDevice("microphone")}>{currentDevice("microphone")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[11.5px] leading-5 text-[var(--ti-text-soft)]">The microphone you speak into.</small>
          </label>

          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Sound from your meeting</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving} value={currentDevice("meeting-sound")} onchange={(event) => void changeDevice("meeting-sound", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.output_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)}>{device.name}{device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("meeting-sound")}
                <option value={currentDevice("meeting-sound")}>{currentDevice("meeting-sound")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Choose where you hear the call if you want TranslateIT to translate incoming speech.</small>
          </label>
        </div>

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
              {snapshot.readiness.meetingRouteReady
                ? "Select this microphone in Zoom, Google Meet, Discord, or your calling app."
                : "TranslateIT microphone isn't ready yet. Run Repair Setup before starting Meeting translation."}
            </p>
          </div>
          {#if !snapshot.readiness.meetingRouteReady}
            <StatusBadge
              label={snapshot.readiness.level === "unavailable" ? "Unavailable" : "Setup Needed"}
              tone={snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
            />
          {/if}
        </div>

        <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <p class="m-0 min-w-0 flex-1 text-[12px] leading-5 text-[var(--ti-text-muted)]" aria-live="polite">{meetingResourcesLocked ? meetingResourceLockMessage : deviceMessage}</p>
          <div class="ti-action-row shrink-0">
            <button type="button" class="ti-button ti-button-secondary" disabled={micTestBlockedByMeeting || micTestBusy || setupBusy || deviceSaving} onclick={() => void onMicTest()}>{micTestBusy ? "Working..." : snapshot.readiness.recording || micTestOwnsResources ? "Stop Mic Test" : "Mic Test"}</button>
            <button type="button" class="ti-button ti-button-secondary" disabled={meetingResourcesLocked || setupBusy || deviceSaving} onclick={() => void runSetupRepair()}>{setupBusy ? "Fixing..." : "Repair Setup"}</button>
          </div>
        </footer>
      </article>
    {:else if tab === "translation"}
      <TranslationPreferences {settings} {onSettingsChange} {onNotice} />
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
    {:else}
      <SettingsDiagnostics
        {snapshot}
        {setupBusy}
        {diagnosticsLoading}
        onRefreshDiagnostics={refreshDiagnostics}
        {onSetupAction}
        onBack={() => { diagnosticsOpen = false; }}
      />
    {/if}
  </div>
</section>
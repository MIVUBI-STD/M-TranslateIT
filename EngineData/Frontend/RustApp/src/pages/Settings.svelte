<script lang="ts">
  import { ArrowLeft, Bug, RefreshCw } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../app/bridge/runtimeApi";
  import {
    parseWorkerCapabilities,
    runtimeProductFacade,
    type ProductAudioDeviceKind,
    type ProductRuntimeSnapshot,
    type ProductSetupAction,
  } from "../app/bridge/runtimeProductFacade";
  import { sanitizeDiagnosticText } from "../app/shared/diagnosticPrivacy";
  import { deviceId } from "../app/shared/state";
  import type { AudioDeviceListReport, RuntimeSettings } from "../app/shared/types";
  import MeetingPerformanceDiagnostics from "../components/settings/MeetingPerformanceDiagnostics.svelte";
  import TranslationPreferences from "../components/settings/TranslationPreferences.svelte";
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

  const meetingResourcesLocked = $derived(snapshot.meeting.hasSession);
  const micTestOwnsResources = $derived(snapshot.meeting.hasSession && !snapshot.meeting.applicationOwned);
  const micTestBlockedByMeeting = $derived(snapshot.meeting.applicationOwned);
  const meetingResourceLockMessage = "Stop Translation or Mic Test before changing meeting audio or running Repair Setup.";
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim() || "Meeting microphone not configured",
  );

  const workerDiagnostics = $derived(parseWorkerCapabilities(snapshot.workerStatus));
  const helperDiagnosticMessage = $derived(
    sanitizeDiagnosticText(snapshot.helper?.message, "Refresh status to check the local worker."),
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
        <p class="ti-page-copy">Meeting audio, setup checks, and diagnostics.</p>
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
        >Advanced</button>
      </nav>
    </header>

    {#if tab === "meeting"}
      <article class="ti-panel overflow-hidden">
        <header class="flex items-start justify-between gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <div>
            <h3 class="m-0 text-[15px] font-semibold tracking-[-0.015em]">Meeting audio</h3>
            <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Choose what you speak into and where you hear the meeting.</p>
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
            <span class="ti-field-label">Meeting sound</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving} value={currentDevice("meeting-sound")} onchange={(event) => void changeDevice("meeting-sound", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.output_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)}>{device.name}{device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("meeting-sound")}
                <option value={currentDevice("meeting-sound")}>{currentDevice("meeting-sound")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Optional incoming English → Indonesian text listens here.</small>
          </label>
        </div>

        <div class="flex items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
          <div class="min-w-0">
            <span class="ti-field-label">Meeting microphone</span>
            <strong class="mt-1.5 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
            <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
              {snapshot.readiness.meetingRouteReady
                ? "Choose this exact microphone inside your meeting app."
                : "Meeting microphone isn't ready yet. Run Repair Setup before starting Meeting translation."}
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
            <button type="button" class="ti-button ti-button-secondary" disabled={meetingResourcesLocked || setupBusy || deviceSaving} onclick={() => void runSetupRepair()}>{setupBusy ? "Repairing..." : "Repair Setup"}</button>
          </div>
        </footer>
      </article>
    {:else if tab === "translation"}
      <TranslationPreferences {settings} {onSettingsChange} {onNotice} />
    {:else if !diagnosticsOpen}
      <article class="ti-panel overflow-hidden">
        <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <h3 class="m-0 text-[15px] font-semibold">Setup health</h3>
          <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Use Diagnostics only when you need technical detail.</p>
        </header>

        <div class="divide-y divide-[var(--ti-border)]">
          <StatusRow
            label="Text translation"
            value="Indonesian ↔ English"
            detail="Standalone text translation."
            status={snapshot.readiness.textReady ? "Ready" : snapshot.readiness.textStatus}
            tone={snapshot.readiness.textReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
          />
          <StatusRow
            label="Meeting translation"
            value="Indonesian voice → English voice"
            detail="Required microphone and meeting output."
            status={snapshot.readiness.meetingReady ? "Ready" : snapshot.readiness.meetingStatus}
            tone={snapshot.readiness.meetingReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
          />
        </div>

        <footer class="flex justify-end border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={diagnosticsLoading} onclick={() => void refreshDiagnostics()}>{diagnosticsLoading ? "Opening..." : "Open Diagnostics"}</button>
        </footer>
      </article>
    {:else}
      <section class="grid gap-4">
        <header class="ti-page-header">
          <div>
            <h3 class="m-0 text-xl font-semibold tracking-[-0.02em]">Diagnostics</h3>
            <p class="mb-0 mt-1.5 text-[12.5px] text-[var(--ti-text-muted)]">Technical status for troubleshooting.</p>
          </div>
          <button type="button" class="ti-button ti-button-secondary" onclick={() => { diagnosticsOpen = false; }}><ArrowLeft size={15} /> Back</button>
        </header>

        <article class="ti-panel p-5">
          <div class="grid grid-cols-[repeat(3,minmax(0,1fr))] gap-3">
            <div class="ti-state-card"><span class="ti-field-label">Worker</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.state ?? "Not checked"}</strong></div>
            <div class="ti-state-card"><span class="ti-field-label">Outbound provider</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.provider_ready ? "Available" : "Setup Needed"}</strong></div>
            <div class="ti-state-card"><span class="ti-field-label">Execution device</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.cuda_ready ? "CUDA" : snapshot.helper?.degraded_mode ? "CPU / degraded" : "Not verified"}</strong></div>
          </div>

          <p class="mb-0 mt-4 text-[12px] leading-5 text-[var(--ti-text-muted)]">{helperDiagnosticMessage}</p>

          <div class="ti-action-row mt-4">
            <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading} onclick={() => void refreshDiagnostics()}><RefreshCw size={15} /> {diagnosticsLoading || setupBusy ? "Refreshing..." : "Refresh Status"}</button>
            <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading} onclick={() => void onSetupAction("verify-models")}><Bug size={15} /> Verify Models</button>
          </div>
        </article>

        <article class="ti-panel p-5">
          <div>
            <span class="ti-field-label">Loaded local runtimes</span>
            <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">AI execution truth</h4>
          </div>
          <div class="mt-4 grid grid-cols-[repeat(3,minmax(0,1fr))] gap-3">
            <div class="ti-state-card">
              <span class="ti-field-label">ASR</span>
              <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.asrDisplay}</strong>
            </div>
            <div class="ti-state-card">
              <span class="ti-field-label">Translation</span>
              <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.translationDisplay}</strong>
            </div>
            <div class="ti-state-card">
              <span class="ti-field-label">Meeting voice</span>
              <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.voiceDisplay}</strong>
            </div>
          </div>
          <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Selected execution: {workerDiagnostics.executionDisplay}. Refresh after Meeting Start to see the loaded runtime state.</p>
        </article>

        <MeetingPerformanceDiagnostics status={snapshot.meetingSession} />

        <article class="ti-panel p-5">
          <div class="flex items-end justify-between gap-5">
            <div>
              <span class="ti-field-label">Troubleshooting</span>
              <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Recent command errors</h4>
            </div>
            <span class="ti-pill">{runtimeApi.getCommandErrors().length} recent</span>
          </div>
          <div class="mt-4 grid gap-2">
            {#if runtimeApi.getCommandErrors().length === 0}
              <p class="m-0 text-[12px] text-[var(--ti-text-muted)]">No recent frontend/Tauri command failures.</p>
            {:else}
              {#each runtimeApi.getCommandErrors() as error (`${error.occurred_at}-${error.command}`)}
                <div class="ti-subtle-card px-4 py-3">
                  <strong class="text-[11px]">{error.command}</strong>
                  <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--ti-text-muted)]">{error.message}</p>
                </div>
              {/each}
            {/if}
          </div>
        </article>
      </section>
    {/if}
  </div>
</section>
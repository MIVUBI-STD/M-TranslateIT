<script lang="ts">
  import { onMount } from "svelte";
  import { myVoiceBuildApi } from "../app/bridge/myVoiceBuildApi";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../app/bridge/runtimeApi";
  import {
    runtimeProductFacade,
    type ProductRuntimeSnapshot,
  } from "../app/bridge/runtimeProductFacade";
  import {
    persistedSetupCheckpoint,
    safeSetupResumeStep,
    setupCheckpoint,
    type SetupStep,
  } from "../app/runtime/setupFlow";
  import { cloneSettings, compact, deviceId } from "../app/shared/state";
  import type { AudioDeviceListReport, RuntimeSettings } from "../app/shared/types";
  import SetupNavigation from "../components/setup/SetupNavigation.svelte";
  import StatusRow from "../components/ui/StatusRow.svelte";

  type SetupState = "new" | "deferred" | "completed";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";

  let {
    initialSettings,
    onComplete,
    onOpenMyVoice,
  }: {
    initialSettings: RuntimeSettings;
    onComplete: (settings: RuntimeSettings) => void | Promise<void>;
    onOpenMyVoice: (settings: RuntimeSettings) => void | Promise<void>;
  } = $props();

  let settings = $state<RuntimeSettings>((() => cloneSettings(initialSettings))());
  let snapshot = $state<ProductRuntimeSnapshot | null>(null);
  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let devices = $state<AudioDeviceListReport | null>(null);
  let step = $state<SetupStep>((() => setupCheckpoint(settings.meeting_setup_checkpoint))());
  let busy = $state(false);
  let message = $state("");
  let selectedMicrophone = $state((() => String(settings.audio.input_device_id ?? ""))());
  let builtinPendingId = $state<BuiltinVoiceId | null>(null);

  const meetingVoiceReady = $derived(snapshot?.readiness.approvedVoiceReady ?? null);

  function microphoneDevices() {
    return devices?.input_devices ?? [];
  }

  function currentMicrophoneId(): string {
    return String(settings.audio.input_device_id ?? "").trim();
  }

  function savedMicrophoneMissing(): boolean {
    const current = currentMicrophoneId();
    return Boolean(current && devices && !microphoneDevices().some((device) => deviceId(device) === current));
  }

  function currentMicrophone(): string {
    return compact(snapshot?.inputStatus?.selected_device_name ?? settings.audio.input_device_id, "Windows Default");
  }

  function currentMeetingMicrophone(): string {
    return compact(routeStatus?.selected_input_device, "Meeting microphone not configured");
  }

  function currentMeetingSound(): string {
    return compact(settings.audio.output_device_id, "Windows Default");
  }

  function builtinLabel(voiceId: BuiltinVoiceId): string {
    return voiceId === "MaleVoice" ? "Built-in Male" : "Built-in Female";
  }

  async function refreshSnapshot(): Promise<void> {
    try {
      const [nextSnapshot, nextRouteStatus] = await Promise.all([
        runtimeProductFacade.loadProductRuntimeSnapshot(),
        runtimeApi.getVirtualMicRouteStatus(),
      ]);
      snapshot = nextSnapshot;
      routeStatus = nextRouteStatus;
      settings = cloneSettings(nextSnapshot.settings);
      selectedMicrophone = String(settings.audio.input_device_id ?? "");
    } catch {
      routeStatus = null;
      message = "Couldn't check setup. Try again.";
    }
  }

  async function refreshDevices(): Promise<void> {
    try {
      devices = await runtimeProductFacade.loadProductAudioDevices();
      if (!devices.ok && !message) message = "Microphones are unavailable right now. Try again.";
    } catch {
      devices = null;
      if (!message) message = "Microphones are unavailable right now. Try again.";
    }
  }

  async function initialize(): Promise<void> {
    if (step > 1) {
      await refreshSnapshot();
      step = safeSetupResumeStep(step, {
        microphoneReady: snapshot?.readiness.microphoneReady === true,
        meetingRouteReady: snapshot?.readiness.meetingRouteReady === true,
      });
      if (step === 2) await refreshDevices();
    }
  }

  async function persistSetupFact(state: SetupState, nextCheckpoint: SetupStep): Promise<boolean> {
    const candidate = cloneSettings(settings);
    candidate.meeting_setup_state = state;
    const currentCheckpoint = setupCheckpoint(candidate.meeting_setup_checkpoint);
    const effectiveStep = Math.max(currentCheckpoint, nextCheckpoint) as SetupStep;
    candidate.meeting_setup_checkpoint = persistedSetupCheckpoint(effectiveStep);
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) throw new Error("setup save failed");
      const savedSettings = await runtimeApi.loadSettings();
      settings = cloneSettings(savedSettings ?? candidate);
      return true;
    } catch {
      message = "Couldn't save setup progress. Try again.";
      return false;
    }
  }

  async function advance(nextStep: SetupStep): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving...";
    const saved = await persistSetupFact("new", nextStep);
    if (saved) {
      step = nextStep;
      message = "";
      await refreshSnapshot();
      if (nextStep === 2) await refreshDevices();
    }
    busy = false;
  }

  function goBack(): void {
    if (busy || step <= 1) return;
    step = Math.max(1, step - 1) as SetupStep;
    message = "";
    if (step === 2) void refreshDevices();
  }

  async function deferSetup(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving...";
    const saved = await persistSetupFact("deferred", step);
    busy = false;
    if (saved) await onComplete(settings);
  }

  async function saveSelectedMicrophone(): Promise<void> {
    if (busy) return;
    const candidate = selectedMicrophone.trim() || null;

    busy = true;
    message = "Checking microphone...";
    try {
      const result = await runtimeProductFacade.selectProductAudioDevice("microphone", candidate, settings);
      settings = cloneSettings(result.settings);
      message = result.message;
      if (result.ok) {
        selectedMicrophone = String(settings.audio.input_device_id ?? "");
        await refreshDevices();
        await refreshSnapshot();
      }
    } catch {
      message = "The microphone wasn't changed. Try again.";
    } finally {
      busy = false;
    }
  }

  async function repairSetup(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Repairing setup...";
    try {
      const result = await runtimeProductFacade.runProductRecoveryAction("fix-setup");
      await refreshSnapshot();
      message = compact(result, "Setup check finished.");
    } catch {
      message = "Couldn't repair setup. Try again.";
    } finally {
      busy = false;
    }
  }

  async function refreshReadiness(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Refreshing status...";
    try {
      if (step === 4) await runtimeProductFacade.runProductSetupAction("check-readiness");
      await refreshSnapshot();
      message = meetingVoiceReady && snapshot?.readiness.meetingReady
        ? "Everything needed for Meeting translation is ready."
        : "Setup still needs attention.";
    } catch {
      message = "Couldn't refresh setup status. Try again.";
    } finally {
      busy = false;
    }
  }

  async function selectBuiltin(voiceId: BuiltinVoiceId, confirmed = false): Promise<void> {
    if (busy) return;
    busy = true;
    message = `Selecting ${builtinLabel(voiceId)}...`;
    try {
      const result = await myVoiceBuildApi.selectBuiltin(voiceId, confirmed);
      if (result.state === "approval_required") {
        builtinPendingId = voiceId;
        message = `Confirm before replacing the current Meeting voice with ${builtinLabel(voiceId)}.`;
        return;
      }
      builtinPendingId = null;
      message = result.message;
      if (result.ok) {
        await runtimeProductFacade.runProductSetupAction("check-readiness");
        await refreshSnapshot();
        message = `${builtinLabel(voiceId)} is selected.`;
      }
    } catch {
      message = "Couldn't change the Meeting voice. Try again or check Diagnostics.";
    } finally {
      busy = false;
    }
  }

  async function confirmPendingBuiltin(): Promise<void> {
    const voiceId = builtinPendingId;
    if (!voiceId) return;
    await selectBuiltin(voiceId, true);
  }

  async function openMyVoice(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving setup before opening My Voice...";
    const saved = await persistSetupFact("deferred", 4);
    busy = false;
    if (saved) await onOpenMyVoice(settings);
  }

  async function completeSetup(): Promise<void> {
    if (busy || !meetingVoiceReady || !snapshot?.readiness.meetingReady) return;
    busy = true;
    message = "Saving...";
    const saved = await persistSetupFact("completed", 4);
    busy = false;
    if (saved) await onComplete(settings);
  }

  function selectValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  onMount(() => {
    void initialize();
  });
</script>

<main class="grid min-h-screen place-items-center overflow-y-auto bg-[var(--ti-bg)] px-8 py-10" aria-label="TranslateIT Setup">
  <section class="ti-panel w-full max-w-[900px] overflow-hidden">
    <header class="flex items-center gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-7 py-5">
      <div class="grid size-10 place-items-center rounded-[12px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-base font-bold">T</div>
      <div>
        <strong class="block text-sm font-semibold">TranslateIT</strong>
        <span class="mt-0.5 block text-xs text-[var(--ti-text-muted)]">Meeting setup</span>
      </div>
      <div class="ml-auto min-w-44" role="progressbar" aria-label="Setup progress" aria-valuemin="1" aria-valuemax="4" aria-valuenow={step}>
        <div class="text-[11px] font-semibold text-[var(--ti-text-muted)]">Step {step} of 4</div>
        <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-[var(--ti-border)]">
          <div class="h-full rounded-full bg-[var(--ti-accent)]" style={`width:${step * 25}%`}></div>
        </div>
      </div>
    </header>

    <section class="mx-auto grid max-w-[720px] gap-6 px-8 py-9">
      {#if step === 1}
        <div>
          <span class="ti-kicker">Welcome</span>
          <h1 class="ti-page-title text-[2.2rem]">Set up meeting translation</h1>
          <p class="ti-page-copy">Set up the three things required for Indonesian → English meeting voice. Optional incoming translation is configured later in Settings.</p>
          <div class="mt-6 grid grid-cols-3 gap-3">
            <div class="ti-subtle-card p-4"><span class="ti-field-label">1</span><strong class="mt-1 block text-sm font-semibold">Your microphone</strong></div>
            <div class="ti-subtle-card p-4"><span class="ti-field-label">2</span><strong class="mt-1 block text-sm font-semibold">Meeting microphone</strong></div>
            <div class="ti-subtle-card p-4"><span class="ti-field-label">3</span><strong class="mt-1 block text-sm font-semibold">Meeting voice</strong></div>
          </div>
        </div>
      {:else if step === 2}
        <div>
          <span class="ti-kicker">Microphone</span>
          <h1 class="ti-page-title">Which microphone do you use?</h1>
          <p class="ti-page-copy">Choose the microphone you normally speak into during calls. TranslateIT checks it before you continue.</p>
        </div>
        <div class="grid grid-cols-[1fr_auto] items-end gap-3">
          <label class="grid gap-2">
            <span class="ti-field-label">Microphone</span>
            <select class="ti-field min-h-11 px-3" disabled={busy} value={selectedMicrophone} onchange={(event) => { selectedMicrophone = selectValue(event); }}>
              <option value="">Windows Default</option>
              {#each microphoneDevices() as device (deviceId(device))}
                <option value={deviceId(device)}>{device.name}{device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if savedMicrophoneMissing()}
                <option value={currentMicrophoneId()}>{currentMicrophoneId()} · unavailable</option>
              {/if}
            </select>
          </label>
          <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void saveSelectedMicrophone()}>Use This Microphone</button>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="Selected microphone"
            value={currentMicrophone()}
            detail="The microphone you speak into."
            status={snapshot?.readiness.microphoneReady ? "Ready" : snapshot?.readiness.level === "checking" ? "Checking" : "Setup Needed"}
            tone={snapshot?.readiness.microphoneReady ? "good" : snapshot?.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>
      {:else if step === 3}
        <div>
          <span class="ti-kicker">Meeting microphone</span>
          <h1 class="ti-page-title">Set your meeting microphone</h1>
          <p class="ti-page-copy">Choose the TranslateIT microphone below inside your meeting app before the call.</p>
        </div>
        <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] p-5">
          <span class="ti-field-label">In Zoom, Meet, Discord, or another meeting app</span>
          <strong class="mt-2 block text-base font-semibold">Microphone → {currentMeetingMicrophone()}</strong>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="TranslateIT Meeting Microphone"
            value={currentMeetingMicrophone()}
            detail={snapshot?.readiness.meetingRouteReady ? "Available on Windows. Make sure your meeting app is using this device." : "TranslateIT needs to repair or configure the meeting output route."}
            status={snapshot?.readiness.meetingRouteReady ? "Available" : snapshot?.readiness.level === "checking" ? "Checking" : "Setup Needed"}
            tone={snapshot?.readiness.meetingRouteReady ? "good" : snapshot?.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>
      {:else}
        <div>
          <span class="ti-kicker">Meeting voice</span>
          <h1 class="ti-page-title">{meetingVoiceReady && snapshot?.readiness.meetingReady ? "You're ready to translate." : "Choose your English Meeting voice."}</h1>
          <p class="ti-page-copy">Built-in voices work immediately. My Voice is an optional personalized replacement.</p>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <button type="button" class="ti-subtle-card p-5 text-left" disabled={busy} onclick={() => void selectBuiltin("MaleVoice")}>
            <span class="ti-field-label">Ready now</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Male</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use as your English Meeting voice.</span>
          </button>
          <button type="button" class="ti-subtle-card p-5 text-left" disabled={busy} onclick={() => void selectBuiltin("FemaleVoice")}>
            <span class="ti-field-label">Ready now</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Female</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use as your English Meeting voice.</span>
          </button>
        </div>

        {#if builtinPendingId}
          <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
            <strong class="text-sm font-semibold">Replace the current Meeting voice?</strong>
            <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{builtinLabel(builtinPendingId)} will replace the voice currently selected for Meeting.</p>
            <div class="ti-action-row mt-4">
              <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => { builtinPendingId = null; message = ""; }}>Cancel</button>
              <button type="button" class="ti-button" disabled={busy} onclick={() => void confirmPendingBuiltin()}>Replace Voice</button>
            </div>
          </div>
        {/if}

        <div class="ti-subtle-card divide-y divide-[var(--ti-border)] overflow-hidden">
          <StatusRow label="Microphone" value={currentMicrophone()} status={snapshot?.readiness.microphoneReady ? "Ready" : "Setup Needed"} tone={snapshot?.readiness.microphoneReady ? "good" : "warning"} />
          <StatusRow label="Meeting voice" value={meetingVoiceReady ? "Selected Meeting voice" : meetingVoiceReady === null ? "Checking Meeting voice" : "Choose Built-in Male/Female or create My Voice"} status={meetingVoiceReady ? "Ready" : meetingVoiceReady === null ? "Checking" : "Setup Needed"} tone={meetingVoiceReady ? "good" : meetingVoiceReady === null ? "neutral" : "warning"} />
          <StatusRow label="Meeting microphone" value={currentMeetingMicrophone()} detail="Availability does not confirm your meeting app selection." status={snapshot?.readiness.meetingRouteReady ? "Available" : "Setup Needed"} tone={snapshot?.readiness.meetingRouteReady ? "good" : "warning"} />
          <StatusRow label="Incoming translation" value={`English → Indonesian text · ${currentMeetingSound()}`} detail="Optional. Change Meeting Sound later in Settings." status="Optional" tone="neutral" />
        </div>

        <button type="button" class="ti-button ti-button-secondary justify-self-start" disabled={busy} onclick={() => void openMyVoice()}>Create My Voice instead</button>
      {/if}

      {#if message}
        <p class="m-0 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3 text-sm leading-6 text-[var(--ti-text-muted)]" aria-live="polite">{message}</p>
      {/if}

      <SetupNavigation
        {step}
        {busy}
        microphoneReady={snapshot?.readiness.microphoneReady === true}
        meetingRouteReady={snapshot?.readiness.meetingRouteReady === true}
        {meetingVoiceReady}
        meetingReady={snapshot?.readiness.meetingReady === true}
        onBack={goBack}
        onDefer={deferSetup}
        onAdvance={advance}
        onRepair={repairSetup}
        onRefresh={refreshReadiness}
        onComplete={completeSetup}
      />
    </section>
  </section>
</main>

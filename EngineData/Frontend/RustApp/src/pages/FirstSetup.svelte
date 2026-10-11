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
  import SetupVoiceChoice from "../components/setup/SetupVoiceChoice.svelte";
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
  // An unsaved device choice is not the verified microphone in the runtime snapshot.
  const microphoneSelectionPending = $derived(
    selectedMicrophone.trim() !== String(settings.audio.input_device_id ?? "").trim(),
  );

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
    return compact(
      routeStatus?.selected_input_device,
      snapshot?.readiness.meetingRouteReady ? "Device name unavailable — check Settings" : "TranslateIT microphone not ready",
    );
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
    message = "Fixing setup...";
    try {
      const result = await runtimeProductFacade.runProductRecoveryAction("fix-setup");
      await refreshSnapshot();
      message = compact(result, "Setup check finished.");
    } catch {
      message = "Couldn't fix setup. Try again.";
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
        : "A few setup items still need attention.";
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
        message = `Confirm before replacing the current Voice others hear with ${builtinLabel(voiceId)}.`;
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
      message = "Couldn't change the Voice others hear. Try again.";
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
    message = "Saving your choices...";
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

<main class="flex h-screen min-h-0 items-start justify-center overflow-y-auto bg-[var(--ti-bg)] px-8 py-10" aria-label="TranslateIT Setup">
  <section class="ti-panel w-full max-w-[900px] shrink-0 overflow-hidden">
    <header class="flex items-center gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-7 py-5">
      <div class="grid size-10 place-items-center rounded-[12px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-base font-bold">T</div>
      <div>
        <strong class="block text-sm font-semibold">TranslateIT</strong>
        <span class="mt-0.5 block text-xs text-[var(--ti-text-muted)]">Quick setup</span>
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
          <h1 class="ti-page-title text-[2.2rem]">Get ready for meeting translation</h1>
          <p class="ti-page-copy">TranslateIT only needs a few choices before your first call. You can change them later in Settings.</p>
          <div class="mt-6 grid grid-cols-3 gap-3">
            <div class="ti-subtle-card p-4"><span class="ti-field-label">1</span><strong class="mt-1 block text-sm font-semibold">The mic you speak into</strong></div>
            <div class="ti-subtle-card p-4"><span class="ti-field-label">2</span><strong class="mt-1 block text-sm font-semibold">Mic used by your meeting app</strong></div>
            <div class="ti-subtle-card p-4"><span class="ti-field-label">3</span><strong class="mt-1 block text-sm font-semibold">Voice others will hear</strong></div>
          </div>
        </div>
      {:else if step === 2}
        <div>
          <span class="ti-kicker">Microphone</span>
          <h1 class="ti-page-title">Which microphone do you speak into?</h1>
          <p class="ti-page-copy">Choose the microphone you speak into. If unsure, keep Windows Default. Save your choice before continuing.</p>
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
          <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void saveSelectedMicrophone()}>Use this microphone</button>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="Selected microphone"
            value={currentMicrophone()}
            detail={microphoneSelectionPending ? "The saved microphone is shown here. Save your new choice before continuing." : "The microphone currently saved for your calls."}
            status={microphoneSelectionPending ? "Not saved" : snapshot?.readiness.microphoneReady ? "Ready" : snapshot?.readiness.level === "checking" ? "Checking" : "Setup Needed"}
            tone={microphoneSelectionPending ? "warning" : snapshot?.readiness.microphoneReady ? "good" : snapshot?.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>
        {#if microphoneSelectionPending}
          <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]" role="status">This microphone choice has not been saved or checked. Select “Use this microphone” first.</p>
        {:else if snapshot && !snapshot.readiness.microphoneReady}
          <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]" role="status">This microphone is not ready. Check its connection, then choose “Use this microphone” again.</p>
        {/if}
      {:else if step === 3}
        <div>
          <span class="ti-kicker">Meeting app</span>
          <h1 class="ti-page-title">Choose TranslateIT as your call microphone</h1>
          <p class="ti-page-copy">In the audio settings of Zoom, Google Meet, Discord, or your calling app, select the microphone shown below.</p>
        </div>
        <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] p-5">
          <span class="ti-field-label">Choose this as the microphone in your meeting app</span>
          <strong class="mt-2 block break-words text-base font-semibold">{currentMeetingMicrophone()}</strong>
          <p class="mb-0 mt-2 text-xs leading-5 text-[var(--ti-text-muted)]">Check Again verifies only the TranslateIT device in Windows. It cannot confirm which microphone your calling app selected.</p>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="TranslateIT microphone"
            value={currentMeetingMicrophone()}
            detail={snapshot?.readiness.meetingRouteReady ? "Available in Windows. Select it separately in your calling app." : "TranslateIT needs to finish setting up this microphone."}
            status={snapshot?.readiness.meetingRouteReady ? "Available" : snapshot?.readiness.level === "checking" ? "Checking" : "Setup Needed"}
            tone={snapshot?.readiness.meetingRouteReady ? "good" : snapshot?.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>
      {:else}
        <SetupVoiceChoice
          {snapshot}
          {meetingVoiceReady}
          {busy}
          {builtinPendingId}
          microphone={currentMicrophone()}
          meetingMicrophone={currentMeetingMicrophone()}
          meetingSound={currentMeetingSound()}
          onSelect={(voiceId) => selectBuiltin(voiceId)}
          onConfirm={confirmPendingBuiltin}
          onCancel={() => { builtinPendingId = null; message = ""; }}
          onOpenMyVoice={openMyVoice}
        />
      {/if}

      {#if message}
        <p class="m-0 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3 text-sm leading-6 text-[var(--ti-text-muted)]" aria-live="polite">{message}</p>
      {/if}

      <SetupNavigation
        {step}
        {busy}
        microphoneReady={snapshot?.readiness.microphoneReady === true}
        {microphoneSelectionPending}
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

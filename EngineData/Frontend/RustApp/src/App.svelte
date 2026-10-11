<script lang="ts">
  import { onMount } from "svelte";
  import { appUpdateApi } from "./app/update/appUpdateApi";
  import { runtimeApi, type MeetingSessionStatus } from "./app/bridge/runtimeApi";
  import {
    applicationRuntimeApi,
    type ApplicationSnapshot,
  } from "./app/bridge/applicationRuntimeApi";
  import {
    runtimeProductFacade,
    type ProductSetupAction,
  } from "./app/bridge/runtimeProductFacade";
  import { startMeetingRuntimeMonitors } from "./app/runtime/reliabilityMonitor";
import { subscribeOverlayReady } from "./app/runtime/translationOverlayRuntime";
import { discardLegacyOverlayCaption } from "./app/runtime/translationOverlayState";
  import { startVoiceBuildRuntimeSync } from "./app/runtime/voiceBuildRuntimeSync";
  import { installNativeCloseGuard } from "./app/runtime/nativeCloseRuntime";
  import {
    createCloseController,
    type CloseControllerState,
  } from "./app/runtime/closeController";
  import { compact, defaultSettings } from "./app/shared/state";
  import {
    createApplicationController,
    type ApplicationControllerState,
  } from "./app/runtime/applicationController";
  import {
    createMeetingLiveController,
    type MeetingLiveViewState,
  } from "./app/runtime/meetingLiveController";
  import type {
    AppRoute,
    RuntimeSettings,
  } from "./app/shared/types";
  import type { UtilityDestination } from "./app/runtime/productCommandRegistry";

  import NativeCloseDialog from "./components/runtime/NativeCloseDialog.svelte";

  import FirstSetup from "./pages/FirstSetup.svelte";
  import ProductWorkspace from "./components/layout/ProductWorkspace.svelte";
  import StartupStatus from "./components/layout/StartupStatus.svelte";





  const applicationController = createApplicationController(defaultSettings());
  const meetingLiveController = createMeetingLiveController();
  const closeController = createCloseController();
  let applicationState = $state<ApplicationControllerState>(applicationController.read());
  let meetingViewState = $state<MeetingLiveViewState>(meetingLiveController.read());
  let closeState = $state<CloseControllerState>(closeController.read());

  let booting = $state(true);
  let route=$state<AppRoute>("meeting");
  let utilityTarget = $state<UtilityDestination | null>(null);
  let utilityRequest = $state(0);
  let notice=$state("Getting TranslateIT ready...");
  let meetingActionBusy=$state(false);
  let setupActionBusy=$state(false);
  let micTestBusy=$state(false);
  let myVoiceRecording = $state(false);


  const snapshot = $derived(applicationState.snapshot);
  const setupRequired = $derived(applicationState.setupRequired);
  const setupSettings = $derived(applicationState.setupSettings);
  const meetingStatus = $derived(snapshot?.meetingSession ?? null);

  const presence = $derived(
    snapshot?.meeting.live
      ? "Live"
      : snapshot?.readiness.level === "unavailable"
        ? "Unavailable"
        : snapshot?.readiness.meetingReady
          ? "Ready"
          : snapshot?.readiness.textReady
            ? "Degraded"
            : snapshot?.readiness.level === "blocked"
              ? "Setup Needed"
              : "Checking",
  );

  const closePrimaryLabel = $derived(closeState.action === "retry" ? "Try Again" : closeState.busy ? "Stopping..." : "Stop & Close");
  // Stable boolean dependencies avoid restarting listeners on each refreshed session snapshot.
  const meetingMonitorsActive = $derived(
    !booting && !setupRequired &&
    (snapshot?.resources.owner_kind === "meeting" || closeState.closeAfterExistingStop),
  );
  const meetingReliabilityEnabled = $derived(
    Boolean(snapshot?.meeting.applicationOwned && snapshot.meeting.hasSession),
  );

  function setNotice(message: string): void {
    notice = compact(message, "Status unavailable.", 220);
  }

  function navigate(next: AppRoute): void {
    if (myVoiceRecording && next !== "my-voice") {
      setNotice("Stop the current My Voice recording before leaving My Voice.");
      return;
    }
    route = next;
    utilityTarget = null;
  }

  function openUtility(target: UtilityDestination): void {
    if (myVoiceRecording) {
      setNotice("Stop the current My Voice recording before opening another tool.");
      return;
    }
    utilityTarget = target;
    utilityRequest += 1;
    route = target.startsWith("meeting-") ? "meeting" : "settings";
  }

  function applyMeetingStatus(status: MeetingSessionStatus, preferredNotice?: string): void {
    const ownershipChanged = applicationController.applyMeetingSession(status);
    applicationState = applicationController.read();
    if (preferredNotice) setNotice(preferredNotice);
    if (ownershipChanged) void refreshSnapshot();
  }

  function applyRefreshResult(
    result: Awaited<ReturnType<typeof applicationController.refresh>>,
    preferredNotice?: string,
  ): void {
    applicationState = applicationController.read();
    if (!result.applied) return;

    if (result.setupRequired) {
      meetingViewState = meetingLiveController.reset();
      setNotice(preferredNotice ?? "Continue Meeting setup to use voice translation.");
      return;
    }

    const next = result.snapshot;
    if (!next) return;
    if (!next.meeting.hasSession || result.previousSessionId !== result.nextSessionId) {
      meetingViewState = meetingLiveController.reset();
    }
    setNotice(preferredNotice ?? (next.meeting.hasSession ? next.meeting.message : next.readiness.summary));
  }

  async function refreshSnapshot(preferredNotice?: string, knownSettings?: RuntimeSettings): Promise<void> {
    try {
      applyRefreshResult(
        await applicationController.refresh(knownSettings),
        preferredNotice,
      );
    } catch {
      setNotice("TranslateIT couldn't refresh its status. Try again.");
    }
  }

  async function refreshFromApplicationEvent(
    application: ApplicationSnapshot,
    reason: string,
  ): Promise<void> {
    try {
      applyRefreshResult(
        await applicationController.refreshFromApplication(application, reason),
      );
    } catch {
      setNotice("TranslateIT couldn't reconcile its runtime event. Try again.");
    }
  }

  async function applySettings(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
  }

  async function syncMeetingVoice(message?: string): Promise<void> {
    setNotice(message ?? "Meeting voice updated.");
  }

  async function finishFirstSetup(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
    await refreshSnapshot("Setup saved.", next);
    route = "meeting";
  }

  async function openMyVoiceFromSetup(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
    await refreshSnapshot("Choose a Meeting voice before starting translation.", next);
    route = "my-voice";
  }

  async function handleMeetingAction(): Promise<void> {
    if (meetingActionBusy || !snapshot) return;
    const meeting = snapshot.meeting;
    const action = meeting.canStop ? "stop" : "start";
    if (action === "start" && !meeting.canStart) {
      setNotice(snapshot.readiness.nextAction ?? meeting.message);
      return;
    }
    if (action === "stop" && !meeting.canStop) {
      setNotice(meeting.message);
      return;
    }

    meetingActionBusy = true;
    setNotice(action === "start" ? "Starting translation..." : "Stopping translation...");
    try {
      const result = await runtimeProductFacade.runProductMeetingAction(action);
      const resultNotice = !result.ok && result.state === "frontend_bridge_error"
        ? "Meeting status is unknown. Reconnect and retry."
        : result.ok
          ? action === "start" ? "Translation is live." : "Translation stopped."
          : action === "start"
            ? "Translation couldn't start. Check the setup items and try again."
            : "Translation couldn't stop. Try again.";
      applyMeetingStatus(result.status, resultNotice);
      if (!result.status.has_session || action === "start") {
        meetingViewState = meetingLiveController.reset();
      }
    } catch {
      setNotice("That action couldn't be completed. Try again.");
      await refreshSnapshot();
    } finally {
      meetingActionBusy = false;
    }
  }

  async function runSetupAction(action: ProductSetupAction): Promise<void> {
    if (setupActionBusy) return;
    setupActionBusy = true;
    setNotice(action === "verify-models" ? "Checking translation files..." : "Checking setup...");
    try {
      const message = await runtimeProductFacade.runProductSetupAction(action);
      await refreshSnapshot(message);
    } catch {
      setNotice("The setup check could not finish. Open Help or try again.");
    } finally {
      setupActionBusy = false;
    }
  }

  async function fixSetup(): Promise<void> {
    if (setupActionBusy) return;
    setupActionBusy = true;
    setNotice("Checking setup...");
    try {
      const message = await runtimeProductFacade.runProductRecoveryAction("fix-setup");
      await refreshSnapshot(message);
    } catch {
      setNotice("Repair Setup could not finish. Check your devices and try again.");
    } finally {
      setupActionBusy = false;
    }
  }

  async function toggleMicTest(): Promise<void> {
    if (micTestBusy || !snapshot) return;
    const ownerKind = snapshot.resources.owner_kind;
    const micTestOwnsRuntime = ownerKind === "mic_test";

    if (snapshot.resources.audio_locked && !micTestOwnsRuntime) {
      const message = ownerKind === "meeting"
        ? "Stop Meeting translation before using Mic Test."
        : ownerKind === "voice_recording"
          ? "Stop My Voice recording before using Mic Test."
          : "Finish the current audio action before using Mic Test.";
      setNotice(message);
      return;
    }

    if (!snapshot.resources.microphone_available && !micTestOwnsRuntime) {
      setNotice("Microphone setup isn't ready yet.");
      return;
    }

    micTestBusy = true;
    try {
      const result = await applicationRuntimeApi.dispatchIntent(
        micTestOwnsRuntime ? "stop_mic_test" : "start_mic_test",
      );
      await refreshSnapshot(result.ok
        ? (micTestOwnsRuntime ? "Mic Test stopped." : "Mic Test started.")
        : result.message || "Mic Test couldn't be completed. Try again.");
    } catch {
      setNotice("Mic Test couldn't be completed. Check your microphone and try again.");
    } finally {
      micTestBusy = false;
    }
  }

  async function pollMeeting(forceTurns = false): Promise<void> {
    try {
      meetingViewState = await meetingLiveController.reconcile(
        {
          ownerKind: snapshot?.resources.owner_kind ?? null,
          booting,
          setupRequired,
          closeAfterExistingStop: closeState.closeAfterExistingStop,
        },
        {
          onStatus: (status) => applyMeetingStatus(status),
          onNotice: setNotice,
          onStoppedForClose: async () => {
            closeState = await closeController.closeWindow();
          },
        },
        forceTurns,
      );
    } catch {
      setNotice("Couldn't refresh the meeting display. Translation status is unknown; open Help if this continues.");
    }
  }

  async function inspectNativeCloseRequest(): Promise<void> {
    closeState = await closeController.inspect();
  }

  async function handleCloseDialogPrimary(): Promise<void> {
    closeState = await closeController.primary();
  }

  function keepApplicationOpen(): void {
    closeState = closeController.keepOpen();
  }

  onMount(() => {
    void appUpdateApi.checkAtStartupOnce().catch(() => null);

    let disposed = false;
    let unlistenClose: (() => void) | null = null;
    let unlistenApplicationRuntime: (() => void) | null = null;
    let unlistenOverlayReady: (() => void) | null = null;
    discardLegacyOverlayCaption();
    const stopVoiceBuildRuntimeSync = startVoiceBuildRuntimeSync((message) => refreshSnapshot(message));
    const boot = async () => {
      try {
        const loadedSettings = await runtimeApi.loadSettings();
        if (!loadedSettings) {
          setNotice("TranslateIT can't reach the desktop runtime yet. Try again when it is available.");
          return;
        }
        applicationController.applySettings(loadedSettings);
        applicationState = applicationController.read();
        if (loadedSettings.meeting_setup_state !== "new") {
          await refreshSnapshot(undefined, loadedSettings);
        }
      } catch {
        if (!disposed) setNotice("Could not load your settings. Select Try Again to check the desktop runtime.");
      } finally {
        if (!disposed) booting = false;
      }
    };

    const installCloseGuard = async () => {
      try {
        const stop = await installNativeCloseGuard(inspectNativeCloseRequest);
        if (disposed) {
          stop();
          return;
        }
        unlistenClose = stop;
      } catch {}
    };

    const installApplicationRuntimeSync = async () => {
      try {
        const stop = await applicationRuntimeApi.subscribe((event) => {
          if (disposed || event.snapshot.revision <= 0) return;
          void refreshFromApplicationEvent(event.snapshot, event.reason);
        });
        if (disposed) {
          stop();
          return;
        }
        unlistenApplicationRuntime = stop;
      } catch {}
    };



    void boot();
    void installCloseGuard();
    void installApplicationRuntimeSync();
    void subscribeOverlayReady().then((stop) => {
      if (disposed) stop();
      else unlistenOverlayReady = stop;
    }).catch(() => { /* Live event delivery remains available. */ });

    return () => {
      disposed = true;
      unlistenClose?.();
      unlistenApplicationRuntime?.();
      unlistenOverlayReady?.();
      stopVoiceBuildRuntimeSync();
    };
  });

  $effect(() =>
    meetingMonitorsActive
      ? startMeetingRuntimeMonitors(
          pollMeeting,
          setNotice,
          meetingReliabilityEnabled,
        )
      : undefined,
  );
</script>

{#if booting}
  <StartupStatus loading message={notice} onRetry={() => void refreshSnapshot()} />
{:else if setupRequired}
  <FirstSetup initialSettings={setupSettings} onComplete={finishFirstSetup} onOpenMyVoice={openMyVoiceFromSetup} />
{:else if snapshot}
  <ProductWorkspace
    {snapshot}
    {route}
    {presence}
    {notice}
    {myVoiceRecording}
    {meetingStatus}
    meetingTurns={meetingViewState.turns}
    {utilityTarget}
    {utilityRequest}
    {meetingActionBusy}
    {setupActionBusy}
    {micTestBusy}
    onNavigate={navigate}
    onOpenUtility={openUtility}
    onToggleMeeting={handleMeetingAction}
    onRefresh={refreshSnapshot}
    onFixSetup={fixSetup}
    onSettingsChange={applySettings}
    onSetupAction={runSetupAction}
    onMicTest={toggleMicTest}
    onRecordingChange={(recording) => { myVoiceRecording = recording; }}
    onMeetingVoiceChanged={syncMeetingVoice}
    onNotice={setNotice}
  />
{:else}
  <StartupStatus loading={false} message={notice} onRetry={() => { setNotice("Checking again..."); void refreshSnapshot(); }} />
{/if}

<NativeCloseDialog
  open={closeState.open}
  title={closeState.title}
  message={closeState.message}
  action={closeState.action}
  busy={closeState.busy}
  primaryLabel={closePrimaryLabel}
  onOpenChange={(open) => {
    if (!open) closeState = closeController.keepOpen();
  }}
  onKeepOpen={keepApplicationOpen}
  onPrimary={handleCloseDialogPrimary}
/>

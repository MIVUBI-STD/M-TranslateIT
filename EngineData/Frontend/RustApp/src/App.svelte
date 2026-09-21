<script lang="ts">
  import { onMount } from "svelte";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "./app/bridge/runtimeApi";
  import { applicationRuntimeApi } from "./app/bridge/applicationRuntimeApi";
  import {
    runtimeProductFacade,
    type ProductSetupAction,
  } from "./app/bridge/runtimeProductFacade";
  import { type CloseDialogAction, type CloseVerdict } from "./app/runtime/closePolicy";
  import { readMeetingPoll } from "./app/runtime/meetingPoll";
  import { startMeetingRuntimeMonitors } from "./app/runtime/reliabilityMonitor";
  import { publishLatestMeetingOverlay } from "./app/runtime/translationOverlayRuntime";
  import {
    destroyTranslateItWindows,
    installNativeCloseGuard,
    resolveNativeCloseVerdict,
    stopAndResolveNativeClose,
  } from "./app/runtime/nativeCloseRuntime";
  import { cloneSettings, compact, defaultSettings } from "./app/shared/state";
  import {
    createApplicationController,
    type ApplicationControllerState,
  } from "./app/runtime/applicationController";
  import type {
    AppRoute,
    RuntimeSettings,
  } from "./app/shared/types";
  import Sidebar from "./components/layout/Sidebar.svelte";
  import NativeCloseDialog from "./components/runtime/NativeCloseDialog.svelte";
  import AppHeader from "./components/layout/AppHeader.svelte";
  import FirstSetup from "./pages/FirstSetup.svelte";
  import Meeting from "./pages/Meeting.svelte";
  import MyVoice from "./pages/MyVoice.svelte";
  import Settings from "./pages/Settings.svelte";
  import Text from "./pages/Text.svelte";

  const applicationController = createApplicationController(defaultSettings());
  let applicationState = $state<ApplicationControllerState>(applicationController.read());

  let booting = $state(true);
  let route=$state<AppRoute>("meeting");
  let notice=$state("Getting TranslateIT ready...");
  let meetingActionBusy=$state(false);
  let setupActionBusy=$state(false);
  let micTestBusy=$state(false);
  let myVoiceRecording = $state(false);
  let meetingTurns = $state<MeetingCommittedTurnsSnapshot | null>(null);

  let closeDialogOpen = $state(false);
  let closeDialogTitle = $state("Close TranslateIT?");
  let closeDialogMessage = $state("");
  let closeDialogAction = $state<CloseDialogAction>(null);
  let stopAndCloseBusy = $state(false);
  let closeAfterExistingStop = $state(false);
  let meetingPollInFlight = false;
  let closeCheckInFlight = false;
  let lastTranscriptStatusKey = "";
  let lastOverlayMeetingRevision = "";
  let runtimeStateRevision=0;

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

  const closePrimaryLabel = $derived(closeDialogAction === "retry" ? "Try Again" : stopAndCloseBusy ? "Stopping..." : "Stop & Close");

  function setNotice(message: string): void {
    notice = compact(message, "Status unavailable.", 220);
  }

  function navigate(next: AppRoute): void {
    if (myVoiceRecording && next !== "my-voice") {
      setNotice("Stop the current My Voice recording before leaving My Voice.");
      return;
    }
    route = next;
  }

  function applyMeetingStatus(status: MeetingSessionStatus, preferredNotice?: string): void {
    applicationController.applyMeetingSession(status);
    applicationState = applicationController.read();
    if (preferredNotice) setNotice(preferredNotice);
  }

  async function refreshSnapshot(preferredNotice?: string, knownSettings?: RuntimeSettings): Promise<void> {
    try {
      const result = await applicationController.refresh(knownSettings);
      applicationState = applicationController.read();
      if (!result.applied) return;

      if (result.setupRequired) {
        meetingTurns = null;
        lastTranscriptStatusKey = "";
        lastOverlayMeetingRevision = "";
        setNotice(preferredNotice ?? "Continue Meeting setup to use voice translation.");
        return;
      }

      const next = result.snapshot;
      if (!next) return;
      if (!next.meeting.hasSession || result.previousSessionId !== result.nextSessionId) {
        meetingTurns = null;
        lastTranscriptStatusKey = "";
        lastOverlayMeetingRevision = "";
      }
      setNotice(preferredNotice ?? (next.meeting.hasSession ? next.meeting.message : next.readiness.summary));
    } catch {
      setNotice("TranslateIT couldn't refresh its status. Try again.");
    }
  }

  async function applySettings(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
  }

  async function syncMeetingVoice(message?: string): Promise<void> {
    await refreshSnapshot(message ?? "Meeting voice updated.");
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
    runtimeStateRevision += 1;
    setNotice(action === "start" ? "Starting translation..." : "Stopping translation...");
    try {
      const result = await runtimeProductFacade.runProductMeetingAction(action);
      runtimeStateRevision += 1;
      const resultNotice = result.ok
        ? action === "start" ? "Translation is live." : "Translation stopped."
        : action === "start"
          ? "Translation couldn't start. Check the setup items and try again."
          : "Translation couldn't stop. Try again.";
      applyMeetingStatus(result.status, resultNotice);
      if (!result.status.has_session || action === "start") {
        meetingTurns = null;
        lastTranscriptStatusKey = "";
        lastOverlayMeetingRevision = "";
      }
    } catch {
      runtimeStateRevision += 1;
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

  async function closeNativeWindow(): Promise<void> {
    closeAfterExistingStop = false;
    try {
      await destroyTranslateItWindows();
      closeDialogOpen = false;
    } catch {
      showCloseDialog("Couldn't close TranslateIT", "The floating caption couldn't close safely. TranslateIT will stay open.", "retry");
    }
  }

  async function pollMeeting(): Promise<void> {
    if (meetingPollInFlight || booting || setupRequired) return;
    if (snapshot?.resources.owner_kind !== "meeting" && !closeAfterExistingStop) return;

    meetingPollInFlight = true;
    const pollRevision = runtimeStateRevision;
    try {
      const result = await readMeetingPoll(meetingTurns, lastTranscriptStatusKey);
      if (!result || pollRevision !== runtimeStateRevision) return;
      applyMeetingStatus(result.status);
      meetingTurns = result.turns;
      lastTranscriptStatusKey = result.transcriptStatusKey;
      lastOverlayMeetingRevision = await publishLatestMeetingOverlay(result.turns, lastOverlayMeetingRevision);
      if (result.unavailable) {
        setNotice("Meeting translation is temporarily unavailable.");
      } else if (!result.status.has_session && closeAfterExistingStop) {
        await closeNativeWindow();
      }
    } finally {
      meetingPollInFlight = false;
    }
  }

  function showCloseDialog(title: string, message: string, action: CloseDialogAction): void {
    closeDialogTitle = title;
    closeDialogMessage = compact(message, "Status unavailable.", 220);
    closeDialogAction = action;
    closeDialogOpen = true;
  }

  function keepApplicationOpen(): void {
    closeAfterExistingStop = false;
    stopAndCloseBusy = false;
    closeDialogAction = null;
    closeDialogOpen = false;
  }

  function applyCloseVerdict(verdict: CloseVerdict): void {
    if (verdict.kind === "destroy") {
      void closeNativeWindow();
      return;
    }
    if (verdict.kind === "stop-and-close") {
      showCloseDialog(
        "Translation is still running",
        "Stop & Close ends Meeting translation safely before closing TranslateIT.",
        "stop",
      );
      return;
    }
    if (verdict.kind === "wait-for-stop") {
      closeAfterExistingStop = true;
      showCloseDialog(verdict.title, verdict.message, null);
      return;
    }
    showCloseDialog(verdict.title, verdict.message, verdict.action);
  }

  async function inspectNativeCloseRequest(): Promise<void> {
    if (closeCheckInFlight || stopAndCloseBusy) return;
    closeCheckInFlight = true;
    try {
      applyCloseVerdict(await resolveNativeCloseVerdict());
    } catch {
      showCloseDialog(
        "Couldn't close TranslateIT",
        "TranslateIT couldn't confirm that it is safe to close. Keep the app open and try again.",
        "retry",
      );
    } finally {
      closeCheckInFlight = false;
    }
  }

  async function handleStopAndClose(): Promise<void> {
    if (stopAndCloseBusy) return;
    stopAndCloseBusy = true;
    try {
      applyCloseVerdict(await stopAndResolveNativeClose());
    } catch {
      showCloseDialog(
        "Couldn't close TranslateIT",
        "The app will stay open. Try again in a moment.",
        "retry",
      );
    } finally {
      stopAndCloseBusy = false;
    }
  }

  async function handleCloseDialogPrimary(): Promise<void> {
    if (closeDialogAction === "retry") {
      closeDialogOpen = false;
      await inspectNativeCloseRequest();
      return;
    }
    if (closeDialogAction === "stop") await handleStopAndClose();
  }

  onMount(() => {
    let disposed = false;
    let unlistenClose: (() => void) | null = null;
    let unlistenApplicationRuntime: (() => void) | null = null;
    const boot = async () => {
      try {
        const loadedSettings = await runtimeApi.loadSettings();
        if (!loadedSettings) {
          setNotice("TranslateIT can't reach the desktop runtime yet. Try again when it is available.");
          return;
        }
        applicationController.applySettings(loadedSettings);
        applicationState = applicationController.read();
        if (loadedSettings.meeting_setup_state === "new") {
          applicationState = {
            ...applicationState,
            setupRequired: true,
            applicationState.runtimeLoaded: false,
            snapshot: null,
          };
        } else {
          await refreshSnapshot(undefined, loadedSettings);
        }
      } finally {
        if (!disposed) booting = false;
      }
    };

    const installCloseGuard = async () => {
      try {
        unlistenClose = await installNativeCloseGuard(inspectNativeCloseRequest);
      } catch {}
    };

    const installApplicationRuntimeSync = async () => {
      try {
        unlistenApplicationRuntime = await applicationRuntimeApi.subscribe((event) => {
          if (disposed || event.snapshot.revision <= 0) return;
          void refreshSnapshot();
        });
      } catch {}
    };

    void boot();
    void installCloseGuard();
    void installApplicationRuntimeSync();

    return () => {
      disposed = true;
      unlistenClose?.();
      unlistenApplicationRuntime?.();
    };
  });

  $effect(() =>
    !booting && !setupRequired && (snapshot?.resources.owner_kind === "meeting" || closeAfterExistingStop)
      ? startMeetingRuntimeMonitors(
          pollMeeting,
          setNotice,
          Boolean(snapshot?.meeting.applicationOwned && snapshot.meeting.hasSession),
        )
      : undefined,
  );
</script>

{#if booting}
  <main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="w-full max-w-[560px] text-center">
      <div class="mx-auto grid size-12 place-items-center rounded-[14px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-lg font-bold">T</div>
      <h1 class="mb-0 mt-5 text-2xl font-semibold tracking-[-0.03em]">Getting TranslateIT ready</h1>
      <p class="mb-0 mt-2 text-sm leading-6 text-[var(--ti-text-muted)]">Checking your saved setup.</p>
    </section>
  </main>
{:else if setupRequired}
  <FirstSetup initialSettings={setupSettings} onComplete={finishFirstSetup} onOpenMyVoice={openMyVoiceFromSetup} />
{:else if snapshot}
  <main class="flex h-screen min-h-0 bg-[var(--ti-bg)]">
    <Sidebar active={route} {presence} onNavigate={navigate} />

    <section class="flex min-w-0 flex-1 flex-col">
      <AppHeader
        {snapshot}
        {route}
        {notice}
        {myVoiceRecording}
        onNavigate={navigate}
        onToggleMeeting={handleMeetingAction}
        onNotice={setNotice}
      />

      <div class="min-h-0 flex-1 overflow-y-auto">
        {#if route === "meeting"}
          <Meeting
            {snapshot}
            {meetingStatus}
            {meetingTurns}
            actionBusy={meetingActionBusy}
            onMeetingAction={handleMeetingAction}
            onRefresh={refreshSnapshot}
            onFixSetup={fixSetup}
            onOpenMyVoice={() => navigate("my-voice")}
          />
        {:else if route === "text"}
          <Text
            settings={snapshot.settings}
            textStatus={snapshot.readiness.textStatus}
            onSettingsChange={applySettings}
            onNotice={setNotice}
          />
        {:else if route === "my-voice"}
          <MyVoice
            onNotice={setNotice}
            onMeetingVoiceChanged={syncMeetingVoice}
            onRecordingChange={(recording) => { myVoiceRecording = recording; }}
          />
        {:else}
          <Settings
            {snapshot}
            settings={snapshot.settings}
            setupBusy={setupActionBusy}
            {micTestBusy}
            onSettingsChange={applySettings}
            onRefresh={refreshSnapshot}
            onSetupAction={runSetupAction}
            onFixSetup={fixSetup}
            onMicTest={toggleMicTest}
            onNotice={setNotice}
          />
        {/if}
      </div>
    </section>
  </main>
{:else}
  <main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="ti-panel w-full max-w-[560px] p-7">
      <span class="ti-kicker">TranslateIT</span>
      <h1 class="mb-0 mt-3 text-2xl font-semibold">TranslateIT isn't ready yet</h1>
      <p class="mb-0 mt-3 text-sm leading-6 text-[var(--ti-text-muted)]">{notice}</p>
      <button type="button" class="ti-button mt-5" onclick={() => { setNotice("Checking again..."); void refreshSnapshot(); }}>Try Again</button>
    </section>
  </main>
{/if}

<NativeCloseDialog bind:open={closeDialogOpen} title={closeDialogTitle} message={closeDialogMessage} action={closeDialogAction} busy={stopAndCloseBusy} primaryLabel={closePrimaryLabel} onKeepOpen={keepApplicationOpen} onPrimary={handleCloseDialogPrimary} />

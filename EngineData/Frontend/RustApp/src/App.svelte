<script lang="ts">
  import { onMount } from "svelte";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "./app/bridge/runtimeApi";
  import {
    mapProductMeetingState,
    mapProductReadiness,
    runtimeProductFacade,
    type ProductRuntimeSnapshot,
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
  import type {
    AppRoute,
    HelperBridgeStatus,
    HelperBridgeWorkerResponse,
    InputPreparationStatus,
    RuntimeSettings,
  } from "./app/shared/types";
  import Sidebar from "./components/layout/Sidebar.svelte";
  import NativeCloseDialog from "./components/runtime/NativeCloseDialog.svelte";
  import ProductivityActions from "./components/runtime/ProductivityActions.svelte";
  import FirstSetup from "./pages/FirstSetup.svelte";
  import Meeting from "./pages/Meeting.svelte";
  import MyVoice from "./pages/MyVoice.svelte";
  import Settings from "./pages/Settings.svelte";
  import Text from "./pages/Text.svelte";

  let booting = $state(true), setupRequired = $state(false);
  let setupSettings=$state<RuntimeSettings>(defaultSettings());
  let route=$state<AppRoute>("meeting");
  let notice=$state("Getting TranslateIT ready...");
  let meetingActionBusy=$state(false);
  let setupActionBusy=$state(false);
  let micTestBusy=$state(false);
  let myVoiceRecording = $state(false);
  let runtimeLoaded = $state(false);
  let runtimeSettings = $state<RuntimeSettings>(defaultSettings());
  let helperStatus = $state<HelperBridgeStatus | null>(null);
  let workerStatus = $state<HelperBridgeWorkerResponse | null>(null);
  let inputStatus = $state<InputPreparationStatus | null>(null);
  let approvedVoiceReady = $state<boolean | null>(null);
  let meetingStatus = $state<MeetingSessionStatus | null>(null);
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

  const snapshot = $derived.by<ProductRuntimeSnapshot | null>(() => {
    if (!runtimeLoaded) return null;
    return {
      settings: runtimeSettings,
      helper: helperStatus,
      workerStatus,
      inputStatus,
      meetingSession: meetingStatus,
      meeting: mapProductMeetingState(meetingStatus),
      readiness: mapProductReadiness({
        settings: runtimeSettings,
        helper: helperStatus,
        workerStatus,
        inputStatus,
        meetingSession: meetingStatus,
        approvedVoiceReady,
      }),
    };
  });

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

  const routeTitle = $derived(
    route === "meeting" ? "Meeting" : route === "text" ? "Text" : route === "my-voice" ? "My Voice" : "Settings",
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
    meetingStatus = status;
    if (preferredNotice) setNotice(preferredNotice);
  }

  async function refreshSnapshot(preferredNotice?: string, knownSettings?: RuntimeSettings): Promise<void> {
    const requestRevision = ++runtimeStateRevision;
    try {
      const previousSessionId = snapshot?.meeting.sessionId ?? null;
      const next = await runtimeProductFacade.loadProductRuntimeSnapshot(knownSettings);
      if (requestRevision !== runtimeStateRevision) return;
      runtimeSettings = cloneSettings(next.settings);
      setupSettings = cloneSettings(next.settings);
      helperStatus = next.helper;
      workerStatus = next.workerStatus;
      inputStatus = next.inputStatus;
      approvedVoiceReady = next.readiness.approvedVoiceReady;

      if (next.settings.meeting_setup_state === "new") {
        setupRequired = true;
        runtimeLoaded = false;
        meetingStatus = null;
        meetingTurns = null;
        lastTranscriptStatusKey = "";
        lastOverlayMeetingRevision = "";
        setNotice(preferredNotice ?? "Continue Meeting setup to use voice translation.");
        return;
      }

      setupRequired = false;
      runtimeLoaded = true;
      meetingStatus = next.meetingSession;
      if (!next.meeting.hasSession || previousSessionId !== next.meeting.sessionId) {
        meetingTurns = null;
        lastTranscriptStatusKey = "";
        lastOverlayMeetingRevision = "";
      }
      setNotice(preferredNotice ?? (next.meeting.hasSession ? next.meeting.message : next.readiness.summary));
    } catch {
      if (requestRevision === runtimeStateRevision) {
        setNotice("TranslateIT couldn't refresh its status. Try again.");
      }
    }
  }

  async function applySettings(next: RuntimeSettings): Promise<void> {
    runtimeStateRevision += 1;
    const nextSettings = cloneSettings(next);
    runtimeSettings = nextSettings;
    setupSettings = nextSettings;
  }

  async function syncMeetingVoice(message?: string): Promise<void> {
    await refreshSnapshot(message ?? "Meeting voice updated.");
  }

  async function finishFirstSetup(next: RuntimeSettings): Promise<void> {
    setupSettings = cloneSettings(next);
    setupRequired = false;
    await refreshSnapshot("Setup saved.", next);
    route = "meeting";
  }

  async function openMyVoiceFromSetup(next: RuntimeSettings): Promise<void> {
    setupSettings = cloneSettings(next);
    setupRequired = false;
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
    if (myVoiceRecording) {
      setNotice("Stop the current My Voice recording before using Mic Test.");
      return;
    }
    if (snapshot.meeting.applicationOwned) {
      setNotice(snapshot.meeting.live
        ? "Stop Meeting translation before using Mic Test."
        : "Mic Test is unavailable while Meeting audio is in use.");
      return;
    }
    const micTestOwnsRuntime = snapshot.meeting.hasSession && !snapshot.meeting.applicationOwned;
    if (!snapshot.readiness.microphoneReady && !snapshot.readiness.recording && !micTestOwnsRuntime) {
      setNotice("Microphone setup isn't ready yet.");
      return;
    }

    micTestBusy = true;
    const wasRecording = snapshot.readiness.recording || micTestOwnsRuntime;
    try {
      const result = wasRecording ? await runtimeApi.stopCapture() : await runtimeApi.startCapture();
      await refreshSnapshot(result.ok
        ? (wasRecording ? "Mic Test stopped." : "Mic Test started.")
        : "Mic Test couldn't be completed. Try again.");
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
    if (!snapshot?.meeting.hasSession && !closeAfterExistingStop) return;

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
    const boot = async () => {
      try {
        const loadedSettings = await runtimeApi.loadSettings();
        if (!loadedSettings) {
          setNotice("TranslateIT can't reach the desktop runtime yet. Try again when it is available.");
          return;
        }
        setupSettings = cloneSettings(loadedSettings);
        setupRequired = setupSettings.meeting_setup_state === "new";
        if (!setupRequired) await refreshSnapshot(undefined, loadedSettings);
      } finally {
        if (!disposed) booting = false;
      }
    };

    const installCloseGuard = async () => {
      try {
        unlistenClose = await installNativeCloseGuard(inspectNativeCloseRequest);
      } catch {}
    };

    void boot();
    void installCloseGuard();

    return () => {
      disposed = true;
      unlistenClose?.();
    };
  });

  $effect(() =>
    !booting && !setupRequired && (snapshot?.meeting.hasSession || closeAfterExistingStop)
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
      <header class="ti-appbar flex min-h-16 shrink-0 items-center gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-bg)] px-6">
        <div class="min-w-0 shrink-0">
          <div class="flex items-baseline gap-2.5">
            <strong class="text-[13.5px] font-semibold tracking-[-0.01em]">{routeTitle}</strong>
          </div>
        </div>

        <p class="m-0 min-w-0 flex-1 truncate text-right text-[11.5px] text-[var(--ti-text-muted)]" aria-live="polite" title={notice}>{notice}</p>

        <ProductivityActions
          {snapshot}
          {route}
          meetingBusy={snapshot.meeting.hasSession}
          {myVoiceRecording}
          onNavigate={navigate}
          onToggleMeeting={handleMeetingAction}
          onNotice={setNotice}
        />

        {#if snapshot.meeting.applicationOwned && snapshot.meeting.hasSession}
          <button
            type="button"
            class="flex shrink-0 items-center gap-2 rounded-full border border-[var(--ti-success-border)] bg-[var(--ti-success-surface)] px-3 py-1.5 text-left"
            aria-label="Open active Meeting translation"
            onclick={() => { navigate("meeting"); }}
          >
            <span class="size-2 rounded-full bg-[var(--ti-success)]" aria-hidden="true"></span>
            <strong class="text-[11.5px] font-semibold text-[var(--ti-success)]">{snapshot.meeting.label}</strong>
          </button>
        {/if}
      </header>

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

<script lang="ts">
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../app/bridge/runtimeApi";
  import type { AppRoute } from "../app/shared/types";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { meetingControlState } from "../app/bridge/productMeetingState";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingSessionReview from "../components/meeting/MeetingSessionReview.svelte";
  import MeetingTranscriptExport from "../components/meeting/MeetingTranscriptExport.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";

  let { snapshot, meetingStatus, meetingTurns, actionBusy = false, onMeetingAction, onRefresh, onNavigate }: {
    snapshot: ProductRuntimeSnapshot;
    meetingStatus: MeetingSessionStatus | null;
    meetingTurns: MeetingCommittedTurnsSnapshot | null;
    actionBusy?: boolean;
    onMeetingAction: (source?: "original" | "translateit") => void | Promise<void>;
    onRefresh: (preferredNotice?: string) => void | Promise<void>;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  let savedDeletionRevision = $state(0);
  let resumeBusy = $state(false);
  let sourceChanging = $state(false);
  let currentSource = $state<"original" | "translateit">("translateit");
  $effect(() => {
    // Both Meeting and Quick Access show the same Rust-owned pre-Start choice
    // and the same confirmed Live selection.
    if (!sourceChanging && meetingStatus) {
      currentSource = meetingStatus.audio_source === "original" ? "original" : "translateit";
    }
  });
  async function selectSource(source: "original" | "translateit"): Promise<void> {
    // Keep Stop/Retry Stop as the only control while cleanup is pending.
    if (sourceChanging || currentSource === source || !sourceChangeAllowed) return;
    sourceChanging = true;
    try {
      const result = meeting.live
        ? await runtimeApi.selectMeetingAudioSource(source)
        : !meeting.hasSession && meeting.lifecycle === "idle"
          ? await runtimeApi.selectMeetingPreferredAudioSource(source)
          : null;
      if (result?.ok) {
        currentSource = result.source;
        await onRefresh(result.message);
      } else {
        await onRefresh(result?.message ?? "Could not change microphone source. Check Meeting status.");
      }
    } catch {
      await onRefresh("Could not change microphone source. Your previous selection was kept.");
    } finally { sourceChanging = false; }
  }
  const meeting = $derived(snapshot.meeting);
  const sourceChangeAllowed = $derived(meeting.live || (!meeting.hasSession && meeting.lifecycle === "idle"));
  const needsSetup = $derived(!meeting.hasSession && !meeting.canStop
    && ["new", "deferred"].includes(snapshot.settings.meeting_setup_state));
  const needsVoice = $derived(!needsSetup && !meeting.hasSession && !meeting.canStop
    && snapshot.readiness.approvedVoiceReady === false);

  async function continueSetup(): Promise<void> {
    if (resumeBusy || actionBusy || meeting.hasSession || meeting.canStop) return;
    resumeBusy = true;
    try {
      const settings = {
        ...snapshot.settings,
        meeting_setup_state: "new",
        audio: { ...snapshot.settings.audio },
      };
      const save = await runtimeApi.saveSettings(settings);
      if (!save.ok) throw new Error("Setup could not be resumed");
      await onRefresh("Continue the initial Meeting setup.");
    } catch {
      await onRefresh("Couldn't resume setup. Check Settings and try again.");
    } finally {
      resumeBusy = false;
    }
  }

  function handlePrimary(): void {
    if (!actionEnabled) return;
    if (currentSource === "original" && !meeting.hasSession && !meeting.canStop) {
      if (originalAvailable) void onMeetingAction("original");
      else onNavigate("settings");
      return;
    }
    if (needsSetup) { void continueSetup(); return; }
    if (needsVoice) { onNavigate("my-voice"); return; }
    if (needsSettings) { onNavigate("settings"); return; }
    void onMeetingAction(currentSource);
  }
  const primaryControl = $derived(meetingControlState(
    meeting, snapshot.readiness.approvedVoiceReady, snapshot.settings.meeting_setup_state, actionBusy,
  ));
  const needsSettings = $derived(!needsSetup && !needsVoice && !meeting.hasSession && !meeting.canStop
    && !primaryControl.enabled && !meeting.busy && !actionBusy);
  const originalAvailable = $derived(
    currentSource === "original" && !meeting.hasSession && meeting.lifecycle === "idle"
    && snapshot.readiness.microphoneReady === true
    && snapshot.readiness.meetingRouteReady === true
  );
  const actionEnabled = $derived(!resumeBusy && !sourceChanging && !meeting.busy && !actionBusy
    && (primaryControl.enabled || originalAvailable || needsSetup || needsVoice || needsSettings));
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.live));
  const primaryLabel = $derived(
    primaryControl.action === "stop"
      ? meeting.lifecycle === "cleanup_incomplete" ? primaryControl.label : "Stop Microphone"
      : currentSource === "original" && originalAvailable ? "Start Original Microphone"
      : currentSource === "original" && !meeting.hasSession ? "Check Audio Setup"
      : meeting.busy || actionBusy ? primaryControl.label
      : resumeBusy ? "Opening setup..."
      : needsSetup ? "Finish Setup"
      : needsVoice ? "Choose Voice"
      : needsSettings ? "Open Settings"
      : "Start Translation"
  );
  const statusMessage = $derived(
    meeting.canStop || meeting.hasSession
      ? meeting.live && meetingStatus?.muted
        ? "Microphone muted. No voice is being sent to the meeting."
        : meeting.live && meetingStatus?.audio_source === "original"
          ? "Original microphone is live. Audio bypasses translation and AI."
          : meeting.message
    : currentSource === "original"
      ? "Original microphone sends your own voice directly to the meeting."
    : snapshot.settings.meeting_setup_state === "deferred" || snapshot.settings.meeting_setup_state === "new"
      ? "Finish initial setup before starting."
    : snapshot.readiness.level === "unavailable"
      ? "Translation is unavailable. Check Help in Settings."
    : snapshot.readiness.approvedVoiceReady === false
      ? "Choose a voice in My Voice before starting."
    : !snapshot.readiness.microphoneReady
      ? "No microphone available. Check Voice & Audio in Settings."
    : !snapshot.readiness.meetingRouteReady
      ? "Meeting audio needs setup. Check Voice & Audio in Settings."
    : snapshot.readiness.meetingReady
      ? "Ready to translate. Start when your call is open."
      : "Checking translation readiness. If this persists, open Help in Settings."
  );
  const statusTone = $derived<"good" | "warning" | "danger" | "neutral">(
    meeting.lifecycle === "cleanup_incomplete" ? "warning"
      : meeting.live || snapshot.readiness.meetingReady && !meeting.hasSession && !needsSetup ? "good"
      : snapshot.readiness.level === "unavailable" ? "danger" : "neutral"
  );
</script>

<section class="ti-page ti-page-wide" style="margin-inline: 0; max-width: 1080px;">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Meeting</h2>
      <p class="ti-page-copy">Indonesian speech → English voice</p>
    </div>
  </header>

  <article class="ti-panel overflow-hidden">
    <div class="flex items-start justify-between gap-4 px-6 py-5">
      <div class="min-w-0">
        <strong class="text-[15px] font-semibold">{meeting.live && meetingStatus?.audio_source === "original" ? "Original microphone is on" : meeting.live ? "Translation session active" : "Meeting microphone"}</strong>
        <p class="mb-0 mt-2 text-[13px] leading-6 text-[var(--ti-text-muted)]" aria-live="polite">{statusMessage}</p>
      </div>
      <StatusBadge label={meeting.live ? "Live" : meeting.label} tone={statusTone} />
    </div>
    <footer class="flex justify-end border-t border-[var(--ti-border)] px-6 py-4" aria-label="Meeting main action">
      <button type="button" class={"ti-button min-w-44 " + (primaryControl.action === "stop" ? "ti-button-danger" : "")}
        disabled={!actionEnabled} onclick={handlePrimary}>{primaryLabel}</button>
    </footer>
    {#if sourceChangeAllowed && meetingStatus}
      <div class="border-t border-[var(--ti-border)] px-6 py-4">
        <label for="meeting-audio-source" class="ti-field-label">Microphone output</label>
        <select id="meeting-audio-source" class="ti-field mt-2 min-h-11 px-3"
          value={currentSource} disabled={sourceChanging || actionBusy || !sourceChangeAllowed}
          onchange={(event) => void selectSource(event.currentTarget.value === "original" ? "original" : "translateit")}
        >
          <option value="translateit">TranslateIT Audio</option>
          <option value="original">Original Microphone</option>
        </select>
        <p class="mb-0 mt-2 text-xs text-[var(--ti-text-muted)]">
          Both options use the same virtual microphone selected in your calling app.
        </p>
      </div>
    {/if}
    {#if activityVisible && meetingStatus && meetingStatus.audio_source !== "original"}
      <div class="border-t border-[var(--ti-border)] px-6 py-5">
        <MeetingActivity status={meetingStatus} turns={meetingTurns} />
      </div>
    {/if}
  </article>
  {#key savedDeletionRevision}
    <MeetingSessionReview sessionActive={meeting.hasSession} onNotice={(message) => void onRefresh(message)} />
  {/key}
  <MeetingTranscriptExport hasSession={meeting.hasSession}
    onNotice={(message) => void onRefresh(message)}
    onDeleted={() => { savedDeletionRevision += 1; }} />
</section>

<script lang="ts">
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../app/bridge/runtimeApi";
  import type { AppRoute } from "../app/shared/types";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { meetingControlState } from "../app/bridge/productMeetingState";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingSessionReview from "../components/meeting/MeetingSessionReview.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";

  let { snapshot, meetingStatus, meetingTurns, actionBusy = false, onMeetingAction, onRefresh, onNavigate }: {
    snapshot: ProductRuntimeSnapshot;
    meetingStatus: MeetingSessionStatus | null;
    meetingTurns: MeetingCommittedTurnsSnapshot | null;
    actionBusy?: boolean;
    onMeetingAction: () => void | Promise<void>;
    onRefresh: (preferredNotice?: string) => void | Promise<void>;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  let resumeBusy = $state(false);
  const meeting = $derived(snapshot.meeting);
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
    if (needsSetup) { void continueSetup(); return; }
    if (needsVoice) { onNavigate("my-voice"); return; }
    if (needsSettings) { onNavigate("settings"); return; }
    void onMeetingAction();
  }
  const primaryControl = $derived(meetingControlState(
    meeting, snapshot.readiness.approvedVoiceReady, snapshot.settings.meeting_setup_state, actionBusy,
  ));
  const needsSettings = $derived(!needsSetup && !needsVoice && !meeting.hasSession && !meeting.canStop
    && !primaryControl.enabled && !meeting.busy && !actionBusy);
  const actionEnabled = $derived(!resumeBusy && !meeting.busy && !actionBusy
    && (primaryControl.enabled || needsSetup || needsVoice || needsSettings));
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.live));
  const primaryLabel = $derived(
    primaryControl.action === "stop" ? primaryControl.label
      : meeting.busy || actionBusy ? primaryControl.label
      : resumeBusy ? "Opening setup..."
      : needsSetup ? "Finish Setup"
      : needsVoice ? "Choose Voice"
      : needsSettings ? "Open Settings"
      : "Start Translation"
  );
  const statusMessage = $derived(
    meeting.canStop || meeting.hasSession ? meeting.message
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
    meeting.live || snapshot.readiness.meetingReady && !meeting.hasSession && !needsSetup ? "good"
      : meeting.lifecycle === "cleanup_incomplete" ? "warning"
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
        <strong class="text-[15px] font-semibold">{meeting.live ? "Translation is on" : "Meeting translation"}</strong>
        <p class="mb-0 mt-2 text-[13px] leading-6 text-[var(--ti-text-muted)]" aria-live="polite">{statusMessage}</p>
      </div>
      <StatusBadge label={meeting.live ? "Live" : meeting.label} tone={statusTone} />
    </div>
    {#if activityVisible && meetingStatus}
      <div class="border-t border-[var(--ti-border)] px-6 py-5">
        <MeetingActivity status={meetingStatus} turns={meetingTurns} />
      </div>
    {/if}
    <footer class="flex justify-end border-t border-[var(--ti-border)] px-6 py-4">
      <button type="button" class={"ti-button min-w-44 " + (primaryControl.action === "stop" ? "ti-button-danger" : "")}
        disabled={!actionEnabled} onclick={handlePrimary}>{primaryLabel}</button>
    </footer>
  </article>
  <MeetingSessionReview sessionActive={meeting.hasSession} onNotice={(message) => void onRefresh(message)} />
</section>

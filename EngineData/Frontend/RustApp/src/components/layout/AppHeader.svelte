<script lang="ts">
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import type { UtilityDestination } from "../../app/runtime/productCommandRegistry";
  import { meetingControlState } from "../../app/bridge/productMeetingState";
  import ProductivityActions from "../runtime/ProductivityActions.svelte";

  let {
    snapshot,
    route,
    notice,
    myVoiceRecording,
    meetingActionBusy,
    onNavigate,
    onOpenUtility,
    onToggleMeeting,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    notice: string;
    myVoiceRecording: boolean;
    meetingActionBusy: boolean;
    onNavigate: (route: AppRoute) => void;
    onOpenUtility: (destination: UtilityDestination) => void;
    onToggleMeeting: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  const meetingControl = $derived(meetingControlState(
    snapshot.meeting,
    snapshot.readiness.approvedVoiceReady,
    snapshot.settings.meeting_setup_state,
    meetingActionBusy,
  ));
  const meetingNeedsAttention = $derived(snapshot.meeting.hasSession && !snapshot.meeting.live
    || snapshot.meeting.lifecycle === "cleanup_incomplete");

  const routeTitle = $derived(
    route === "meeting"
      ? "Meeting"
      : route === "text"
        ? "Text"
        : route === "my-voice"
          ? "My Voice"
          : "Settings",
  );
</script>

<header class="ti-appbar flex min-h-16 shrink-0 items-center gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-bg)] px-6">
  <div class="min-w-0 shrink-0">
    <div class="flex items-baseline gap-2.5">
      <strong class="text-[13.5px] font-semibold tracking-[-0.01em]">{routeTitle}</strong>
    </div>
  </div>

  <p class="m-0 min-w-0 flex-1 truncate text-right text-[11.5px] text-[var(--ti-text-muted)]" aria-live="polite">{notice}</p>

  <ProductivityActions
    {snapshot}
    {meetingControl}
    audioLocked={snapshot.resources.audio_locked}
    audioOwnerKind={snapshot.resources.owner_kind}
    voiceBuildActive={snapshot.voiceBuildActive}
    {myVoiceRecording}
    {onNavigate}
    {onOpenUtility}
    {onToggleMeeting}
    {onNotice}
  />

  {#if snapshot.meeting.applicationOwned && snapshot.meeting.hasSession || meetingNeedsAttention}
    <button
      type="button"
      class={`flex shrink-0 items-center gap-2 rounded-full border px-3 py-1.5 text-left ${meetingNeedsAttention
        ? "border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)]"
        : "border-[var(--ti-success-border)] bg-[var(--ti-success-surface)]"}`}
      aria-label="Open Meeting status"
      onclick={() => { onNavigate("meeting"); }}
    >
      <span class={`size-2 rounded-full ${meetingNeedsAttention ? "bg-[var(--ti-warning)]" : "bg-[var(--ti-success)]"}`} aria-hidden="true"></span>
      <strong class={`text-[11.5px] font-semibold ${meetingNeedsAttention ? "text-[var(--ti-warning)]" : "text-[var(--ti-success)]"}`}>{snapshot.meeting.label}</strong>
    </button>
  {/if}
  {#if route !== "meeting" && meetingControl.action === "stop" && meetingControl.enabled}
    <button type="button" class="ti-button ti-button-danger min-h-8 shrink-0" onclick={() => void onToggleMeeting()}>
      {meetingControl.label}
    </button>
  {/if}
</header>

<script lang="ts">
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import { Zap } from "@lucide/svelte";
  import UpdateAction from "../runtime/UpdateAction.svelte";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";

  let {
    snapshot,
    route,
    notice,
    myVoiceRecording,
    onNavigate,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    notice: string;
    myVoiceRecording: boolean;
    onNavigate: (route: AppRoute) => void;
    onNotice: (message: string) => void;
  } = $props();

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

  <button
    type="button"
    class="ti-button ti-button-secondary min-h-9 shrink-0 px-3 text-[12px]"
    aria-label="Open Quick Access"
    title="Quick Access (Ctrl+K)"
    onclick={() => void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."))}
  >
    <Zap size={15} />
    Quick Access
    <span class="ml-1 rounded bg-[var(--ti-surface-soft)] px-1.5 py-0.5 text-[10px] text-[var(--ti-text-muted)]" aria-hidden="true">Ctrl K</span>
  </button>
  <UpdateAction
    audioLocked={snapshot.resources.audio_locked}
    audioOwnerKind={snapshot.resources.owner_kind}
    voiceBuildActive={snapshot.voiceBuildActive}
    {myVoiceRecording}
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
</header>

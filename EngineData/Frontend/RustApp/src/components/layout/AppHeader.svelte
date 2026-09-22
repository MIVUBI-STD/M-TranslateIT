<script lang="ts">
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import ProductivityActions from "../runtime/ProductivityActions.svelte";

  let {
    snapshot,
    route,
    notice,
    myVoiceRecording,
    onNavigate,
    onToggleMeeting,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    notice: string;
    myVoiceRecording: boolean;
    onNavigate: (route: AppRoute) => void;
    onToggleMeeting: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

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

  <p class="m-0 min-w-0 flex-1 truncate text-right text-[11.5px] text-[var(--ti-text-muted)]" aria-live="polite" title={notice}>{notice}</p>

  <ProductivityActions
    {snapshot}
    audioLocked={snapshot.resources.audio_locked}
    audioOwnerKind={snapshot.resources.owner_kind}
    voiceBuildActive={snapshot.voiceBuildActive}
    {myVoiceRecording}
    {onNavigate}
    {onToggleMeeting}
    {onNotice}
  />

  {#if snapshot.meeting.applicationOwned && snapshot.meeting.hasSession}
    <button
      type="button"
      class="flex shrink-0 items-center gap-2 rounded-full border border-[var(--ti-success-border)] bg-[var(--ti-success-surface)] px-3 py-1.5 text-left"
      aria-label="Open active Meeting translation"
      onclick={() => { onNavigate("meeting"); }}
    >
      <span class="size-2 rounded-full bg-[var(--ti-success)]" aria-hidden="true"></span>
      <strong class="text-[11.5px] font-semibold text-[var(--ti-success)]">{snapshot.meeting.label}</strong>
    </button>
  {/if}
</header>

<script lang="ts">
  import { Check } from "@lucide/svelte";
  import type { MyVoiceBuildStatus } from "../../app/bridge/myVoiceBuildApi";
  let {
    build, statusCheckUnavailable, busy, quickPreviewBusy,
    readyDurationLabel, minimumDurationLabel, speechProgress, onRetry,
  }: {
    build: MyVoiceBuildStatus;
    statusCheckUnavailable: boolean;
    busy: boolean;
    quickPreviewBusy: boolean;
    readyDurationLabel: string;
    minimumDurationLabel: string;
    speechProgress: number;
    onRetry: () => void | Promise<void>;
  } = $props();
</script>

{#if statusCheckUnavailable}
    <div role="alert" class="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] p-3">
      <span class="text-sm">My Voice status is unavailable. The previous progress may be outdated.</span>
      <button type="button" class="ti-button ti-button-secondary" disabled={busy || quickPreviewBusy} onclick={() => void onRetry()}>Retry status</button>
    </div>
  {/if}
  <div class="flex items-start justify-between gap-5">
    <div>
      <span class="ti-kicker">My Voice</span>
      <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Step 3 · Create and review My Voice</h3>
      <p class="mb-0 mt-2 max-w-[680px] text-sm leading-6 text-[var(--ti-text-muted)]">Use your accepted recordings to create a personalized English voice. You can listen before choosing it.</p>
    </div>
    {#if build.approved_voice_ready}
      <span class="flex items-center gap-1.5 text-sm font-semibold text-[var(--ti-success)]"><Check size={15} />My Voice ready</span>
    {/if}
  </div>

<div class="mt-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3.5">
    <div class="flex items-center justify-between gap-4">
      <strong class="text-sm font-semibold">Ready recordings</strong>
      <span class="text-xs font-semibold text-[var(--ti-text-muted)]">{readyDurationLabel} / {minimumDurationLabel}</span>
    </div>
    <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">{build.accepted_take_count} accepted recordings · clear, varied recordings help the result</p>
    <div
      class="mt-3 h-1.5 overflow-hidden rounded-full bg-[var(--ti-border)]"
      role="progressbar"
      aria-label="Ready recordings progress"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={speechProgress}
      aria-valuetext={`${readyDurationLabel} of ${minimumDurationLabel} accepted speech`}
    >
      <div class="h-full rounded-full bg-[var(--ti-accent)]" style={`width:${speechProgress}%`}></div>
    </div>
  </div>

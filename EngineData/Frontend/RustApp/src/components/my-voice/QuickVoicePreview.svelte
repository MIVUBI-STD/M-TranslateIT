<script lang="ts">
  import { Play } from "@lucide/svelte";
  import type { MyVoiceBuildStatus } from "../../app/bridge/myVoiceBuildApi";
  let {
    build, busy, quickPreviewBusy, quickPreviewReady, quickPreviewPlaying,
    previewAuthorized = $bindable(false),
    quickPreviewOptionsOpen = $bindable(false),
    onGenerate, onPlay,
  }: {
    build: MyVoiceBuildStatus;
    busy: boolean;
    quickPreviewBusy: boolean;
    quickPreviewReady: boolean;
    quickPreviewPlaying: boolean;
    previewAuthorized?: boolean;
    quickPreviewOptionsOpen?: boolean;
    onGenerate: () => void | Promise<void>;
    onPlay: () => void | Promise<void>;
  } = $props();
</script>

<div class="mt-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <strong class="text-sm font-semibold">Quick Preview · Not for meetings</strong>
        <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">Optional temporary preview; never usable in Meeting.</p>
      </div>
      <button
        type="button"
        class="ti-button ti-button-secondary min-h-9"
        aria-controls="my-voice-quick-preview-options"
        aria-expanded={quickPreviewOptionsOpen || quickPreviewBusy || build.preview_active}
        disabled={quickPreviewBusy || build.preview_active}
        onclick={() => { quickPreviewOptionsOpen = !quickPreviewOptionsOpen; }}
      >{quickPreviewOptionsOpen || quickPreviewBusy || build.preview_active ? "Hide options" : "Show options"}</button>
    </div>
    <div id="my-voice-quick-preview-options" hidden={!quickPreviewOptionsOpen && !quickPreviewBusy && !build.preview_active}>
        <p class="mb-0 mt-3 text-xs leading-5 text-[var(--ti-text-muted)]">Try a short accepted recording before full training. A temporary sample is not an approved Meeting voice.</p>
        <label class="mt-3 flex items-start gap-2 text-xs text-[var(--ti-text-muted)]">
          <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={previewAuthorized} />
          I own this voice or have permission to create a preview.
        </label>
        <div class="mt-3 flex flex-wrap gap-2">
          <button type="button" class="ti-button ti-button-secondary"
            disabled={!previewAuthorized || busy || quickPreviewBusy || build.active || build.preview_active || build.accepted_take_count === 0}
            onclick={() => void onGenerate()}>Generate Quick Preview</button>
          {#if quickPreviewReady}
            <button type="button" class="ti-button ti-button-secondary" disabled={quickPreviewBusy || quickPreviewPlaying}
              onclick={() => void onPlay()}><Play size={14} />{quickPreviewPlaying ? "Playing..." : "Listen to Preview"}</button>
          {/if}
        </div>
      </div>
  </div>

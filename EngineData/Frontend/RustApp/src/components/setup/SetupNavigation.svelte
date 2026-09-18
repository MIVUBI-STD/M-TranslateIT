<script lang="ts">
  import { ArrowLeft, Check, ChevronRight } from "@lucide/svelte";
  import type { SetupStep } from "../../app/runtime/setupFlow";

  let {
    step,
    busy,
    microphoneReady,
    meetingRouteReady,
    meetingVoiceReady,
    meetingReady,
    onBack,
    onDefer,
    onAdvance,
    onRepair,
    onRefresh,
    onComplete,
  }: {
    step: SetupStep;
    busy: boolean;
    microphoneReady: boolean;
    meetingRouteReady: boolean;
    meetingVoiceReady: boolean | null;
    meetingReady: boolean;
    onBack: () => void;
    onDefer: () => void | Promise<void>;
    onAdvance: (step: SetupStep) => void | Promise<void>;
    onRepair: () => void | Promise<void>;
    onRefresh: () => void | Promise<void>;
    onComplete: () => void | Promise<void>;
  } = $props();
</script>

<footer class="flex items-center justify-between gap-4 border-t border-[var(--ti-border)] pt-5">
  <div>
    {#if step > 1}
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onBack}>
        <ArrowLeft size={16} /> Back
      </button>
    {/if}
  </div>

  <div class="ti-action-row justify-end">
    <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void onDefer()}>
      Set Up Later
    </button>

    {#if step === 1}
      <button type="button" class="ti-button" disabled={busy} onclick={() => void onAdvance(2)}>
        Continue <ChevronRight size={16} />
      </button>
    {:else if step === 2}
      <button type="button" class="ti-button" disabled={busy || !microphoneReady} onclick={() => void onAdvance(3)}>
        Continue <ChevronRight size={16} />
      </button>
    {:else if step === 3}
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void onRepair()}>
        Repair Setup
      </button>
      <button type="button" class="ti-button" disabled={busy || !meetingRouteReady} onclick={() => void onAdvance(4)}>
        Continue <ChevronRight size={16} />
      </button>
    {:else}
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void onRefresh()}>
        Refresh Status
      </button>
      {#if meetingVoiceReady && !meetingReady}
        <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void onRepair()}>
          Repair Setup
        </button>
      {/if}
      <button type="button" class="ti-button" disabled={busy || !meetingVoiceReady || !meetingReady} onclick={() => void onComplete()}>
        <Check size={16} /> Open Meeting
      </button>
    {/if}
  </div>
</footer>

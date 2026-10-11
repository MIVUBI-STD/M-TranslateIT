<script lang="ts">
  import { Play } from "@lucide/svelte";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";
  let { busy, builtinPreviewing, builtinPendingId, onPreview, onSelect, onConfirm, onCancel }: {
    busy: boolean;
    builtinPreviewing: BuiltinVoiceId | null;
    builtinPendingId: BuiltinVoiceId | null;
    onPreview: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onSelect: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
  } = $props();
</script>

  <section class="ti-panel mb-6 p-5">
    <span class="ti-kicker">Ready now</span>
    <strong class="mt-2 block text-base font-semibold">Step 1 · Choose a ready-made voice</strong>
    <p class="mb-0 mt-1.5 max-w-[720px] text-sm leading-6 text-[var(--ti-text-muted)]">Pick one of these to start immediately. You can make a personalized voice later.</p>
    <div class="mt-4 grid grid-cols-2 gap-3">
      {#each [
        { id: "MaleVoice" as BuiltinVoiceId, label: "Built-in Male", detail: "Ready-made English voice" },
        { id: "FemaleVoice" as BuiltinVoiceId, label: "Built-in Female", detail: "Ready-made English voice" },
      ] as voice (voice.id)}
        <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface)] p-4">
          <strong class="block text-sm font-semibold">{voice.label}</strong>
          <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">{voice.detail}</p>
          <div class="ti-action-row mt-4">
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || builtinPreviewing !== null} onclick={() => void onPreview(voice.id)}>
              <Play size={14} /> {builtinPreviewing === voice.id ? "Playing..." : "Preview"}
            </button>
            <button type="button" class="ti-button" disabled={busy || builtinPreviewing !== null} onclick={() => void onSelect(voice.id)}>Select</button>
          </div>
        </div>
      {/each}
    </div>

    {#if builtinPendingId}
      <div class="mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
        <strong class="text-sm font-semibold">Replace the current Meeting voice?</strong>
        <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{(builtinPendingId === "MaleVoice" ? "Built-in Male" : "Built-in Female")} will replace the voice currently selected for Meeting.</p>
        <div class="ti-action-row mt-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onCancel}>Cancel</button>
          <button type="button" class="ti-button" disabled={busy} onclick={() => void onConfirm()}>Replace Voice</button>
        </div>
      </div>
    {/if}
  </section>

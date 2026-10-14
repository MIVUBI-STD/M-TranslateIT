<script lang="ts">
  import { Check, ChevronDown, Play } from "@lucide/svelte";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";
  let {
    busy, builtinPreviewing, builtinPendingId, selectedVoiceKind,
    selectedBuiltinVoiceId, savedPersonalVoiceAvailable, selectedVoiceVerified,
    voiceChangesLocked, onPreview, onSelect, onSelectPersonal, onConfirm, onCancel, onRetry,
  }: {
    busy: boolean;
    builtinPreviewing: BuiltinVoiceId | null;
    builtinPendingId: BuiltinVoiceId | null;
    selectedVoiceKind: string;
    selectedBuiltinVoiceId: string | null;
    savedPersonalVoiceAvailable: boolean;
    selectedVoiceVerified: boolean;
    voiceChangesLocked: boolean;
    onPreview: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onSelect: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onSelectPersonal: () => void | Promise<void>;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
    onRetry: () => void | Promise<void>;
  } = $props();
  let choicesOpen = $state(false);
  const currentLabel = $derived(
    !selectedVoiceVerified ? "Selection unavailable"
      : selectedVoiceKind === "personal" ? "My Voice"
      : selectedVoiceKind === "built_in" && selectedBuiltinVoiceId === "MaleVoice" ? "Built-in Male"
      : selectedVoiceKind === "built_in" && selectedBuiltinVoiceId === "FemaleVoice" ? "Built-in Female"
      : "No voice selected",
  );
</script>

<section class="ti-panel p-3 sm:p-4" aria-labelledby="my-voice-selected-heading">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="min-w-0">
      <h3 id="my-voice-selected-heading" class="mb-0 text-xs font-semibold uppercase tracking-wide text-[var(--ti-text-muted)]">Current Meeting voice</h3>
      <p class="mb-0 mt-0.5 text-base font-semibold">{currentLabel}</p>
      <p class="mb-0 mt-0.5 text-xs text-[var(--ti-text-muted)]">
        {selectedVoiceVerified
          ? (selectedVoiceKind === "none" || selectedVoiceKind === "unknown" ? "Select a voice to use for meetings." : "Selected for meetings")
          : "Couldn't verify the current selection."}
      </p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      {#if !selectedVoiceVerified}
        <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void onRetry()}>Retry status</button>
      {/if}
      <button type="button" class="ti-button ti-button-secondary" aria-controls="my-voice-voice-choices" aria-expanded={choicesOpen || builtinPendingId !== null}
        onclick={() => { if (!builtinPendingId) choicesOpen = !choicesOpen; }}>
        Change voice <ChevronDown size={16} />
      </button>
    </div>
  </div>

  <div id="my-voice-voice-choices" hidden={!choicesOpen && !builtinPendingId} class="mt-4 border-t border-[var(--ti-border)] pt-4">
    <p class="mb-0 text-xs leading-5 text-[var(--ti-text-muted)]">Preview a voice before selecting it.</p>
    <div class="mt-3 space-y-2">
      {#each [
        { id: "MaleVoice" as BuiltinVoiceId, label: "Built-in Male" },
        { id: "FemaleVoice" as BuiltinVoiceId, label: "Built-in Female" },
      ] as voice (voice.id)}
        {@const selected = selectedVoiceVerified && selectedVoiceKind === "built_in" && selectedBuiltinVoiceId === voice.id}
        <div class="flex flex-wrap items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] px-3 py-3">
          <div class="min-w-0 flex-1">
            <strong class="block text-sm font-semibold">{voice.label}</strong>
            {#if selected}<span class="flex items-center gap-1 text-xs text-[var(--ti-success)]"><Check size={13} /> Selected</span>{/if}
          </div>
          <button type="button" class="ti-button ti-button-secondary" disabled={busy || builtinPreviewing !== null} onclick={() => void onPreview(voice.id)}>
            <Play size={14} /> {builtinPreviewing === voice.id ? "Playing..." : "Preview"}
          </button>
          <button type="button" class="ti-button" disabled={busy || voiceChangesLocked || builtinPreviewing !== null || selected || !selectedVoiceVerified} onclick={() => void onSelect(voice.id)}>{selected ? "Selected" : "Select"}</button>
        </div>
      {/each}
      {#if savedPersonalVoiceAvailable && selectedVoiceVerified}
        <div class="flex flex-wrap items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] px-3 py-3">
          <div class="min-w-0 flex-1">
            <strong class="block text-sm font-semibold">My Voice</strong>
            <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">Saved personal voice</p>
          </div>
          <button type="button" class="ti-button ti-button-secondary" disabled={busy || voiceChangesLocked || builtinPreviewing !== null || selectedVoiceKind === "personal"} onclick={() => void onSelectPersonal()}>
            {selectedVoiceKind === "personal" ? "Selected" : "Use My Voice"}
          </button>
        </div>
      {/if}
    </div>
    {#if voiceChangesLocked}
      <p class="mb-0 mt-3 text-xs text-[var(--ti-text-muted)]">Finish the current Meeting or recording before switching voices.</p>
    {/if}
    {#if builtinPendingId}
      <div class="mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] p-4" role="group" aria-label="Confirm voice replacement">
        <strong class="text-sm font-semibold">Replace the current Meeting voice?</strong>
        <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{(builtinPendingId === "MaleVoice" ? "Built-in Male" : "Built-in Female")} will replace the voice currently selected for Meeting.</p>
        <div class="mt-3 flex flex-wrap gap-2">
          <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onCancel}>Cancel</button>
          <button type="button" class="ti-button" disabled={busy || voiceChangesLocked} onclick={() => void onConfirm()}>Replace Voice</button>
        </div>
      </div>
    {/if}
  </div>
</section>

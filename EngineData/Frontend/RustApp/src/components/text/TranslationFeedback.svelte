<script lang="ts">
  import { Flag, X } from "@lucide/svelte";
  import { saveTranslationFeedback, type TranslationIssueCategory } from "../../app/runtime/translationFeedbackState";

  let {
    source,
    translation,
    sourceLanguage,
    targetLanguage,
    onNotice,
  }: {
    source: string;
    translation: string;
    sourceLanguage: string;
    targetLanguage: string;
    onNotice: (message: string) => void;
  } = $props();

  let open = $state(false);

  const options: Array<{ value: TranslationIssueCategory; label: string }> = [
    { value: "meaning_changed", label: "Meaning changed" },
    { value: "wrong_terminology", label: "Wrong terminology" },
    { value: "name_number", label: "Name / number incorrect" },
    { value: "incomplete", label: "Incomplete" },
    { value: "unnatural", label: "Unnatural wording" },
  ];

  function report(category: TranslationIssueCategory): void {
    const saved = saveTranslationFeedback({ category, source, translation, sourceLanguage, targetLanguage });
    if (!saved) {
      onNotice("Translation issue couldn't be saved locally.");
      return;
    }
    open = false;
    onNotice("Translation issue saved locally for later quality review.");
  }
</script>

<div class="relative">
  <button type="button" class="ti-button ti-button-secondary min-h-9" disabled={!source.trim() || !translation.trim()} onclick={() => { open = !open; }}>
    <Flag size={14} /> Report issue
  </button>
  {#if open}
    <div class="absolute bottom-11 right-0 z-20 w-[250px] overflow-hidden rounded-[12px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] shadow-xl">
      <header class="flex items-center justify-between border-b border-[var(--ti-border)] px-3.5 py-3">
        <div>
          <strong class="block text-[12px] font-semibold">Translation issue</strong>
          <span class="mt-0.5 block text-[10.5px] text-[var(--ti-text-soft)]">Saved locally and bounded.</span>
        </div>
        <button type="button" class="grid size-7 place-items-center rounded-[7px] hover:bg-[var(--ti-surface-soft)]" aria-label="Close" onclick={() => { open = false; }}><X size={14} /></button>
      </header>
      <div class="p-1.5">
        {#each options as option (option.value)}
          <button type="button" class="w-full rounded-[8px] px-3 py-2.5 text-left text-[11.5px] hover:bg-[var(--ti-surface-soft)]" onclick={() => report(option.value)}>{option.label}</button>
        {/each}
      </div>
    </div>
  {/if}
</div>

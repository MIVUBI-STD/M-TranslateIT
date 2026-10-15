<script lang="ts">
  import { ArrowLeftRight, Copy, Check, X } from "@lucide/svelte";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { textTranslationCacheKey, getCachedTextTranslation, putCachedTextTranslation } from "../app/runtime/textTranslationCache";
  import { canUseCurrentTextResult, type TextResultState } from "../app/runtime/textResultPolicy";
  import { languageName } from "../app/shared/state";
  import type { RuntimeSettings } from "../app/shared/types";
  import StatusBadge from "../components/ui/StatusBadge.svelte";

  const MAX_LENGTH = 2000;
  let { settings, textStatus, onSettingsChange, onNotice }: {
    settings: RuntimeSettings; textStatus: string;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let sourceText = $state("");
  let targetText = $state("");
  let translating = $state(false);
  let settingsSaving = $state(false);
  let resultState = $state<TextResultState>("idle");
  let resultMessage = $state("Type or paste text to translate.");
  let reviewHints = $state<string[]>([]);
  let lastTranslatedSource = $state<string | null>(null);
  let lastTranslatedSettingsKey = $state<string | null>(null);
  let targetRevision = 0;
  let copied = $state(false);

  const sourceLanguageName = $derived(languageName(settings.source_language));
  const targetLanguageName = $derived(languageName(settings.target_language));
  const currentTextSettingsKey = $derived(textTranslationCacheKey("", settings));
  const currentResultUsable = $derived(canUseCurrentTextResult({
    source: sourceText, target: targetText, translatedSource: lastTranslatedSource,
    translatedSettingsKey: lastTranslatedSettingsKey,
    currentSettingsKey: currentTextSettingsKey, state: resultState,
  }));

  $effect(() => {
    if (lastTranslatedSource !== null && lastTranslatedSettingsKey !== currentTextSettingsKey && !translating) {
      resultState = "stale";
      reviewHints = [];
      resultMessage = "Language direction changed. Translate again.";
    }
  });

  function sourceChanged(): void {
    copied = false;
    if (lastTranslatedSource !== null && (sourceText.trim() !== lastTranslatedSource
      || lastTranslatedSettingsKey !== currentTextSettingsKey)) {
      resultState = "stale";
      reviewHints = [];
      resultMessage = "Text changed. Translate again.";
    }
  }

  function targetChanged(): void {
    targetRevision += 1;
    copied = false;
    reviewHints = [];
    resultState = lastTranslatedSource !== null && sourceText.trim() === lastTranslatedSource
      && lastTranslatedSettingsKey === currentTextSettingsKey ? "edited" : "stale";
  }

  function clearWorkspace(): void {
    if (translating || settingsSaving) return;
    sourceText = ""; targetText = ""; copied = false;
    reviewHints = [];
    lastTranslatedSource = null; lastTranslatedSettingsKey = null;
    targetRevision += 1; resultState = "idle";
    resultMessage = "Type or paste text to translate.";
  }

  function applyTranslationResult(
    translated: string,
    needsReview: boolean,
    hints: string[],
    source: string,
    settingsKey: string,
  ): void {
    targetText = translated;
    lastTranslatedSource = source;
    lastTranslatedSettingsKey = settingsKey;
    targetRevision += 1;
    copied = false;
    reviewHints = needsReview ? [...hints] : [];
    resultState = needsReview ? "review" : "success";
    resultMessage = needsReview
      ? "Translation ready. Check the review notes before using it."
      : "Translation ready.";
  }

  async function submitText(): Promise<void> {
    const requestSource = sourceText.trim();
    if (!requestSource || translating || settingsSaving) return;
    if (Array.from(requestSource).length > MAX_LENGTH) {
      reviewHints = [];
      resultState = "error"; resultMessage = "Text is too long."; return;
    }
    const key = textTranslationCacheKey(requestSource, settings);
    const currentKey = currentTextSettingsKey;
    const revision = targetRevision;
    const cached = getCachedTextTranslation(key);
    if (cached) {
      if (sourceText.trim() !== requestSource || currentTextSettingsKey !== currentKey) return;
      applyTranslationResult(cached.translated, cached.needsReview, cached.reviewHints, requestSource, currentKey);
      return;
    }
    translating = true; copied = false;
    reviewHints = [];
    resultState = "translating"; resultMessage = "Translating...";
    try {
      const result = await runtimeProductFacade.runProductTranslation(requestSource);
      if (sourceText.trim() !== requestSource || currentTextSettingsKey !== currentKey) {
        resultState = "stale"; resultMessage = "Text changed while translating. Try again."; return;
      }
      if (!result.ok) {
        resultState = "error"; resultMessage = result.message; return;
      }
      if (targetRevision !== revision) {
        resultState = "stale"; resultMessage = "Your newer edit was kept."; return;
      }
      applyTranslationResult(result.translated, result.needsReview, result.reviewHints, requestSource, currentKey);
      putCachedTextTranslation({
        key, translated: result.translated,
        reviewHints: result.reviewHints, needsReview: result.needsReview,
      });
    } catch {
      resultState = "error"; resultMessage = "Translation is unavailable. Try again.";
    } finally {
      translating = false;
    }
  }

  async function copyTranslation(): Promise<void> {
    if (!currentResultUsable) return;
    try {
      if (!navigator.clipboard?.writeText) throw new Error("clipboard unavailable");
      await navigator.clipboard.writeText(targetText.trim());
      copied = true; onNotice("Translation copied.");
    } catch {
      copied = false; onNotice("Couldn't copy the translation.");
    }
  }

  async function swapLanguages(): Promise<void> {
    if (translating || settingsSaving) return;
    settingsSaving = true;
    const originalInput = sourceText;
    const originalOutput = targetText;
    // Swap preserves both editable panes. A source-only draft stays intact.
    const nextInput = originalOutput.trim() ? originalOutput : originalInput;
    const nextTarget = originalOutput.trim() ? originalInput : "";
    const candidate: RuntimeSettings = {
      ...settings, source_language: settings.target_language,
      target_language: settings.source_language, audio: { ...settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) throw new Error("save failed");
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      sourceText = nextInput; targetText = nextTarget;
      reviewHints = [];
      lastTranslatedSource = null; lastTranslatedSettingsKey = null;
      targetRevision += 1; copied = false; resultState = "idle";
      resultMessage = "Type or paste text to translate.";
    } catch { onNotice("Couldn't change language direction."); }
    finally { settingsSaving = false; }
  }
  function keydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault(); void submitText();
    }
  }
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Translate text</h2>
      <p class="ti-page-copy">Indonesian ↔ English</p>
    </div>
    {#if textStatus !== "Ready"}
      <StatusBadge label={textStatus} tone={textStatus === "Unavailable" ? "danger" : "warning"} />
    {/if}
  </header>
  <article class="ti-panel overflow-hidden">
    <div class="grid grid-cols-[1fr_auto_1fr] items-center gap-3 border-b border-[var(--ti-border)] px-5 py-3">
      <strong class="text-sm">{sourceLanguageName}</strong>
      <button type="button" class="ti-button ti-button-secondary min-h-9 px-3" aria-label="Swap languages"
        disabled={translating || settingsSaving} onclick={() => void swapLanguages()}>
        <ArrowLeftRight size={15} /> Swap
      </button>
      <strong class="text-right text-sm">{targetLanguageName}</strong>
    </div>
    <div class="ti-text-editor-grid">
      <div class="ti-editor-pane grid gap-3 border-r border-[var(--ti-border)]">
        <div class="flex items-center justify-between gap-3">
          <span class="ti-field-label">Original text</span>
          {#if sourceText || targetText}
            <button type="button" class="ti-button ti-button-secondary min-h-8 px-2"
              aria-label="Clear text" title="Clear" disabled={translating || settingsSaving}
              onclick={clearWorkspace}><X size={14} /></button>
          {/if}
        </div>
        <textarea class="ti-editor" placeholder="Type or paste text"
          maxlength={MAX_LENGTH} bind:value={sourceText} oninput={sourceChanged}
          onkeydown={keydown} aria-label="Original text"></textarea>
      </div>
      <div class="ti-editor-pane grid gap-3">
        <span class="ti-field-label">Translation</span>
        <textarea class="ti-editor" placeholder="Translation appears here"
          bind:value={targetText} oninput={targetChanged} aria-label="Translated text"></textarea>
      </div>
    </div>
    {#if resultState === "review" && currentResultUsable}
      <section class="border-t border-[var(--ti-warning-border)] px-5 py-3 text-sm text-[var(--ti-warning)]"
        role="status" aria-live="polite" aria-label="Translation review notes">
        <strong class="font-semibold">Review this translation before using it.</strong>
        {#if reviewHints.length > 0}
          <ul class="mb-0 mt-2 list-disc space-y-1 pl-5">
            {#each reviewHints as hint, index (index)}
              <li>{hint}</li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}
    <footer class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--ti-border)] px-5 py-4">
      <p class="m-0 text-[12px] text-[var(--ti-text-muted)]" aria-live="polite">{resultMessage}</p>
      <div class="ti-action-row ml-auto">
        <button type="button" class="ti-button ti-button-secondary" disabled={!currentResultUsable}
          onclick={() => void copyTranslation()}>
          {#if copied}<Check size={15} /> Copied{:else}<Copy size={15} /> Copy{/if}
        </button>
        <button type="button" class="ti-button min-w-28"
          disabled={!sourceText.trim() || translating || settingsSaving}
          onclick={() => void submitText()}>{translating ? "Translating..." : "Translate"}</button>
      </div>
    </footer>
  </article>
</section>

<script lang="ts">
  import { ArrowLeftRight, Check, Copy, PictureInPicture2, RefreshCw, ShieldAlert } from "@lucide/svelte";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import { runtimeProductFacade } from "../app/bridge/runtimeProductFacade";
  import { publishTranslationOverlay } from "../app/runtime/translationOverlayRuntime";
  import { getCachedTextTranslation, putCachedTextTranslation, textTranslationCacheKey } from "../app/runtime/textTranslationCache";
  import { languageName } from "../app/shared/state";
  import type { RuntimeSettings } from "../app/shared/types";
  import StatusBadge from "../components/ui/StatusBadge.svelte";
  import TranslationFeedback from "../components/text/TranslationFeedback.svelte";

  const MAX_MANUAL_TRANSLATION_CHARS = 2000;
  type TextResultState = "idle" | "translating" | "success" | "stale" | "error";
  type CopyState = "idle" | "copied" | "error";
  type CopyMode = "translation" | "source" | "bilingual";

  let {
    settings,
    textStatus,
    onSettingsChange,
    onNotice,
  }: {
    settings: RuntimeSettings;
    textStatus: string;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let sourceText = $state("");
  let targetText = $state("");
  let translating = $state(false);
  let settingsSaving = $state(false);
  let resultState = $state<TextResultState>("idle");
  let resultLabel = $state("Ready");
  let resultMessage = $state("Enter text, then choose Translate.");
  let lastTranslatedSource = $state<string | null>(null);
  let copyState = $state<CopyState>("idle");
  let copyMode = $state<CopyMode>("translation");
  let reviewHints = $state<string[]>([]);
  let alternativeBusy = $state(false);
  let targetRevision = 0;

  const sourceLanguageName = $derived(languageName(settings.source_language));
  const targetLanguageName = $derived(languageName(settings.target_language));

  function setResult(state: TextResultState, label: string, message: string): void {
    resultState = state;
    resultLabel = label;
    resultMessage = message;
  }

  function handleSourceInput(): void {
    if (lastTranslatedSource === null) {
      if (resultState === "error") setResult("idle", "Ready", "Enter text, then choose Translate.");
      return;
    }
    if (sourceText.trim() === lastTranslatedSource) {
      setResult(
        reviewHints.length > 0 ? "stale" : "success",
        reviewHints.length > 0 ? "Check details" : "Translated",
        reviewHints.length > 0 ? "Translation is ready. Review the noted details before using it." : "Translation is up to date.",
      );
      return;
    }
    setResult("stale", "Needs update", "Source text changed. Translate again to refresh the result.");
  }

  function handleTargetInput(): void {
    targetRevision += 1;
    if (copyState !== "idle") copyState = "idle";
  }

  async function submitText(): Promise<void> {
    const source = sourceText.trim();
    if (!source) {
      setResult("error", "Enter text", "Type or paste something to translate.");
      onNotice("Type or paste something to translate.");
      return;
    }
    if (Array.from(source).length > MAX_MANUAL_TRANSLATION_CHARS) {
      const message = `Text is too long. Limit: ${MAX_MANUAL_TRANSLATION_CHARS} characters.`;
      setResult("error", "Text too long", message);
      onNotice(message);
      return;
    }
    if (translating || alternativeBusy) return;

    const requestSource = source;
    const requestTargetRevision = targetRevision;
    const previousTarget = targetText;
    const cacheKey = textTranslationCacheKey(requestSource, settings);
    const cached = getCachedTextTranslation(cacheKey);
    if (cached) {
      targetText = cached.translated;
      reviewHints = cached.reviewHints;
      lastTranslatedSource = requestSource;
      setResult(
        cached.needsReview ? "stale" : "success",
        cached.needsReview ? "Check details" : "Translated",
        "Translation restored from this app session.",
      );
      onNotice("Translation restored instantly from the local session cache.");
      return;
    }
    translating = true;
    copyState = "idle";
    reviewHints = [];
    setResult("translating", "Translating", "Translating...");
    onNotice("Translating...");

    try {
      const result = await runtimeProductFacade.runProductTranslation(requestSource);
      const userEditedTargetWhileRunning = targetRevision !== requestTargetRevision;
      if (!result.ok) {
        if (!userEditedTargetWhileRunning) targetText = previousTarget;
        setResult("error", "Couldn't translate", result.message);
        onNotice(`Couldn't translate: ${result.message}`);
        return;
      }

      if (userEditedTargetWhileRunning) {
        setResult("stale", "Edit kept", "Translation finished, but your newer edit was kept.");
        onNotice("Your newer edit was kept.");
        return;
      }

      targetText = result.translated;
      reviewHints = result.reviewHints;
      lastTranslatedSource = requestSource;
      putCachedTextTranslation({
        key: cacheKey,
        translated: result.translated,
        reviewHints: result.reviewHints,
        needsReview: result.needsReview,
      });
      if (sourceText.trim() === requestSource) {
        setResult(
          result.needsReview ? "stale" : "success",
          result.needsReview ? "Check details" : "Translated",
          result.message,
        );
        onNotice(result.message);
      } else {
        setResult("stale", "Needs update", "The source text changed. Translate again to update the result.");
        onNotice("Translation finished for the previous text.");
      }
    } catch {
      if (targetRevision === requestTargetRevision) targetText = previousTarget;
      const message = "Translation is unavailable right now. Try again in a moment.";
      setResult("error", "Couldn't translate", message);
      onNotice(message);
    } finally {
      translating = false;
    }
  }

  async function requestAlternative(): Promise<void> {
    const source = sourceText.trim();
    const current = targetText.trim();
    if (!source || !current || alternativeBusy || translating) return;
    alternativeBusy = true;
    const requestTargetRevision = targetRevision;
    try {
      const result = await runtimeProductFacade.runProductTranslationAlternative(source, current);
      if (!result.ok) {
        onNotice(result.message);
        return;
      }
      if (targetRevision !== requestTargetRevision || sourceText.trim() !== source) {
        onNotice("Try another wording finished, but your newer edit was kept.");
        return;
      }
      targetText = result.translated;
      reviewHints = result.reviewHints;
      targetRevision += 1;
      setResult(
        result.needsReview ? "stale" : "success",
        result.needsReview ? "Check details" : "Alternative",
        result.message,
      );
      onNotice("Alternative wording ready.");
    } catch {
      onNotice("Try another wording is unavailable right now.");
    } finally {
      alternativeBusy = false;
    }
  }

  async function showFloatingCaption(): Promise<void> {
    const text = targetText.trim();
    if (!text) {
      onNotice("There is no translated text to show.");
      return;
    }
    const result = await publishTranslationOverlay({
      text,
      language: settings.target_language,
      source: "text",
      revision: `text:${targetRevision}:${Date.now()}`,
    }, true);
    onNotice(result === "shown" ? "Floating caption updated." : "Floating caption is unavailable right now.");
  }

  async function copySelection(): Promise<void> {
    const source = sourceText.trim();
    const target = targetText.trim();
    const value = copyMode === "source"
      ? source
      : copyMode === "bilingual"
        ? `${sourceLanguageName}\n${source}\n\n${targetLanguageName}\n${target}`
        : target;
    if (!value) {
      copyState = "error";
      onNotice("There is no text to copy.");
      return;
    }
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard is unavailable.");
      await navigator.clipboard.writeText(value);
      copyState = "copied";
      onNotice(copyMode === "bilingual" ? "Bilingual text copied." : copyMode === "source" ? "Source text copied." : "Translation copied.");
    } catch {
      copyState = "error";
      onNotice("Couldn't copy the text. Try again.");
    }
  }

  async function swapLanguages(): Promise<void> {
    if (settingsSaving || translating || alternativeBusy) return;
    settingsSaving = true;
    const candidate: RuntimeSettings = {
      ...settings,
      source_language: settings.target_language,
      target_language: settings.source_language,
      audio: { ...settings.audio },
    };
    const visibleTarget = targetText;

    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) throw new Error(result.message || "Language direction could not be saved.");
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      if (visibleTarget.trim()) {
        sourceText = visibleTarget;
        targetText = "";
        targetRevision += 1;
        lastTranslatedSource = null;
        reviewHints = [];
        copyState = "idle";
        setResult("idle", "Ready", "Previous translation moved to the source side.");
      }
      onNotice(`${languageName(saved.source_language)} → ${languageName(saved.target_language)}`);
    } catch {
      onNotice("Couldn't change the language direction. Try again.");
    } finally {
      settingsSaving = false;
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void submitText();
    }
  }

  function stateClass(state: TextResultState): string {
    if (state === "success") return "text-[var(--ti-success)]";
    if (state === "error") return "text-[var(--ti-danger)]";
    if (state === "stale") return "text-[var(--ti-warning)]";
    return "text-[var(--ti-text-muted)]";
  }
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Translate text</h2>
      <p class="ti-page-copy">Translate between Indonesian and English, then edit or copy the result.</p>
    </div>
    {#if textStatus !== "Ready"}
      <StatusBadge
        label={textStatus}
        tone={textStatus === "Unavailable" ? "danger" : textStatus === "Setup Needed" ? "warning" : "neutral"}
      />
    {/if}
  </header>

  <article class="ti-panel overflow-hidden">
    <div class="grid grid-cols-[1fr_auto_1fr] items-center gap-4 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-3.5">
      <div class="flex items-baseline gap-2">
        <span class="ti-field-label">From</span>
        <strong class="text-[14px] font-semibold">{sourceLanguageName}</strong>
      </div>

      <button type="button" class="ti-button ti-button-secondary min-h-9 px-3" aria-label="Swap source and target languages" disabled={settingsSaving || translating || alternativeBusy} onclick={() => void swapLanguages()}>
        <ArrowLeftRight size={15} /><span>Swap</span>
      </button>

      <div class="flex items-baseline justify-end gap-2 text-right">
        <span class="ti-field-label">To</span>
        <strong class="text-[14px] font-semibold">{targetLanguageName}</strong>
      </div>
    </div>

    <div class="grid grid-cols-2">
      <label class="ti-editor-pane grid gap-3 border-r border-[var(--ti-border)]">
        <div class="flex items-center justify-between gap-3">
          <span class="ti-field-label">Enter text</span>
          <span class="text-[11px] text-[var(--ti-text-soft)]">{Array.from(sourceText).length}/{MAX_MANUAL_TRANSLATION_CHARS}</span>
        </div>
        <textarea
          class="ti-editor"
          placeholder="Type or paste text"
          maxlength={MAX_MANUAL_TRANSLATION_CHARS}
          bind:value={sourceText}
          oninput={handleSourceInput}
          onkeydown={handleKeydown}
          aria-label="Source text"
        ></textarea>
      </label>

      <label class="ti-editor-pane grid gap-3">
        <div class="flex items-center justify-between gap-3">
          <span class="ti-field-label">Translation</span>
          <strong class={`text-[11px] font-semibold ${stateClass(resultState)}`}>{resultLabel}</strong>
        </div>
        <textarea
          class="ti-editor"
          placeholder="Translation appears here"
          bind:value={targetText}
          oninput={handleTargetInput}
          aria-label="Translated text"
        ></textarea>
      </label>
    </div>

    {#if reviewHints.length > 0 && targetText.trim()}
      <section class="border-t border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-5 py-3.5" aria-live="polite">
        <div class="flex items-start gap-2.5">
          <ShieldAlert size={15} class="mt-0.5 shrink-0 text-[var(--ti-warning)]" />
          <div>
            <strong class="text-[12px] font-semibold">Review important details</strong>
            <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-muted)]">{reviewHints.join(" ")}</p>
          </div>
        </div>
      </section>
    {/if}

    <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
      <div class="min-w-0">
        <p class="m-0 text-[12.5px] text-[var(--ti-text-muted)]" aria-live="polite">{resultMessage}</p>
        <p class="mb-0 mt-1 text-[11px] text-[var(--ti-text-soft)]">Ctrl + Enter to translate</p>
      </div>
      <div class="ti-action-row shrink-0">
        <button type="button" class="ti-button ti-button-secondary" disabled={!targetText.trim()} onclick={() => void showFloatingCaption()}>
          <PictureInPicture2 size={15} /> Floating caption
        </button>
        <button type="button" class="ti-button ti-button-secondary" disabled={!targetText.trim() || translating || alternativeBusy || Array.from(sourceText.trim()).length > 1000} onclick={() => void requestAlternative()}>
          <RefreshCw size={15} /> {alternativeBusy ? "Trying..." : "Try another wording"}
        </button>
        <label class="flex items-center gap-2">
          <span class="sr-only">Copy format</span>
          <select
            class="ti-field min-h-9 w-[126px] px-2 text-[11.5px]"
            bind:value={copyMode}
            onchange={() => { copyState = "idle"; }}
            aria-label="Copy format"
          >
            <option value="translation">Translation</option>
            <option value="source">Source</option>
            <option value="bilingual">Bilingual</option>
          </select>
        </label>
        <button type="button" class="ti-button ti-button-secondary min-w-24" disabled={copyMode === "source" ? !sourceText.trim() : !targetText.trim()} onclick={() => void copySelection()}>
          {#if copyState === "copied"}<Check size={15} />{:else}<Copy size={15} />{/if}
          {copyState === "copied" ? "Copied" : "Copy"}
        </button>
        <TranslationFeedback
          source={sourceText.trim()}
          translation={targetText.trim()}
          sourceLanguage={settings.source_language}
          targetLanguage={settings.target_language}
          {onNotice}
        />
        <button type="button" class="ti-button min-w-28" disabled={translating || alternativeBusy} onclick={() => void submitText()}>{translating ? "Translating..." : "Translate"}</button>
      </div>
    </footer>
  </article>
</section>

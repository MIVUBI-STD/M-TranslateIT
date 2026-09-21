<script lang="ts">
  import { Copy, Plus, Search, Trash2, Upload } from "@lucide/svelte";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import type { RuntimeSettings, SpokenTermEntry, TerminologyEntry } from "../../app/shared/types";
  import FloatingCaptionPreferences from "./FloatingCaptionPreferences.svelte";
  import TranslationFeedbackReview from "./TranslationFeedbackReview.svelte";

  const MAX_TERMS = 24;
  const MAX_SPOKEN_TERMS = 24;
  const MAX_SPOKEN_ALIASES = 4;

  let {
    settings,
    onSettingsChange,
    onNotice,
  }: {
    settings: RuntimeSettings;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let indonesian = $state("");
  let english = $state("");
  let spokenTerm = $state("");
  let spokenAliases = $state("");
  let saving = $state(false);
  let styleSaving = $state(false);
  let termSearch = $state("");
  const filteredTerms = $derived(settings.terminology.map((entry, index) => ({ entry, index })).filter(({ entry }) => {
    const q = termSearch.trim().toLowerCase();
    return !q || entry.indonesian.toLowerCase().includes(q) || entry.english.toLowerCase().includes(q);
  }));

  async function updateTranslationStyle(style: "natural" | "formal"): Promise<void> {
    if (styleSaving || settings.translation_style === style) return;
    styleSaving = true;
    const candidate: RuntimeSettings = {
      ...settings,
      translation_style: style,
      audio: { ...settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) {
        onNotice(result.message || "Translation style couldn't be saved.");
        return;
      }
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      onNotice(style === "formal" ? "Formal translation style enabled." : "Natural translation style enabled.");
    } catch {
      onNotice("Translation style couldn't be saved. Try again.");
    } finally {
      styleSaving = false;
    }
  }

  async function persist(next: TerminologyEntry[], message: string): Promise<void> {
    if (saving) return;
    saving = true;
    const candidate: RuntimeSettings = {
      ...settings,
      terminology: next,
      audio: { ...settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) {
        onNotice(result.message || "Preferred words couldn't be saved.");
        return;
      }
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      onNotice(message);
    } catch {
      onNotice("Preferred words couldn't be saved. Try again.");
    } finally {
      saving = false;
    }
  }

  async function persistSpokenTerms(next: SpokenTermEntry[], message: string): Promise<void> {
    if (saving) return;
    saving = true;
    const candidate: RuntimeSettings = {
      ...settings,
      spoken_terms: next,
      audio: { ...settings.audio },
    };
    try {
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) {
        onNotice(result.message || "Spoken terms couldn't be saved.");
        return;
      }
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      onNotice(message);
    } catch {
      onNotice("Spoken terms couldn't be saved. Try again.");
    } finally {
      saving = false;
    }
  }

  async function addSpokenTerm(): Promise<void> {
    const term = spokenTerm.trim();
    const aliases = [...new Set(
      spokenAliases
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean)
        .slice(0, MAX_SPOKEN_ALIASES),
    )].filter((value) => value.toLocaleLowerCase() !== term.toLocaleLowerCase());

    if (!term) {
      onNotice("Enter the word or name TranslateIT should listen for.");
      return;
    }
    if (settings.spoken_terms.length >= MAX_SPOKEN_TERMS) {
      onNotice(`You can save up to ${MAX_SPOKEN_TERMS} spoken terms.`);
      return;
    }
    const duplicate = settings.spoken_terms.some((entry) =>
      entry.term.toLocaleLowerCase() === term.toLocaleLowerCase()
      || entry.aliases.some((alias) => alias.toLocaleLowerCase() === term.toLocaleLowerCase()),
    );
    if (duplicate) {
      onNotice("That spoken term is already saved.");
      return;
    }

    await persistSpokenTerms([...settings.spoken_terms, { term, aliases }], "Spoken term saved.");
    spokenTerm = "";
    spokenAliases = "";
  }

  async function removeSpokenTerm(index: number): Promise<void> {
    await persistSpokenTerms(
      settings.spoken_terms.filter((_, itemIndex) => itemIndex !== index),
      "Spoken term removed.",
    );
  }

  async function addTerm(): Promise<void> {
    const id = indonesian.trim();
    const en = english.trim();
    if (!id || !en) {
      onNotice("Enter both the Indonesian and English term.");
      return;
    }
    if (settings.terminology.length >= MAX_TERMS) {
      onNotice(`You can save up to ${MAX_TERMS} preferred word pairs.`);
      return;
    }
    const conflicting = settings.terminology.find(
      (entry) =>
        entry.indonesian.toLocaleLowerCase() === id.toLocaleLowerCase()
        || entry.english.toLocaleLowerCase() === en.toLocaleLowerCase(),
    );
    if (conflicting) {
      onNotice("That word is already saved. Remove the existing pair before adding a different translation.");
      return;
    }
    await persist([...settings.terminology, { indonesian: id, english: en }], "Preferred word saved.");
    indonesian = "";
    english = "";
  }

  async function removeTerm(index: number): Promise<void> {
    await persist(
      settings.terminology.filter((_, itemIndex) => itemIndex !== index),
      "Preferred word removed.",
    );
  }

  function csvEscape(value: string): string {
    return /[",\n]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
  }

  async function copyTermsCsv(): Promise<void> {
    const rows = ["indonesian,english", ...settings.terminology.map((entry) => `${csvEscape(entry.indonesian)},${csvEscape(entry.english)}`)];
    try {
      await navigator.clipboard.writeText(rows.join("\n"));
      onNotice("Preferred words copied as CSV.");
    } catch {
      onNotice("Couldn't copy preferred words.");
    }
  }

  function parseCsvLine(line: string): string[] {
    const values: string[] = [];
    let value = "";
    let quoted = false;
    for (let index = 0; index < line.length; index += 1) {
      const char = line[index];
      if (char === '"') {
        if (quoted && line[index + 1] === '"') { value += '"'; index += 1; }
        else quoted = !quoted;
      } else if (char === "," && !quoted) {
        values.push(value.trim());
        value = "";
      } else {
        value += char;
      }
    }
    values.push(value.trim());
    return values;
  }

  async function importTermsCsv(): Promise<void> {
    try {
      const raw = await navigator.clipboard.readText();
      const lines = raw.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      if (lines[0]?.toLowerCase() === "indonesian,english") lines.shift();
      const merged = [...settings.terminology];
      let added = 0;
      for (const line of lines) {
        if (merged.length >= MAX_TERMS) break;
        const [id, en] = parseCsvLine(line);
        if (!id || !en) continue;
        const conflict = merged.some((entry) =>
          entry.indonesian.toLowerCase() === id.toLowerCase()
          || entry.english.toLowerCase() === en.toLowerCase()
        );
        if (conflict) continue;
        merged.push({ indonesian: id.slice(0, 80), english: en.slice(0, 80) });
        added += 1;
      }
      if (added === 0) {
        onNotice("No new valid preferred words found in clipboard CSV.");
        return;
      }
      await persist(merged, `${added} preferred word pair${added === 1 ? "" : "s"} imported.`);
    } catch {
      onNotice("Couldn't import preferred words from clipboard.");
    }
  }
</script>

<section class="grid gap-4">
<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Translation style</h3>
    <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">Choose how translated wording should sound. Meaning, names, numbers, and preferred terminology stay authoritative.</p>
  </header>
  <div class="grid grid-cols-2 gap-3 p-5">
    <button type="button" class={`rounded-[10px] border p-4 text-left ${settings.translation_style !== "formal" ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" : "border-[var(--ti-border)] bg-[var(--ti-surface)]"}`} disabled={styleSaving} aria-pressed={settings.translation_style !== "formal"} onclick={() => void updateTranslationStyle("natural")}>
      <strong class="block text-[13px] font-semibold">Natural</strong>
      <span class="mt-1 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Default · clear, conversational wording that follows the source naturally.</span>
    </button>
    <button type="button" class={`rounded-[10px] border p-4 text-left ${settings.translation_style === "formal" ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" : "border-[var(--ti-border)] bg-[var(--ti-surface)]"}`} disabled={styleSaving} aria-pressed={settings.translation_style === "formal"} onclick={() => void updateTranslationStyle("formal")}>
      <strong class="block text-[13px] font-semibold">Formal</strong>
      <span class="mt-1 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Professional register for client calls, presentations, and formal communication.</span>
    </button>
  </div>
</article>

<FloatingCaptionPreferences {onNotice} />

<TranslationFeedbackReview {onNotice} />

<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Spoken terms</h3>
    <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">
      Help Meeting transcription recognize names, brands, acronyms, and uncommon words before translation.
    </p>
  </header>

  <div class="grid grid-cols-[1fr_1fr_auto] items-end gap-3 p-5">
    <label class="grid gap-2">
      <span class="ti-field-label">Word or name</span>
      <input class="ti-field min-h-10 px-3" maxlength="80" placeholder="e.g. MIVUBI" bind:value={spokenTerm} disabled={saving} />
    </label>
    <label class="grid gap-2">
      <span class="ti-field-label">Optional aliases</span>
      <input class="ti-field min-h-10 px-3" maxlength="220" placeholder="e.g. mi vu bi, mivubi" bind:value={spokenAliases} disabled={saving} />
      <small class="text-[11px] text-[var(--ti-text-soft)]">Comma-separated, up to {MAX_SPOKEN_ALIASES} aliases.</small>
    </label>
    <button type="button" class="ti-button min-h-10" disabled={saving || !spokenTerm.trim()} onclick={() => void addSpokenTerm()}>
      <Plus size={15} /> Add
    </button>
  </div>

  <div class="border-t border-[var(--ti-border)]">
    {#if settings.spoken_terms.length === 0}
      <p class="m-0 px-5 py-5 text-[12.5px] text-[var(--ti-text-muted)]">
        No spoken terms yet. Add names or specialist terms that speech recognition often misses.
      </p>
    {:else}
      <div class="divide-y divide-[var(--ti-border)]">
        {#each settings.spoken_terms as entry, index (entry.term)}
          <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-5 py-3.5">
            <div class="min-w-0">
              <strong class="block truncate text-[13px] font-semibold">{entry.term}</strong>
              {#if entry.aliases.length > 0}
                <span class="mt-1 block truncate text-[11.5px] text-[var(--ti-text-soft)]">Aliases: {entry.aliases.join(", ")}</span>
              {/if}
            </div>
            <button type="button" class="ti-button ti-button-secondary min-h-8 px-2.5" aria-label={`Remove spoken term ${entry.term}`} disabled={saving} onclick={() => void removeSpokenTerm(index)}>
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
  <footer class="border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-3.5">
    <p class="m-0 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Spoken terms bias ASR recognition only; Preferred words below still control translation wording.</p>
  </footer>
</article>

<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Preferred words</h3>
    <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">
      Tell TranslateIT how specific names or terms should be translated.
    </p>
  </header>

  <div class="grid grid-cols-[1fr_1fr_auto] items-end gap-3 p-5">
    <label class="grid gap-2">
      <span class="ti-field-label">Indonesian</span>
      <input class="ti-field min-h-10 px-3" maxlength="80" placeholder="e.g. pemugaran" bind:value={indonesian} disabled={saving} />
    </label>
    <label class="grid gap-2">
      <span class="ti-field-label">Use this English</span>
      <input class="ti-field min-h-10 px-3" maxlength="80" placeholder="e.g. restoration" bind:value={english} disabled={saving} />
    </label>
    <button type="button" class="ti-button min-h-10" disabled={saving || !indonesian.trim() || !english.trim()} onclick={() => void addTerm()}>
      <Plus size={15} /> Add
    </button>
  </div>

  <div class="flex flex-wrap items-center gap-2 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-3">
    <label class="relative min-w-[200px] flex-1">
      <Search size={14} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" />
      <input class="ti-field min-h-9 w-full pl-9 pr-3 text-xs" bind:value={termSearch} placeholder="Search preferred words..." aria-label="Search preferred words" />
    </label>
    <span class="text-[10.5px] text-[var(--ti-text-soft)]">{settings.terminology.length}/{MAX_TERMS}</span>
    <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-xs" disabled={saving || settings.terminology.length === 0} onclick={() => void copyTermsCsv()}><Copy size={14} /> Copy CSV</button>
    <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-xs" disabled={saving || settings.terminology.length >= MAX_TERMS} onclick={() => void importTermsCsv()}><Upload size={14} /> Import CSV</button>
  </div>

  <div class="border-t border-[var(--ti-border)]">
    {#if settings.terminology.length === 0}
      <p class="m-0 px-5 py-5 text-[12.5px] text-[var(--ti-text-muted)]">
        No preferred words yet. You can add names, product terms, or other wording you want translated consistently.
      </p>
    {:else}
      <div class="divide-y divide-[var(--ti-border)]">
        {#each filteredTerms as item (`${item.entry.indonesian}::${item.entry.english}`)}
          {@const entry = item.entry}
          {@const index = item.index}
          <div class="grid grid-cols-[1fr_auto_1fr_auto] items-center gap-3 px-5 py-3.5">
            <strong class="min-w-0 truncate text-[13px] font-semibold">{entry.indonesian}</strong>
            <span class="text-[var(--ti-text-soft)]">→</span>
            <span class="min-w-0 truncate text-[13px] text-[var(--ti-text-muted)]">{entry.english}</span>
            <button type="button" class="ti-button ti-button-secondary min-h-8 px-2.5" aria-label={`Remove ${entry.indonesian}`} disabled={saving} onclick={() => void removeTerm(index)}>
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <footer class="border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-3.5">
    <p class="m-0 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
      TranslateIT uses these preferences only when the matching word appears in the text.
    </p>
  </footer>
</article>
</section>

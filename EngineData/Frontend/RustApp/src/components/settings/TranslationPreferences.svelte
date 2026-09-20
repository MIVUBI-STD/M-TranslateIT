<script lang="ts">
  import { Plus, Trash2 } from "@lucide/svelte";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import type { RuntimeSettings, TerminologyEntry } from "../../app/shared/types";

  const MAX_TERMS = 24;

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
  let saving = $state(false);

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

  async function addTerm(): Promise<void> {
    const id = indonesian.trim();
    const en = english.trim();
    if (!id || !en) {
      onNotice("Enter both the Indonesian and English term.");
      return;
    }
    if (settings.terminology.length >= MAX_TERMS) {
      onNotice(`Preferred words is limited to ${MAX_TERMS} focused terms.`);
      return;
    }
    const conflicting = settings.terminology.find(
      (entry) =>
        entry.indonesian.toLocaleLowerCase() === id.toLocaleLowerCase()
        || entry.english.toLocaleLowerCase() === en.toLocaleLowerCase(),
    );
    if (conflicting) {
      onNotice("Each Indonesian and English term can have only one preferred mapping. Remove the existing term first.");
      return;
    }
    await persist([...settings.terminology, { indonesian: id, english: en }], "Preferred words saved.");
    indonesian = "";
    english = "";
  }

  async function removeTerm(index: number): Promise<void> {
    await persist(
      settings.terminology.filter((_, itemIndex) => itemIndex !== index),
      "Preferred words removed.",
    );
  }
</script>

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

  <div class="border-t border-[var(--ti-border)]">
    {#if settings.terminology.length === 0}
      <p class="m-0 px-5 py-5 text-[12.5px] text-[var(--ti-text-muted)]">
        No preferred words yet. You can add names, product terms, or other wording you want translated consistently.
      </p>
    {:else}
      <div class="divide-y divide-[var(--ti-border)]">
        {#each settings.terminology as entry, index (`${entry.indonesian}::${entry.english}`)}
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

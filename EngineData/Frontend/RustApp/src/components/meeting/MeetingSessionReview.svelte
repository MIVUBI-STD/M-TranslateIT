<script lang="ts">
  import { Check, Copy, Search } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurn, type MeetingCommittedTurnsSnapshot } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import MeetingTranscriptExport from "./MeetingTranscriptExport.svelte";

  let {
    sessionActive,
    onNotice,
  }: {
    sessionActive: boolean;
    onNotice: (message: string) => void;
  } = $props();

  let snapshot = $state<MeetingCommittedTurnsSnapshot | null>(null);
  let query = $state("");
  let copied = $state(false);
  let loadedForInactive = false;

  const visibleTurns = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const turns = snapshot?.turns ?? [];
    return needle ? turns.filter((turn) => `${turn.source_text} ${turn.translated_text}`.toLowerCase().includes(needle)) : turns;
  });

  async function loadReview(): Promise<void> {
    try {
      const result = await runtimeApi.getRecentMeetingTranscript();
      snapshot = result.ok && result.turns.length > 0 ? result : null;
    } catch {
      snapshot = null;
    }
  }

  async function copyAll(): Promise<void> {
    if (!snapshot?.turns.length) return;
    const value = snapshot.turns.map((turn: MeetingCommittedTurn) =>
      `${languageName(turn.source_language)}\n${turn.source_text}\n${languageName(turn.target_language)}\n${turn.translated_text}`
    ).join("\n\n");
    try {
      await navigator.clipboard.writeText(value);
      copied = true;
      onNotice("Session review copied.");
      window.setTimeout(() => { copied = false; }, 1400);
    } catch {
      onNotice("Couldn't copy the session review.");
    }
  }

  $effect(() => {
    if (sessionActive) {
      loadedForInactive = false;
      snapshot = null;
    } else if (!loadedForInactive) {
      loadedForInactive = true;
      void loadReview();
    }
  });
</script>

{#if !sessionActive && snapshot?.turns.length}
  <article class="ti-panel mt-4 overflow-hidden">
    <header class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
      <div>
        <h3 class="m-0 text-[15px] font-semibold">Last session review</h3>
        <p class="mb-0 mt-1 text-[11.5px] text-[var(--ti-text-soft)]">{snapshot.turns.length} retained phrase{snapshot.turns.length === 1 ? "" : "s"} · temporary until a new Meeting starts or the app closes.</p>
      </div>
      <div class="flex items-center gap-2">
        <button type="button" class="ti-button ti-button-secondary min-h-9" onclick={() => void copyAll()}>
          {#if copied}<Check size={14} />{:else}<Copy size={14} />{/if}{copied ? "Copied" : "Copy all"}
        </button>
        <MeetingTranscriptExport hasSession={false} {onNotice} />
      </div>
    </header>
    <div class="border-b border-[var(--ti-border)] p-3">
      <label class="relative block">
        <Search size={14} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" />
        <input class="ti-field min-h-9 w-full pl-9 pr-3 text-xs" bind:value={query} placeholder="Search last session..." aria-label="Search last session" />
      </label>
    </div>
    <div class="max-h-[360px] overflow-y-auto divide-y divide-[var(--ti-border)]">
      {#each visibleTurns as turn (turn.sequence)}
        <div class="grid gap-2 px-5 py-3.5">
          <div class="grid grid-cols-[72px_minmax(0,1fr)] gap-2">
            <span class="text-[10.5px] font-semibold text-[var(--ti-text-soft)]">{languageName(turn.source_language)}</span>
            <p class="m-0 text-[13px] leading-5">{turn.source_text}</p>
          </div>
          <div class="grid grid-cols-[72px_minmax(0,1fr)] gap-2">
            <span class="text-[10.5px] font-semibold text-[var(--ti-text-soft)]">{languageName(turn.target_language)}</span>
            <p class="m-0 text-[12.5px] leading-5 text-[var(--ti-text-muted)]">{turn.translated_text}</p>
          </div>
        </div>
      {/each}
      {#if visibleTurns.length === 0}<p class="m-0 px-5 py-6 text-sm text-[var(--ti-text-muted)]">No phrase matches this search.</p>{/if}
    </div>
  </article>
{/if}

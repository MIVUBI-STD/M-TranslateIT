<script lang="ts">
  import { Check, Copy, Search } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurn, type MeetingCommittedTurnsSnapshot } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import MeetingTranscriptExport from "./MeetingTranscriptExport.svelte";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";
  import { meetingFirstRecordedTime, meetingSessionTitle, meetingSpeakerLabel, meetingTime } from "../../app/runtime/meetingTranscriptPresentation";

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
  let reviewOpen = $state(false);
  let loadedForInactive = false;

  const sessionLabel = $derived(meetingSessionTitle(meetingFirstRecordedTime(snapshot?.turns ?? [])));
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
      `${meetingSpeakerLabel(turn.lane)} · ${meetingTime(turn.created_unix_ms)}\n${languageName(turn.source_language)}: ${turn.source_text}\n${languageName(turn.target_language)}: ${turn.translated_text}`
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
      reviewOpen = false;
      snapshot = null;
    } else if (!loadedForInactive) {
      loadedForInactive = true;
      void loadReview();
    }
  });
</script>

{#if !sessionActive && snapshot?.turns.length}
  <details bind:open={reviewOpen} class="ti-panel mt-4 overflow-hidden">
    <summary class="cursor-pointer px-5 py-4 hover:bg-[var(--ti-surface-soft)]">
      <strong class="block text-[15px] font-semibold">{sessionLabel} · Last session</strong>
      <span class="mt-1 block text-sm text-[var(--ti-text-muted)]">{snapshot.turns.length} recorded phrase{snapshot.turns.length === 1 ? "" : "s"} · Person 1 groups incoming audio (speaker unverified).</span>
      <span class="mt-1 block text-xs text-[var(--ti-warning)]">Only this meeting is available until the next meeting or app exit. Export manually to keep it.</span>
    </summary>
    <div class="border-t border-[var(--ti-border)]">
      <div class="flex flex-wrap items-center justify-end gap-2 px-5 py-3">
        <button type="button" class="ti-button ti-button-secondary min-h-9" onclick={() => void copyAll()}>
          {#if copied}<Check size={14} />{:else}<Copy size={14} />{/if}{copied ? "Copied" : "Copy all"}
        </button>
        <MeetingTranscriptExport hasSession={false} {onNotice} />
      </div>
      <div class="border-t border-[var(--ti-border)] px-5 py-3">
        <label class="relative block">
          <Search size={14} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" />
          <input class="ti-field min-h-10 w-full pl-9 pr-3 text-sm" bind:value={query} placeholder="Search last session..." aria-label="Search last session" />
        </label>
      </div>
      <div class="max-h-[360px] overflow-y-auto divide-y divide-[var(--ti-border)]">
        {#each visibleTurns as turn (turn.sequence)}
          <div class="px-4 py-3">
            <MeetingTurnBubble {turn} />
          </div>
        {/each}
        {#if visibleTurns.length === 0}
          <p class="m-0 px-5 py-6 text-sm text-[var(--ti-text-muted)]">No phrase matches this search.</p>
        {/if}
      </div>
    </div>
  </details>
{/if}

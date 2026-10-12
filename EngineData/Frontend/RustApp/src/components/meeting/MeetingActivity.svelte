<script lang="ts">
  import { tick } from "svelte";
  import { Check, Copy, Pin, PinOff, Search } from "@lucide/svelte";
  import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { mapProductMeetingState } from "../../app/bridge/productMeetingState";
  import { languageName } from "../../app/shared/state";
  import { meetingSessionTitle, meetingSpeakerLabel, meetingTime } from "../../app/runtime/meetingTranscriptPresentation";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";

  let {
    status,
    turns,
  }: {
    status: MeetingSessionStatus;
    turns: MeetingCommittedTurnsSnapshot | null;
  } = $props();

  let searchOpen = $state(false);
  let searchQuery = $state("");
  let pinnedSequences = $state<Set<number>>(new Set());
  let pinnedOnly = $state(false);
  let copiedSequence = $state<number | null>(null);
  let copyFailedSequence = $state<number | null>(null);
  let followingLatest = $state(true);
  let transcriptScroller = $state<HTMLDivElement>();
  let searchInput = $state<HTMLInputElement>();

  const meeting = $derived(mapProductMeetingState(status));
  const orderedTurns = $derived(
    turns?.ok && turns.has_session && turns.session_id === status.session_id
      ? [...turns.turns].sort((a, b) => a.sequence - b.sequence)
      : [],
  );
  const visibleTurns = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase();
    return orderedTurns.filter((turn) => {
      if (pinnedOnly && !pinnedSequences.has(turn.sequence)) return false;
      return !query || `${turn.source_text} ${turn.translated_text}`.toLowerCase().includes(query);
    });
  });

  const activity = $derived.by((): { label: string; detail: string; warning: boolean } => {
    if (meeting.busy) return { label: meeting.label, detail: "", warning: false };
    if (status.outbound.output_active) {
      return { label: "Speaking", detail: "", warning: false };
    }
    if (status.outbound.stage === "attention_needed") {
      return {
        label: "Needs attention",
        detail: "The last phrase could not be completed. Try speaking again.",
        warning: true,
      };
    }
    if (["transcribing", "translating", "synthesizing", "queued"].includes(status.outbound.stage)) {
      return { label: "Translating", detail: "", warning: false };
    }
    if (["listening", "idle"].includes(status.outbound.stage)) {
      return { label: "Listening", detail: "", warning: false };
    }
    return { label: "Translation on", detail: "", warning: false };
  });

  const latestSequence = $derived(orderedTurns[orderedTurns.length - 1]?.sequence ?? 0);
  $effect(() => {
    const sessionId = status.session_id;
    const sequence = latestSequence;
    if (!sequence || !followingLatest || searchQuery.trim() || pinnedOnly) return;
    void tick().then(() => {
      if (status.session_id !== sessionId || !followingLatest || searchQuery.trim() || pinnedOnly || !transcriptScroller) return;
      transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
    });
  });

  function updateScrollPosition(): void {
    if (!transcriptScroller || searchQuery.trim() || pinnedOnly) return;
    const distance = transcriptScroller.scrollHeight - transcriptScroller.scrollTop - transcriptScroller.clientHeight;
    followingLatest = distance < 72;
  }

  function jumpToLatest(): void {
    searchQuery = "";
    pinnedOnly = false;
    searchOpen = false;
    followingLatest = true;
    void tick().then(() => {
      if (transcriptScroller) transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
    });
  }

  async function toggleSearch(): Promise<void> {
    searchOpen = !searchOpen;
    if (!searchOpen) {
      searchQuery = "";
      pinnedOnly = false;
      return;
    }
    await tick();
    searchInput?.focus();
  }

  function togglePin(sequence: number): void {
    const next = new Set(pinnedSequences);
    if (next.has(sequence)) next.delete(sequence);
    else next.add(sequence);
    pinnedSequences = next;
    if (pinnedOnly && next.size === 0) pinnedOnly = false;
  }

  function deliveryLabel(turn: MeetingCommittedTurn): string {
    switch (turn.delivery_state) {
      case "speaking": return "Playing";
      case "output_complete": return "Played";
      case "output_failed": return "Not played";
      case "interrupted": return "Interrupted";
      default: return "Preparing";
    }
  }

  async function copyTurn(turn: MeetingCommittedTurn): Promise<void> {
    const value = `${meetingSpeakerLabel(turn.lane)} · ${meetingTime(turn.created_unix_ms)}\n${languageName(turn.source_language)}: ${turn.source_text}\n${languageName(turn.target_language)}: ${turn.translated_text}`;
    copyFailedSequence = null;
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard unavailable");
      await navigator.clipboard.writeText(value);
      copiedSequence = turn.sequence;
      window.setTimeout(() => {
        if (copiedSequence === turn.sequence) copiedSequence = null;
      }, 1400);
    } catch {
      copiedSequence = null;
      copyFailedSequence = turn.sequence;
    }
  }
</script>

<section class="grid gap-4">
  <div class="flex flex-wrap items-center justify-between gap-3" role="status" aria-live="polite" aria-atomic="true">
    <div class="flex items-center gap-3">
      <span class={`size-2 shrink-0 rounded-full ${activity.warning ? "bg-[var(--ti-warning)]" : "bg-[var(--ti-success)]"}`} aria-hidden="true"></span>
      <strong class="text-base font-semibold">{activity.label}</strong>
    </div>
    {#if status.incoming.capture_active && !status.incoming.degraded}
      <span class="text-sm text-[var(--ti-text-muted)]">Person 1 audio capture active · speaker unverified</span>
    {/if}
  </div>

  {#if activity.warning}
    <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]">{activity.detail}</p>
  {/if}
  {#if status.incoming.degraded}
    <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]">Incoming captions are unavailable. Your voice translation can continue.</p>
  {/if}

  <section class="border-t border-[var(--ti-border)] pt-4" aria-label="Translated conversation">
    <header class="flex flex-wrap items-center justify-between gap-3 pb-3">
      <div class="flex items-baseline gap-3">
        <h3 class="m-0 text-base font-semibold">Transcript</h3>
        <span class="text-xs text-[var(--ti-text-muted)]">{meetingSessionTitle(status.started_unix_ms)}</span>
        {#if orderedTurns.length > 0}
          <span class="text-sm text-[var(--ti-text-muted)]">{orderedTurns.length} phrase{orderedTurns.length === 1 ? "" : "s"}</span>
        {/if}
      </div>
      <span class="text-xs text-[var(--ti-text-muted)]">Person 1 groups incoming audio; individual speakers are not identified.</span>
      {#if orderedTurns.length > 1 || searchOpen}
        <button type="button" class="ti-button ti-button-secondary min-h-9"
          aria-expanded={searchOpen} aria-controls="meeting-conversation-search"
          onclick={() => void toggleSearch()}>
          <Search size={15} aria-hidden="true" /> {searchOpen ? "Close search" : "Find phrases"}
        </button>
      {/if}
    </header>

    {#if searchOpen}
      <div id="meeting-conversation-search" class="flex flex-wrap items-center gap-2 pb-3">
        <label class="min-w-[170px] flex-1">
          <span class="sr-only">Search conversation</span>
          <input bind:this={searchInput} class="ti-field min-h-10 px-3 text-sm"
            bind:value={searchQuery} placeholder="Search conversation..." aria-label="Search conversation" />
        </label>
        {#if pinnedSequences.size > 0}
          <button type="button" class="ti-button ti-button-secondary min-h-10"
            aria-pressed={pinnedOnly}
            onclick={() => { pinnedOnly = !pinnedOnly; }}>
            <Pin size={14} aria-hidden="true" /> Pinned ({pinnedSequences.size})
          </button>
        {/if}
      </div>
    {/if}

    {#if !turns || !turns.ok || !turns.has_session || turns.session_id !== status.session_id}
      <p class="m-0 py-6 text-sm leading-6 text-[var(--ti-text-muted)]">Conversation is temporarily unavailable. You can still stop translation.</p>
    {:else}
      {#if turns.truncated || turns.dropped_turn_count > 0}
        <p class="m-0 border-b border-[var(--ti-warning-border)] py-3 text-sm leading-5 text-[var(--ti-warning)]">
          Earlier phrases may no longer be available in this session ({turns.dropped_turn_count} dropped).
        </p>
      {/if}
      {#if orderedTurns.length === 0}
        <p class="m-0 py-7 text-sm leading-6 text-[var(--ti-text-muted)]">Translated phrases from You and incoming audio (Person 1) will appear here.</p>
      {:else}
        <div bind:this={transcriptScroller} onscroll={updateScrollPosition}
          class="max-h-[54vh] min-h-[160px] overflow-y-auto" role="log" aria-label="Meeting conversation" aria-live="off">
          {#if visibleTurns.length === 0}
            <p class="m-0 py-7 text-sm text-[var(--ti-text-muted)]">{pinnedOnly ? "No pinned phrase matches this search." : "No phrase matches this search."}</p>
          {/if}
          {#each visibleTurns as turn (turn.sequence)}
            <div class="border-b border-[var(--ti-border)] py-3 last:border-b-0">
              <MeetingTurnBubble {turn} statusLabel={turn.lane === "you" ? deliveryLabel(turn) : ""} />
              <div class={"mt-1 flex items-center gap-1 " + (turn.lane === "you" ? "justify-end" : "justify-start")}>
                <button type="button" class="grid size-8 place-items-center rounded-[7px] text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-raised)]"
                  aria-label={pinnedSequences.has(turn.sequence) ? "Unpin phrase" : "Pin phrase"}
                  title={pinnedSequences.has(turn.sequence) ? "Unpin phrase" : "Pin phrase"}
                  onclick={() => togglePin(turn.sequence)}>
                  {#if pinnedSequences.has(turn.sequence)}<PinOff size={15} />{:else}<Pin size={15} />{/if}
                </button>
                <button type="button" class="grid size-8 place-items-center rounded-[7px] text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-raised)]"
                  aria-label="Copy bilingual phrase" title="Copy source and translation"
                  onclick={() => void copyTurn(turn)}>
                  {#if copiedSequence === turn.sequence}<Check size={15} />{:else}<Copy size={15} />{/if}
                </button>
              </div>
              {#if copyFailedSequence === turn.sequence}
                <p class="m-0 text-sm text-[var(--ti-warning)]" role="alert">Couldn't copy this phrase. Try again.</p>
              {/if}
            </div>
          {/each}
        </div>
        {#if (!followingLatest || searchQuery.trim() || pinnedOnly) && orderedTurns.length > 0}
          <div class="flex justify-end pt-3">
            <button type="button" class="ti-button ti-button-secondary min-h-9"
              onclick={jumpToLatest}>Jump to latest</button>
          </div>
        {/if}
      {/if}
    {/if}
  </section>
</section>

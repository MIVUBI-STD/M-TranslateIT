<script lang="ts">
  import { tick } from "svelte";
  import { Check, Copy } from "@lucide/svelte";
  import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { mapProductMeetingState } from "../../app/bridge/productMeetingState";
  import { languageName } from "../../app/shared/state";
  import { meetingSessionTitle, meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";

  let {
    status,
    turns,
  }: {
    status: MeetingSessionStatus;
    turns: MeetingCommittedTurnsSnapshot | null;
  } = $props();

  let copiedSequence = $state<number | null>(null);
  let copyFailedSequence = $state<number | null>(null);
  let followingLatest = $state(true);
  let transcriptScroller = $state<HTMLDivElement>();

  const meeting = $derived(mapProductMeetingState(status));
  const orderedTurns = $derived(
    turns?.ok && turns.has_session && turns.session_id === status.session_id
      ? [...turns.turns].sort((a, b) => a.sequence - b.sequence)
      : [],
  );
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
    return { label: "Audio status unavailable", detail: "Check Meeting status.", warning: true };
  });

  const latestSequence = $derived(orderedTurns[orderedTurns.length - 1]?.sequence ?? 0);
  $effect(() => {
    const sessionId = status.session_id;
    const sequence = latestSequence;
    if (!sequence || !followingLatest) return;
    void tick().then(() => {
      if (status.session_id !== sessionId || !followingLatest || !transcriptScroller) return;
      transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
    });
  });

  function updateScrollPosition(): void {
    if (!transcriptScroller) return;
    const distance = transcriptScroller.scrollHeight - transcriptScroller.scrollTop - transcriptScroller.clientHeight;
    followingLatest = distance < 72;
  }

  function jumpToLatest(): void {
    followingLatest = true;
    void tick().then(() => {
      if (transcriptScroller) transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
    });
  }

  function deliveryLabel(turn: MeetingCommittedTurn): string {
    if (turn.translation_state === "failed") return "Translation unavailable";
    if (turn.translation_state === "pending") return "Translating";
    switch (turn.delivery_state) {
      case "speaking": return "Playing locally";
      case "output_complete": return "Local playback complete";
      case "output_failed": return "Not played";
      case "interrupted": return "Interrupted";
      default: return "Preparing";
    }
  }

  async function copyTurn(turn: MeetingCommittedTurn): Promise<void> {
    const value = `${meetingSpeakerLabel(turn.lane)} · ${meetingTime(turn.created_unix_ms)}\n${languageName(turn.source_language)}: ${turn.source_text}\n${languageName(turn.target_language)}: ${meetingTranslationDisplay(turn)}`;
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
      <span class="text-sm text-[var(--ti-text-muted)]">Meeting audio capture active · speakers not identified</span>
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
      <span class="text-xs text-[var(--ti-text-muted)]">Meeting audio is combined; individual speakers are not identified.</span>
    </header>

    {#if !turns || !turns.ok || !turns.has_session || turns.session_id !== status.session_id}
      <p class="m-0 py-6 text-sm leading-6 text-[var(--ti-text-muted)]">Conversation is temporarily unavailable. You can still stop translation.</p>
    {:else}
      {#if turns.truncated || turns.dropped_turn_count > 0}
        <p class="m-0 border-b border-[var(--ti-warning-border)] py-3 text-sm leading-5 text-[var(--ti-warning)]">
          Earlier phrases may no longer be available in this session ({turns.dropped_turn_count} dropped).
        </p>
      {/if}
      {#if orderedTurns.length === 0}
        <p class="m-0 py-7 text-sm leading-6 text-[var(--ti-text-muted)]">Recognized phrases from your microphone and meeting audio will appear here.</p>
      {:else}
        <div bind:this={transcriptScroller} onscroll={updateScrollPosition}
          class="max-h-[54vh] min-h-[160px] overflow-y-auto" role="log" aria-label="Meeting conversation" aria-live="off">
          {#each orderedTurns as turn (turn.sequence)}
            <div class="border-b border-[var(--ti-border)] py-3 last:border-b-0">
              <MeetingTurnBubble {turn} statusLabel={turn.lane === "you" ? deliveryLabel(turn) : ""} />
              <div class={"mt-1 flex items-center gap-1 " + (turn.lane === "you" ? "justify-end" : "justify-start")}>
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
        {#if !followingLatest && orderedTurns.length > 0}
          <div class="flex justify-end pt-3">
            <button type="button" class="ti-button ti-button-secondary min-h-9"
              onclick={jumpToLatest}>Jump to latest</button>
          </div>
        {/if}
      {/if}
    {/if}
  </section>
</section>

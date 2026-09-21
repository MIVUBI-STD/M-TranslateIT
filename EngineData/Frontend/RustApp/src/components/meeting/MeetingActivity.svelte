<script lang="ts">
  import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { mapProductMeetingState } from "../../app/bridge/runtimeProductFacade";
  import { languageName } from "../../app/shared/state";
  import { Check, Copy, Pin, PinOff, Search } from "@lucide/svelte";
  import StatusBadge from "../ui/StatusBadge.svelte";

  let {
    status,
    turns,
  }: {
    status: MeetingSessionStatus;
    turns: MeetingCommittedTurnsSnapshot | null;
  } = $props();

  let searchQuery = $state("");
  let pinnedSequences = $state<Set<number>>(new Set());
  let copiedSequence = $state<number | null>(null);
  let pinnedOnly = $state(false);

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
      if (!query) return true;
      return `${turn.source_text} ${turn.translated_text}`.toLowerCase().includes(query);
    });
  });

  function stageCopy(stage: string): { label: string; title: string; detail: string; tone: "neutral" | "good" | "warning" } {
    switch (stage) {
      case "transcribing":
      case "translating":
      case "synthesizing":
        return {
          label: "Translating",
          title: "Translating what you said",
          detail: "Preparing the English translation for the call.",
          tone: "good",
        };
      case "delivering":
        return {
          label: "Speaking",
          title: "Speaking the English translation",
          detail: "Other people in the call are hearing the English translation.",
          tone: "good",
        };
      case "attention_needed":
        return {
          label: "Needs attention",
          title: "The last phrase could not be translated",
          detail: "You can keep the meeting open. If this keeps happening, open Help in Settings.",
          tone: "warning",
        };
      case "listening":
        return {
          label: "Listening",
          title: status.outbound.utterance_sequence > 0 ? "Ready for the next phrase" : "Listening for Indonesian",
          detail: "Speak normally. TranslateIT starts after you finish a phrase.",
          tone: "good",
        };
      default:
        return {
          label: meeting.busy ? meeting.label : "Live",
          title: meeting.busy ? meeting.label : "Meeting translation is active",
          detail: meeting.busy ? "TranslateIT is updating the call connection." : "Speak Indonesian normally. TranslateIT handles the rest.",
          tone: meeting.busy ? "neutral" : "good",
        };
    }
  }

  const activity = $derived(stageCopy(status.outbound.stage));

  function deliveryLabel(turn: MeetingCommittedTurn): string {
    switch (turn.delivery_state) {
      case "speaking": return "Speaking";
      case "output_complete": return "Sent";
      case "output_failed": return "Not sent";
      case "interrupted": return "Interrupted";
      default: return "Preparing";
    }
  }

  function incomingCopy(): { label: string; badge: string; tone: "neutral" | "good" | "warning" } {
    const incoming = status.incoming;
    if (incoming.degraded) {
      return { label: "Translation of what you hear is unavailable. Your voice translation can continue.", badge: "Unavailable", tone: "warning" };
    }
    if (incoming.suppressed || incoming.stage === "suppressed") {
      return { label: "Translation of what you hear pauses briefly while TranslateIT speaks.", badge: "Paused", tone: "neutral" };
    }
    if (incoming.capture_active) {
      return {
        label: incoming.stage === "transcribing" || incoming.stage === "translating"
          ? "Translating what you hear."
          : "Listening to speech in the call.",
        badge: incoming.stage === "transcribing" || incoming.stage === "translating" ? "Translating" : "Listening",
        tone: "good",
      };
    }
    return { label: "Translation of what you hear is optional and currently off.", badge: "Optional", tone: "neutral" };
  }

  const incoming = $derived(incomingCopy());

  function togglePin(sequence: number): void {
    const next = new Set(pinnedSequences);
    if (next.has(sequence)) next.delete(sequence);
    else next.add(sequence);
    pinnedSequences = next;
  }

  async function copyTurn(turn: MeetingCommittedTurn): Promise<void> {
    const value = `${languageName(turn.source_language)}\n${turn.source_text}\n\n${languageName(turn.target_language)}\n${turn.translated_text}`;
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard unavailable");
      await navigator.clipboard.writeText(value);
      copiedSequence = turn.sequence;
      window.setTimeout(() => {
        if (copiedSequence === turn.sequence) copiedSequence = null;
      }, 1400);
    } catch {
      copiedSequence = null;
    }
  }
</script>

<section class="grid gap-4">
  <header class="flex items-start justify-between gap-6" aria-live="polite">
    <div class="min-w-0">
      <span class="ti-kicker">Live</span>
      <h3 class="mb-0 mt-2 text-lg font-semibold tracking-[-0.015em]">{activity.title}</h3>
      <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">{activity.detail}</p>
    </div>
    <StatusBadge label={activity.label} tone={activity.tone} />
  </header>

  <div class="flex items-center justify-between gap-5 border-y border-[var(--ti-border)] py-3">
    <p class="m-0 text-xs leading-5 text-[var(--ti-text-soft)]">{incoming.label}</p>
    {#if incoming.tone === "warning"}
      <StatusBadge label={incoming.badge} tone={incoming.tone} />
    {/if}
  </div>

  <section class="overflow-hidden rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface)]" aria-label="Translated conversation">
    <header class="grid gap-3 border-b border-[var(--ti-border)] px-5 py-4">
      <div class="flex items-center justify-between gap-4">
        <div>
          <strong class="block text-sm font-semibold">Conversation</strong>
          <span class="mt-1 block text-[11px] text-[var(--ti-text-soft)]">Search, pin, or copy phrases without saving a permanent history.</span>
        </div>
        <span class="text-xs text-[var(--ti-text-soft)]">{orderedTurns.length} phrase{orderedTurns.length === 1 ? "" : "s"}</span>
      </div>
      <div class="flex items-center gap-2">
        <label class="relative min-w-0 flex-1">
          <Search size={14} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" />
          <input
            class="ti-field min-h-9 w-full pl-9 pr-3 text-[12px]"
            bind:value={searchQuery}
            placeholder="Search this session..."
            aria-label="Search conversation"
          />
        </label>
        <button
          type="button"
          class={`ti-button ti-button-secondary min-h-9 px-3 text-xs ${pinnedOnly ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" : ""}`}
          aria-pressed={pinnedOnly}
          onclick={() => { pinnedOnly = !pinnedOnly; }}
        >
          <Pin size={14} /> Pinned {pinnedSequences.size > 0 ? `(${pinnedSequences.size})` : ""}
        </button>
      </div>
    </header>

    {#if !turns || !turns.ok || !turns.has_session || turns.session_id !== status.session_id}
      <p class="m-0 px-5 py-5 text-sm leading-6 text-[var(--ti-text-muted)]">Conversation is temporarily unavailable. You can still stop translation normally.</p>
    {:else}
      {#if turns.truncated || turns.dropped_turn_count > 0}
        <p class="m-0 border-b border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-5 py-3 text-xs leading-5 text-[var(--ti-warning)]">
          {turns.dropped_turn_count} earlier phrase{turns.dropped_turn_count === 1 ? " is" : "s are"} no longer shown here.
        </p>
      {/if}

      {#if orderedTurns.length === 0}
        <p class="m-0 px-5 py-8 text-sm leading-6 text-[var(--ti-text-muted)]">Your conversation will appear here after you finish the first phrase.</p>
      {:else}
        <div class="max-h-[430px] overflow-y-auto">
          {#if visibleTurns.length === 0}
            <p class="m-0 px-5 py-7 text-sm leading-6 text-[var(--ti-text-muted)]">{pinnedOnly ? "No pinned phrase matches this search." : "No phrase matches this search."}</p>
          {/if}
          {#each visibleTurns as turn (turn.sequence)}
            <article class={`border-b border-[var(--ti-border)] px-5 py-4 last:border-b-0 ${pinnedSequences.has(turn.sequence) ? "bg-[var(--ti-surface-soft)]" : ""}`}>
              <header class="mb-3 flex items-center justify-between gap-4">
                <span class="text-[11px] font-semibold tracking-[0.08em] text-[var(--ti-text-muted)]">{turn.lane === "incoming" ? "MEETING" : "YOU"}</span>
                <div class="flex items-center gap-1.5">
                  {#if turn.lane !== "incoming"}
                    <span class="mr-1 text-xs text-[var(--ti-text-soft)]">{deliveryLabel(turn)}</span>
                  {/if}
                  <button
                    type="button"
                    class="grid size-7 place-items-center rounded-[7px] text-[var(--ti-text-soft)] hover:bg-[var(--ti-surface-raised)] hover:text-[var(--ti-text)]"
                    aria-label={pinnedSequences.has(turn.sequence) ? "Unpin phrase" : "Pin phrase"}
                    title={pinnedSequences.has(turn.sequence) ? "Unpin" : "Pin"}
                    onclick={() => togglePin(turn.sequence)}
                  >
                    {#if pinnedSequences.has(turn.sequence)}<PinOff size={14} />{:else}<Pin size={14} />{/if}
                  </button>
                  <button
                    type="button"
                    class="grid size-7 place-items-center rounded-[7px] text-[var(--ti-text-soft)] hover:bg-[var(--ti-surface-raised)] hover:text-[var(--ti-text)]"
                    aria-label="Copy bilingual phrase"
                    title="Copy source + translation"
                    onclick={() => void copyTurn(turn)}
                  >
                    {#if copiedSequence === turn.sequence}<Check size={14} />{:else}<Copy size={14} />{/if}
                  </button>
                </div>
              </header>
              <div class="grid grid-cols-[78px_minmax(0,1fr)] gap-2">
                <span class="pt-1 text-[11px] font-semibold text-[var(--ti-text-soft)]">{languageName(turn.source_language)}</span>
                <p class="m-0 text-[15px] font-medium leading-6" lang={turn.source_language}>{turn.source_text}</p>
              </div>
              <div class="mt-2 grid grid-cols-[78px_minmax(0,1fr)] gap-2">
                <span class="pt-1 text-[11px] font-semibold text-[var(--ti-text-soft)]">{languageName(turn.target_language)}</span>
                <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]" lang={turn.target_language}>{turn.translated_text}</p>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    {/if}
  </section>
</section>

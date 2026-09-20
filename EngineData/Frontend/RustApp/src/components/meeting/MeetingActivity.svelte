<script lang="ts">
  import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { mapProductMeetingState } from "../../app/bridge/runtimeProductFacade";
  import StatusBadge from "../ui/StatusBadge.svelte";

  let {
    status,
    turns,
  }: {
    status: MeetingSessionStatus;
    turns: MeetingCommittedTurnsSnapshot | null;
  } = $props();

  const meeting = $derived(mapProductMeetingState(status));
  const orderedTurns = $derived(
    turns?.ok && turns.has_session && turns.session_id === status.session_id
      ? [...turns.turns].sort((a, b) => a.sequence - b.sequence)
      : [],
  );

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
          ? "Translating what you hear into Indonesian text."
          : "Listening for English speech in the call.",
        badge: incoming.stage === "transcribing" || incoming.stage === "translating" ? "Translating" : "Listening",
        tone: "good",
      };
    }
    return { label: "Translation of what you hear is optional and currently off.", badge: "Optional", tone: "neutral" };
  }

  const incoming = $derived(incomingCopy());
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

  <section class="overflow-hidden rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface)]" aria-label="Meeting transcript">
    <header class="flex items-center justify-between border-b border-[var(--ti-border)] px-5 py-4">
      <div>
        <strong class="block text-sm font-semibold">Conversation</strong>
        <span class="mt-1 block text-[11px] text-[var(--ti-text-soft)]">What you said and what others heard</span>
      </div>
      <span class="text-xs text-[var(--ti-text-soft)]">{orderedTurns.length} turn{orderedTurns.length === 1 ? "" : "s"}</span>
    </header>

    {#if !turns || !turns.ok || !turns.has_session || turns.session_id !== status.session_id}
      <p class="m-0 px-5 py-5 text-sm leading-6 text-[var(--ti-text-muted)]">Conversation is temporarily unavailable. You can still stop translation normally.</p>
    {:else}
      {#if turns.truncated || turns.dropped_turn_count > 0}
        <p class="m-0 border-b border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-5 py-3 text-xs leading-5 text-[var(--ti-warning)]">
          {turns.dropped_turn_count} earlier turn{turns.dropped_turn_count === 1 ? " is" : "s are"} no longer shown here.
        </p>
      {/if}

      {#if orderedTurns.length === 0}
        <p class="m-0 px-5 py-8 text-sm leading-6 text-[var(--ti-text-muted)]">Your conversation will appear here after you finish the first phrase.</p>
      {:else}
        <div class="max-h-[430px] overflow-y-auto">
          {#each orderedTurns as turn (turn.sequence)}
            <article class="border-b border-[var(--ti-border)] px-5 py-4 last:border-b-0">
              <header class="mb-3 flex items-center justify-between gap-4">
                <span class="text-[11px] font-semibold tracking-[0.08em] text-[var(--ti-text-muted)]">{turn.lane === "incoming" ? "MEETING" : "YOU"}</span>
                {#if turn.lane !== "incoming"}
                  <span class="text-xs text-[var(--ti-text-soft)]">{deliveryLabel(turn)}</span>
                {/if}
              </header>
              <div class="grid grid-cols-[26px_minmax(0,1fr)] gap-2">
                <span class="pt-1 text-[11px] font-semibold text-[var(--ti-text-soft)]">Indonesian</span>
                <p class="m-0 text-[15px] font-medium leading-6" lang="id">{turn.lane === "incoming" ? turn.translated_text : turn.source_text}</p>
              </div>
              <div class="mt-2 grid grid-cols-[26px_minmax(0,1fr)] gap-2">
                <span class="pt-1 text-[11px] font-semibold text-[var(--ti-text-soft)]">English</span>
                <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]" lang="en">{turn.lane === "incoming" ? turn.source_text : turn.translated_text}</p>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    {/if}
  </section>
</section>

<script lang="ts">
  import type { MeetingCommittedTurn } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerDescription, meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";

  let { turn, statusLabel = "" }: { turn: MeetingCommittedTurn; statusLabel?: string } = $props();
  const own = $derived(turn.lane === "you");
</script>

<div class={"flex w-full " + (own ? "justify-end" : "justify-start")}>
  <article class={"w-[min(88%,690px)] min-w-0 rounded-[14px] border px-4 py-3 " +
    (own ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" :
      "border-[var(--ti-border)] bg-[var(--ti-surface-soft)]")}
    aria-label={meetingSpeakerLabel(turn.lane)}>
    <header class="mb-2 flex flex-wrap items-center justify-between gap-2">
      <strong class="text-[13px] font-semibold" title={meetingSpeakerDescription(turn.lane)}>
        {meetingSpeakerLabel(turn.lane)}
      </strong>
      <span class="text-[11px] text-[var(--ti-text-muted)]">{meetingTime(turn.created_unix_ms)}</span>
    </header>
    <div class="grid gap-1">
      <span class="text-[11px] text-[var(--ti-text-muted)]">{languageName(turn.source_language)} · Original</span>
      <p class="m-0 break-words text-[14px] leading-6" lang={turn.source_language}>{turn.source_text}</p>
    </div>
    <div class="mt-3 border-t border-[var(--ti-border)] pt-2">
      <span class="text-[11px] text-[var(--ti-text-muted)]">{languageName(turn.target_language)} · Translation</span>
      <p class={"mb-0 mt-1 break-words text-[14px] leading-6 " +
        (turn.translation_state === "translated" ? "font-medium" : "text-[var(--ti-text-muted)]")}
        lang={turn.translation_state === "translated" ? turn.target_language : undefined}
        role={turn.translation_state === "failed" ? "status" : undefined}
      >{meetingTranslationDisplay(turn)}</p>
    </div>
    {#if statusLabel}
      <p class="mb-0 mt-2 text-right text-[11px] text-[var(--ti-text-muted)]">{statusLabel}</p>
    {/if}
  </article>
</div>

<script lang="ts">
  import type { MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { sanitizeDiagnosticText } from "../../app/shared/diagnosticPrivacy";

  let { status }: { status: MeetingSessionStatus | null } = $props();

  const timing = $derived(status?.outbound?.timing ?? null);
  const outbound = $derived(status?.outbound ?? null);

  function formatTiming(value: number | null | undefined): string {
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return "Not measured";
    return `${Math.round(value)} ms`;
  }

  function formatCount(value: number | null | undefined): string {
    return typeof value === "number" && Number.isFinite(value) && value >= 0
      ? String(Math.round(value))
      : "Not measured";
  }

  function formatRate(value: number | null | undefined): string {
    return typeof value === "number" && Number.isFinite(value) && value >= 0
      ? `${value.toFixed(1)} tok/s`
      : "Not measured";
  }

  const diagnosis = $derived.by(() => {
    if (!status?.has_session) return "No active Meeting session. Start a real session to collect timing evidence.";
    if ((outbound?.overflow_dropped_utterance_count ?? 0) > 0 ||
        (outbound?.evicted_pending_utterance_count ?? 0) > 0) {
      return "The runtime reports dropped or evicted finalized phrases. Inspect queue pressure before changing ASR or VAD settings.";
    }
    if (outbound?.blocker) {
      return `Latest outbound blocker: ${sanitizeDiagnosticText(outbound.blocker)}. Check the affected stage before changing models.`;
    }
    if (timing?.outbound_latency_ms == null) {
      return "First translated playback has not been measured for this phrase. Do not infer end-to-end latency from partial stages.";
    }
    const stages: Array<[string, number | null | undefined]> = [
      ["Queue", timing.queue_ms],
      ["Audio preparation", timing.audio_prepare_ms],
      ["ASR", timing.asr_ms],
      ["Translation", timing.translation_ms],
      ["Voice TTS", timing.tts_ms],
      ["Delivery", timing.delivery_ms],
    ];
    const measured = stages.filter((stage): stage is [string, number] =>
      typeof stage[1] === "number" && Number.isFinite(stage[1]) && stage[1] >= 0);
    if (!measured.length) return "No individual processing stage has valid timing evidence yet.";
    const largest = measured.reduce((best, stage) => stage[1] > best[1] ? stage : best);
    return `Largest recorded stage: ${largest[0]} (${formatTiming(largest[1])}). This is a measurement, not proof of the underlying bottleneck.`;
  });
</script>

<article class="ti-panel p-5">
  <div>
    <span class="ti-field-label">Latest outbound phrase</span>
    <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Meeting performance</h4>
  </div>

  <div class="mt-4 grid grid-cols-2 gap-3 lg:grid-cols-5">
    <div class="ti-state-card"><span class="ti-field-label">After finalization</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.outbound_latency_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">ASR</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.asr_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Voice TTS</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.tts_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Delivery</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.delivery_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Estimated speech onset → first audio</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.estimated_onset_to_first_audio_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Estimated speech end → first audio</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.estimated_end_to_first_audio_ms)}</strong></div>
  </div>
  <p class="mt-2 text-[11px] leading-5 text-[var(--ti-text-soft)]">Speech onset/end estimates include VAD's buffered speech span and trailing silence. They are not precise microphone or remote-participant timestamps.</p>

  <div class="mt-3 grid grid-cols-3 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">ASR scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.asr_scheduler_wait_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_scheduler_wait_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">TTS scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.tts_scheduler_wait_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Output stream build</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.output_stream_build_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Output stream start → first callback</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.output_stream_start_to_first_audio_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3 lg:grid-cols-4">
    <div class="ti-state-card"><span class="ti-field-label">Translate tokenize</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_tokenization_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate inference</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_inference_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate decode</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_decode_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate throughput</span><strong class="mt-2 block text-[12px]">{formatRate(timing?.translation_tokens_per_second)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-[repeat(4,minmax(0,1fr))] gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Speech boundary</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.speech_boundary_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Finalization</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.finalization_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Queue wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.queue_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Audio prepare</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.audio_prepare_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card">
      <span class="ti-field-label">Overflow-dropped utterances</span>
      <strong class="mt-2 block text-[12px]">{formatCount(outbound?.overflow_dropped_utterance_count)}</strong>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Evicted pending utterances</span>
      <strong class="mt-2 block text-[12px]">{formatCount(outbound?.evicted_pending_utterance_count)}</strong>
    </div>
  </div>

  <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-muted)]" role="status">{diagnosis}</p>
  <p class="mb-0 mt-2 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
    Stage timings are observational evidence from this device only. ASR/translation/TTS totals already include scheduler wait, so do not add wait times to them. Measure whole-product VRAM with Windows/NVIDIA GPU tools (including browser and WDDM); no app-level VRAM figure is inferred.
  </p>
</article>

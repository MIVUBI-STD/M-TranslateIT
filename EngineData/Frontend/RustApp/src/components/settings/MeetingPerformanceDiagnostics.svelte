<script lang="ts">
  import type { MeetingSessionStatus } from "../../app/bridge/runtimeApi";

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
</script>

<article class="ti-panel p-5">
  <div>
    <span class="ti-field-label">Latest outbound phrase</span>
    <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Meeting performance</h4>
  </div>

  <div class="mt-4 grid grid-cols-[repeat(5,minmax(0,1fr))] gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Total</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.outbound_latency_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">ASR</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.asr_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Voice TTS</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.tts_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Delivery</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.delivery_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-[repeat(4,minmax(0,1fr))] gap-3">
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

  <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
    Use these stage timings to identify the first bottleneck. Measure whole-product VRAM with the Windows/NVIDIA GPU monitor because ASR and PyTorch use separate CUDA runtimes.
  </p>
</article>

<script lang="ts">
  import { Check, Play, Square } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    myVoiceBuildApi,
    type MyVoiceBuildActionResult,
    type MyVoiceBuildStatus,
  } from "../../app/bridge/myVoiceBuildApi";

  let {
    onNotice,
    onMeetingVoiceChanged,
    onRuntimeStateChanged,
    refreshRevision = 0,
  }: {
    onNotice: (message: string) => void;
    onMeetingVoiceChanged: (message?: string) => void | Promise<void>;
    onRuntimeStateChanged: () => void | Promise<void>;
    refreshRevision?: number;
  } = $props();

  let build = $state<MyVoiceBuildStatus>({
    active: false,
    generation: null,
    phase: "checking",
    message: "Checking My Voice recordings...",
    accepted_take_count: 0,
    accepted_duration_ms: 0,
    minimum_duration_ms: 60_000,
    missing_coverage: null,
    can_build: false,
    evaluation_ready: false,
    evaluation_samples: [],
    approved_voice_ready: false,
  });
  let authorized = $state(false);
  let busy = $state(false);
  let playingLineId = $state<number | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let audio: HTMLAudioElement | null = null;
  let audioUrl: string | null = null;
  let reviewedLineIds = $state<number[]>([]);

  const evaluationReviewComplete = $derived(
    build.evaluation_ready &&
      build.evaluation_samples.length > 0 &&
      build.evaluation_samples.every((sample) => reviewedLineIds.includes(sample.line_id)),
  );

  const speechProgress = $derived(
    build.minimum_duration_ms > 0
      ? Math.min(100, Math.round((build.accepted_duration_ms / build.minimum_duration_ms) * 100))
      : 100,
  );

  function formatDuration(milliseconds: number): string {
    const seconds = Math.max(0, Math.floor(milliseconds / 1000));
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
  }

  function stopAudio(): void {
    audio?.pause();
    audio = null;
    playingLineId = null;
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    audioUrl = null;
  }

  function schedulePoll(): void {
    if (timer) clearTimeout(timer);
    timer = null;
    if (!build.active) return;
    timer = setTimeout(() => void refresh(), 1400);
  }

  function applyStatus(next: MyVoiceBuildStatus): void {
    const buildFinished = build.active && !next.active;
    build = next;
    if (!next.evaluation_ready) reviewedLineIds = [];
    schedulePoll();
    if (buildFinished) void onRuntimeStateChanged();
  }

  function productMessage(result: MyVoiceBuildActionResult): string {
    switch (result.state) {
      case "building":
        return "Creating My Voice. You can leave My Voice open while it works.";
      case "approved":
        return "My Voice is ready for Meeting translation.";
      case "cancelled":
        return "My Voice creation stopped. Your accepted recordings were kept.";
      case "authorization_required":
        return "Confirm that this is your voice, or that you have permission to use it.";
      case "recording_active":
        return "Stop the current recording before creating My Voice.";
      case "build_active":
        return "My Voice is already being created.";
      case "more_recording_needed":
        return result.message || "Record a few more clear and varied lines before creating My Voice.";
      case "build_blocked":
        return "Stop Meeting translation before creating My Voice, then try again.";
      case "cancel_pending":
        return "My Voice is still stopping. Keep My Voice open and try again shortly.";
      case "evaluation_required":
      case "evaluation_listening_required":
        return "Listen to every voice preview before approving My Voice.";
      case "evaluation_candidate_mismatch":
        return "The review no longer matches the voice candidate. Create My Voice again.";
      case "approved":
        return "My Voice is ready and selected for meetings.";
      case "approved_cleanup_attention":
        return "My Voice is ready. Some temporary build files could not be cleared yet.";
      case "approval_failed":
        return "My Voice couldn't be approved. Try again.";
      case "dataset_prepare_failed":
      case "assets_unavailable":
      case "python_unavailable":
      case "build_state_invalid":
      case "build_storage_failed":
      case "build_log_failed":
      case "build_spawn_failed":
        return "My Voice couldn't be created. Try again.";
      case "cancel_failed":
        return "My Voice couldn't stop yet. Try again in a moment.";
      default:
        return result.ok ? "My Voice action completed." : "My Voice couldn't complete this action. Try again.";
    }
  }

  function activeTitle(): string {
    if (build.phase === "evaluating") return "Preparing voice previews";
    if (build.phase === "cancelling") return "Stopping My Voice creation";
    return "Creating My Voice";
  }

  function activeDetail(): string {
    if (build.phase === "preparing") return "Getting your accepted recordings ready.";
    if (build.phase === "training") return "Creating your English meeting voice from your accepted recordings.";
    if (build.phase === "evaluating") return "Preparing new sentences so you can listen before approving My Voice.";
    if (build.phase === "cancelling") return "Finishing the current stop safely.";
    return "My Voice is being created.";
  }

  function coverageGuidance(): string | null {
    const coverage = build.missing_coverage;
    if (!coverage || build.accepted_duration_ms < build.minimum_duration_ms) return null;
    return `Try one accepted line from Lines ${coverage.start_line_id}–${coverage.end_line_id} for ${coverage.label}.`;
  }

  function idleGuidance(): string {
    const coverage = coverageGuidance();
    if (build.approved_voice_ready && !build.can_build) {
      if (build.accepted_duration_ms < build.minimum_duration_ms) {
        return `Your current Meeting voice is ready. Keep recording if you want to create My Voice; ${formatDuration(build.minimum_duration_ms)} of usable speech is the minimum recording target.`;
      }
      if (coverage) return `Your current Meeting voice is ready. ${coverage}`;
      return "Your current Meeting voice is ready. Add more clear and varied recordings if you want to create My Voice again.";
    }
    if (!build.can_build && coverage) {
      return `Add a little more recording variety before creating My Voice. ${coverage}`;
    }
    if (!build.can_build && build.message.trim()) return build.message;
    if (!build.can_build) return "Keep recording clear and varied lines before creating My Voice.";
    return "Your accepted recordings are ready.";
  }

  function applyResult(result: MyVoiceBuildActionResult): void {
    applyStatus(result.build);
    onNotice(productMessage(result));
  }

  async function refresh(): Promise<void> {
    applyStatus(await myVoiceBuildApi.getStatus());
  }

  async function startBuild(): Promise<void> {
    if (busy || !build.can_build || !authorized) return;
    stopAudio();
    reviewedLineIds = [];
    busy = true;
    try {
      applyResult(await myVoiceBuildApi.start(authorized));
    } finally {
      busy = false;
    }
  }

  async function cancelBuild(): Promise<void> {
    if (busy || !build.active) return;
    busy = true;
    try {
      applyResult(await myVoiceBuildApi.cancel());
    } finally {
      busy = false;
    }
  }

  async function approve(): Promise<void> {
    if (busy || build.active || !build.evaluation_ready || !evaluationReviewComplete) return;
    stopAudio();
    busy = true;
    try {
      const result = await myVoiceBuildApi.approve(reviewedLineIds);
      applyResult(result);
      if (result.ok && result.state === "approved") {
        await onMeetingVoiceChanged(productMessage(result));
      }
    } finally {
      busy = false;
    }
  }

  async function playEvaluation(lineId: number): Promise<void> {
    if (busy || playingLineId !== null) return;
    const bytes = await myVoiceBuildApi.getEvaluationAudio(lineId);
    if (!bytes) {
      onNotice("This My Voice preview is unavailable.");
      return;
    }
    stopAudio();
    audioUrl = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
    audio = new Audio(audioUrl);
    playingLineId = lineId;
    audio.onended = () => {
      if (!reviewedLineIds.includes(lineId)) reviewedLineIds = [...reviewedLineIds, lineId];
      stopAudio();
    };
    audio.onerror = () => {
      stopAudio();
      onNotice("This My Voice preview couldn't be played.");
    };
    try {
      await audio.play();
    } catch {
      stopAudio();
      onNotice("This My Voice preview couldn't be played.");
    }
  }

  $effect(() => {
    refreshRevision;
    void refresh();
  });

  onMount(() => {
    return () => {
      if (timer) clearTimeout(timer);
      stopAudio();
    };
  });
</script>

<section class="ti-panel p-6" aria-busy={build.active || busy}>
  <div class="flex items-start justify-between gap-5">
    <div>
      <span class="ti-kicker">My Voice</span>
      <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Create My Voice</h3>
      <p class="mb-0 mt-2 max-w-[680px] text-sm leading-6 text-[var(--ti-text-muted)]">Use your accepted recordings to create a personalized English voice. You can listen before choosing it.</p>
    </div>
    {#if build.approved_voice_ready}
      <span class="flex items-center gap-1.5 text-sm font-semibold text-[var(--ti-success)]"><Check size={15} />My Voice ready</span>
    {/if}
  </div>

  <div class="mt-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3.5">
    <div class="flex items-center justify-between gap-4">
      <strong class="text-sm font-semibold">Ready recordings</strong>
      <span class="text-xs font-semibold text-[var(--ti-text-muted)]">{formatDuration(build.accepted_duration_ms)} / {formatDuration(build.minimum_duration_ms)}</span>
    </div>
    <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">{build.accepted_take_count} accepted recordings · clear, varied recordings help the result</p>
    <div
      class="mt-3 h-1.5 overflow-hidden rounded-full bg-[var(--ti-border)]"
      role="progressbar"
      aria-label="Ready recordings progress"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={speechProgress}
      aria-valuetext={`${formatDuration(build.accepted_duration_ms)} of ${formatDuration(build.minimum_duration_ms)} accepted speech`}
    >
      <div class="h-full rounded-full bg-[var(--ti-accent)]" style={`width:${speechProgress}%`}></div>
    </div>
  </div>

  {#if build.active}
    <div class="mt-5 border-l-2 border-[var(--ti-border-strong)] pl-4" role="status" aria-live="polite" aria-atomic="true">
      <strong class="text-sm font-semibold">{activeTitle()}</strong>
      <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{activeDetail()}</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary mt-5" disabled={busy} onclick={() => void cancelBuild()}>
      <Square size={14} /><span>{busy ? "Stopping..." : "Stop Creating"}</span>
    </button>
  {:else if build.evaluation_ready}
    <div class="mt-5">
      <strong class="text-sm font-semibold">Listen before using My Voice</strong>
      <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">Listen to these new sentences. Use My Voice only if it sounds like you.</p>
      <div class="mt-4 space-y-2">
        {#each build.evaluation_samples as sample}
          <div class="flex items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] px-4 py-3">
            <button type="button" class="ti-button ti-button-secondary shrink-0" disabled={playingLineId !== null} onclick={() => void playEvaluation(sample.line_id)}>
              <Play size={14} /><span>{playingLineId === sample.line_id ? "Playing..." : "Preview"}</span>
            </button>
            <span class="min-w-0 flex-1 text-sm leading-5 text-[var(--ti-text-muted)]">{sample.exact_text}</span>
            {#if reviewedLineIds.includes(sample.line_id)}
              <span class="flex shrink-0 items-center gap-1 text-xs font-semibold text-[var(--ti-success)]"><Check size={13} />Listened</span>
            {/if}
          </div>
        {/each}
      </div>
      <p class="mb-0 mt-3 text-xs text-[var(--ti-text-muted)]">{reviewedLineIds.length} of {build.evaluation_samples.length} previews listened</p>
      <button type="button" class="ti-button mt-5" disabled={busy || !evaluationReviewComplete} onclick={() => void approve()}>
        <Check size={15} /><span>{busy ? "Saving..." : "Use My Voice"}</span>
      </button>
    </div>
  {:else}
    {#if !build.can_build}
      <p class="mb-0 mt-5 text-sm leading-5 text-[var(--ti-text-muted)]">{idleGuidance()}</p>
    {:else}
      <label class="mt-5 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
        <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} />
        <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I confirm these recordings are my voice, or a voice I have permission to create.</span>
      </label>
      <button type="button" class="ti-button mt-4" disabled={busy || !authorized} onclick={() => void startBuild()}>
        <span>{busy ? "Starting..." : build.approved_voice_ready ? "Create Again" : "Create My Voice"}</span>
      </button>
    {/if}
  {/if}

  <p class="mb-0 mt-5 text-xs leading-5 text-[var(--ti-text-soft)]">My Voice is saved only after you listen to the previews and approve it.</p>
</section>
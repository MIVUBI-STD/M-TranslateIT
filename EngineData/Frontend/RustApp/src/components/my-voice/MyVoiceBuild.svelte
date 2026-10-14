<script lang="ts">
  import { Check, Play, Square } from "@lucide/svelte";
  import { onMount, untrack } from "svelte";
  import {
    myVoiceBuildApi,
    type MyVoiceBuildActionResult,
    type MyVoiceBuildStatus,
    type MyVoiceComputeStatus,
  } from "../../app/bridge/myVoiceBuildApi";
  import QuickVoicePreview from "./QuickVoicePreview.svelte";

  let {
    onNotice,
    onMeetingVoiceChanged,
    onBuildStatus,
    onBuildStatusUnavailable,
    onTrainingComputeEligibility,
    refreshRevision = 0,
    voiceChangesLocked = false,
  }: {
    onNotice: (message: string) => void;
    onMeetingVoiceChanged: (message?: string) => void | Promise<void>;
    onBuildStatus?: (status: MyVoiceBuildStatus) => void;
    onBuildStatusUnavailable?: () => void;
    onTrainingComputeEligibility?: (allowed: boolean) => void;
    refreshRevision?: number;
    voiceChangesLocked?: boolean;
  } = $props();

  let build = $state<MyVoiceBuildStatus>({
    active: false,
    preview_active: false,
    generation: null,
    phase: "checking",
    message: "Checking My Voice recordings...",
    accepted_take_count: 0,
    accepted_duration_ms: 0,
    minimum_duration_ms: 60_000,
    missing_coverage: null,
    recording_suggestions: [],
    can_build: false,
    evaluation_ready: false,
    evaluation_review_id: null,
    evaluation_samples: [],
    approved_voice_ready: false,
    selected_voice_kind: "unknown",
    selected_builtin_voice_id: null,
    saved_personal_voice_available: false,
  });
  let authorized = $state(false);
  let previewAuthorized = $state(false);
  let quickPreviewOptionsOpen = $state(false);
  let quickPreviewBusy = $state(false);
  let quickPreviewReady = $state(false);
  let quickPreviewPlaying = $state(false);
  let quickPreviewLoading = $state(false);
  let busy = $state(false);
  let playingLineId = $state<number | null>(null);
  let evaluationLoadingLineId = $state<number | null>(null);
  let seenRefreshRevision = untrack(() => refreshRevision);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let disposed = false;
  let statusCheckUnavailable = $state(false);
  let computeStatus = $state<MyVoiceComputeStatus | null>(null);
  let computeChecking = $state(false);
  let computeReadVersion = 0;
  const computeCanStart = $derived(
    !computeChecking &&
    computeStatus?.state === "cuda_detected" &&
    computeStatus.admission_state === "admitted",
  );

  // One hardware eligibility view, reused by Create and archive-backed Improve.
  $effect(() => {
    onTrainingComputeEligibility?.(computeCanStart);
  });
  // Read-order fence for polling, explicit refresh, and mutation results.
  // It is local to this view; Rust remains the only owner of build truth.
  let statusReadVersion = 0;
  let audio: HTMLAudioElement | null = null;
  let audioUrl: string | null = null;
  // Invalidates delayed audio reads and event callbacks on cancellation,
  // review changes, switching playback or component disposal.
  let audioRequestVersion = 0;
  let reviewedLineIds = $state<number[]>([]);
  let qualityConfirmed = $state(false);

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
    audioRequestVersion += 1;
    const previous = audio;
    audio = null;
    if (previous) {
      previous.onended = null;
      previous.onerror = null;
      previous.pause();
    }
    playingLineId = null;
    evaluationLoadingLineId = null;
    quickPreviewPlaying = false;
    quickPreviewLoading = false;
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    audioUrl = null;
  }

  function schedulePoll(): void {
    if (timer) clearTimeout(timer);
    timer = null;
    if (disposed || !build.active) return;
    timer = setTimeout(() => void refresh(), 1400);
  }

  function invalidatePendingStatusReads(): void {
    statusReadVersion += 1;
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function applyStatus(next: MyVoiceBuildStatus): void {
    if (disposed) return;
    if (build.evaluation_review_id !== next.evaluation_review_id) {
      reviewedLineIds = [];
      qualityConfirmed = false;
      stopAudio();
    }
    build = next;
    onBuildStatus?.(next);
    if (!next.evaluation_ready) {
      reviewedLineIds = [];
      qualityConfirmed = false;
    }
    // Another training lifecycle supersedes an earlier temporary preview.
    if (next.active && !next.preview_active) {
      quickPreviewReady = false;
      stopAudio();
    }
    schedulePoll();
  }

  function productMessage(result: MyVoiceBuildActionResult): string {
    switch (result.state) {
      case "building":
        return "Creating My Voice. You can leave My Voice open while it works.";
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
      case "gpu_admission_blocked":
        return result.message;
      case "resource_admission_blocked":
        if (result.message.includes("insufficient_system_ram"))
          return "Not enough free system memory to create My Voice. Close unnecessary apps and recheck hardware.";
        if (result.message.includes("insufficient_temporary_disk_space"))
          return "Temporary training storage has insufficient free space. Free space without deleting saved My Voice recordings.";
        if (result.message.includes("insufficient_saved_disk_space"))
          return "Saved My Voice storage has insufficient free space to protect your models and recordings.";
        return "RAM or storage could not be verified. Training has not started.";
      case "build_process_guard_failed":
        return "Windows could not securely contain the training process. Training was blocked without using an unprotected fallback.";
      case "build_identity_failed":
        return "Couldn't safely register this training attempt. The existing voice was not replaced.";
      case "meeting_active":
        return "Stop Meeting translation before selecting My Voice.";
      case "cancel_pending":
        return result.message || "Stop requested. Training is waiting for a safe boundary.";
      case "evaluation_required":
      case "evaluation_listening_required":
        return "Listen to every voice preview before approving My Voice.";
      case "evaluation_quality_confirmation_required":
        return "Confirm that the trained voice sounds clear, natural, and like the authorized speaker before using it.";
      case "evaluation_candidate_mismatch":
        return "The review no longer matches the voice candidate. Create My Voice again.";
      case "approved":
        return "My Voice is ready and selected for meetings.";
      case "approved_cleanup_attention":
        return "My Voice is ready. Some temporary build files could not be cleared yet.";
      case "approved_archive_attention":
        return result.message || "My Voice is selected, but its separate saved copy could not be verified. Switching voices remains unavailable until it can be preserved.";
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
    if (build.active && build.phase === "training" && build.message.includes("SoVITS training stage")) return "Training voice characteristics";
    if (build.active && build.phase === "training" && build.message.includes("GPT training stage")) return "Training speech generation";
    if (build.phase === "evaluating") return "Preparing voice previews";
    if (build.phase === "cancelling") return "Stopping My Voice creation";
    return "Creating My Voice";
  }

  function activeDetail(): string {
    // The local Python worker emits this actual phase message before each stage.
    // Never estimate epochs, percent complete, or remaining training time.
    if (build.active && build.phase !== "cancelling" && build.message.trim()) return build.message.replace(/\bVoiceLab\b/g, "My Voice");
    if (build.phase === "preparing") return "Getting your accepted recordings ready.";
    if (build.phase === "training") return "Creating your English meeting voice from your accepted recordings.";
    if (build.phase === "evaluating") return "Preparing new sentences so you can listen before approving My Voice.";
    if (build.phase === "cancelling") return "Finishing the current stop safely.";
    return "My Voice is being created.";
  }

  function coverageGuidance(): string | null {
    const coverage = build.missing_coverage;
    if (!coverage || build.accepted_duration_ms < build.minimum_duration_ms) return null;
    return `Record another sentence for ${coverage.label}.`;
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

  // A mutation may reach Rust even when its reply is lost. Freeze controls
  // until a new authoritative build snapshot replaces stale local state.
  async function reconcileUncertainBuildAction(message: string): Promise<void> {
    if (disposed) return;
    invalidatePendingStatusReads();
    statusCheckUnavailable = true;
    onBuildStatusUnavailable?.();
    onNotice(message);
    await refresh();
  }

  async function applyResult(result: MyVoiceBuildActionResult): Promise<void> {
    if (disposed) return;
    // A confirmed mutation snapshot supersedes every older polling read.
    invalidatePendingStatusReads();
    if (result.build.phase === "unavailable") {
      await reconcileUncertainBuildAction(
        "Couldn't verify this My Voice action. Checking the current build status.",
      );
      return;
    }
    // A verified native response supersedes a previous bridge failure.
    statusCheckUnavailable = false;
    applyStatus(result.build);
    onNotice(productMessage(result));
  }

  async function refreshMeetingAfterUncertainApproval(): Promise<void> {
    if (disposed) return;
    // This refresh is read-only. It must not claim the candidate was approved
    // or silently repeat a voice selection request.
    try {
      await onMeetingVoiceChanged(
        "My Voice approval could not be confirmed. Check the selected Meeting voice.",
      );
    } catch {
      if (!disposed) onNotice("Couldn't verify the selected Meeting voice. Check its status before using it.");
    }
  }

  async function refresh(): Promise<void> {
    const requestVersion = ++statusReadVersion;
    try {
      const next = await myVoiceBuildApi.getStatus();
      if (disposed || requestVersion !== statusReadVersion) return;
      if (next.phase === "unavailable") {
        statusCheckUnavailable = true;
        onBuildStatusUnavailable?.();
        schedulePoll();
        return;
      }
      statusCheckUnavailable = false;
      applyStatus(next);
    } catch {
      if (disposed || requestVersion !== statusReadVersion) return;
      if (!statusCheckUnavailable) onNotice("Couldn't check My Voice progress. Select Retry status.");
      statusCheckUnavailable = true;
      onBuildStatusUnavailable?.();
      schedulePoll();
    }
  }

  async function generateQuickPreview(): Promise<void> {
    if (busy || statusCheckUnavailable || quickPreviewBusy || build.active || !previewAuthorized || build.accepted_take_count === 0) return;
    invalidatePendingStatusReads();
    stopAudio();
    quickPreviewReady = false;
    quickPreviewBusy = true;
    try {
      const result = await myVoiceBuildApi.generateQuickPreview();
      quickPreviewReady = result.ok && result.state === "preview_ready";
      onNotice(result.message);
    } catch {
      onNotice("Quick Preview could not be generated.");
    } finally {
      quickPreviewBusy = false;
      await refresh();
    }
  }

  async function cancelQuickPreview(): Promise<void> {
    if (!quickPreviewBusy && !build.preview_active) return;
    invalidatePendingStatusReads();
    try {
      const result = await myVoiceBuildApi.cancelQuickPreview();
      if (!disposed) onNotice(result.message);
    } catch {
      if (!disposed) onNotice("Couldn't stop Quick Preview. Check its status and try again.");
    } finally {
      await refresh();
    }
  }

  async function playQuickPreview(): Promise<void> {
    if (busy || statusCheckUnavailable || quickPreviewBusy || quickPreviewLoading || !quickPreviewReady || audio !== null) return;
    const requestVersion = ++audioRequestVersion;
    quickPreviewLoading = true;
    let bytes: ArrayBuffer | null;
    try {
      bytes = await myVoiceBuildApi.getQuickPreviewAudio();
    } catch {
      if (!disposed && requestVersion === audioRequestVersion) onNotice("Quick Preview audio couldn't be loaded.");
      return;
    } finally {
      if (!disposed && requestVersion === audioRequestVersion) quickPreviewLoading = false;
    }
    if (disposed || requestVersion !== audioRequestVersion || !quickPreviewReady) return;
    if (!bytes) {
      quickPreviewReady = false;
      onNotice("Quick Preview audio is unavailable.");
      return;
    }
    stopAudio();
    const playbackVersion = audioRequestVersion;
    audioUrl = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
    const player = new Audio(audioUrl);
    audio = player;
    quickPreviewPlaying = true;
    player.onended = () => {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion) return;
      stopAudio();
    };
    player.onerror = () => {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion) return;
      stopAudio();
      onNotice("Quick Preview could not be played.");
    };
    try {
      await player.play();
    } catch {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion) return;
      stopAudio();
      onNotice("Quick Preview could not be played.");
    }
  }

  async function checkCompute(): Promise<void> {
    const requestVersion = ++computeReadVersion;
    computeChecking = true;
    computeStatus = null;
    try {
      const next = await myVoiceBuildApi.getComputeStatus();
      if (disposed || requestVersion !== computeReadVersion) return;
      computeStatus = next ?? {
        state: "unavailable",
        cpu_name: "CPU name unavailable",
        gpu_name: null,
        gpu_uuid: null,
        compute_capability: null,
        precision_profile: null,
        cuda_index: null,
        visible_cuda_devices: 0,
        vram_mib: null,
        free_vram_mib: null,
        reserve_mib: null,
        safe_budget_mib: null,
        admission_state: "unknown",
        profile: "SAFE_CUDA_V1",
        detail: "Could not verify local compute hardware.",
      };
    } catch {
      if (disposed || requestVersion !== computeReadVersion) return;
      computeStatus = {
        state: "unavailable",
        cpu_name: "CPU name unavailable",
        gpu_name: null,
        gpu_uuid: null,
        compute_capability: null,
        precision_profile: null,
        cuda_index: null,
        visible_cuda_devices: 0,
        vram_mib: null,
        free_vram_mib: null,
        reserve_mib: null,
        safe_budget_mib: null,
        admission_state: "unknown",
        profile: "SAFE_CUDA_V1",
        detail: "Hardware check failed. Retry before training.",
      };
    } finally {
      if (!disposed && requestVersion === computeReadVersion) computeChecking = false;
    }
  }

  async function startBuild(): Promise<void> {
    if (busy || statusCheckUnavailable || voiceChangesLocked || quickPreviewBusy || !build.can_build || !authorized || !computeCanStart) return;
    invalidatePendingStatusReads();
    quickPreviewReady = false;
    stopAudio();
    reviewedLineIds = [];
    busy = true;
    try {
      await applyResult(await myVoiceBuildApi.start(authorized));
    } catch {
      await reconcileUncertainBuildAction(
        "Couldn't confirm whether My Voice creation started. Checking status.",
      );
    } finally {
      busy = false;
    }
  }

  async function cancelBuild(): Promise<void> {
    if (busy || statusCheckUnavailable || quickPreviewBusy || !build.active) return;
    invalidatePendingStatusReads();
    busy = true;
    try {
      await applyResult(await myVoiceBuildApi.cancel());
    } catch {
      await reconcileUncertainBuildAction(
        "Couldn't confirm whether My Voice creation stopped. Checking status.",
      );
    } finally {
      busy = false;
    }
  }

  async function approve(): Promise<void> {
    if (busy || statusCheckUnavailable || voiceChangesLocked || build.active || !build.evaluation_ready || !build.evaluation_review_id || !evaluationReviewComplete || !qualityConfirmed) return;
    invalidatePendingStatusReads();
    stopAudio();
    busy = true;
    try {
      const result = await myVoiceBuildApi.approve(reviewedLineIds, qualityConfirmed, build.evaluation_review_id);
      await applyResult(result);
      if (result.ok && ["approved", "approved_cleanup_attention", "approved_archive_attention"].includes(result.state)) {
        await onMeetingVoiceChanged(productMessage(result));
      } else if (result.build.phase === "unavailable") {
        await refreshMeetingAfterUncertainApproval();
      }
    } catch {
      await reconcileUncertainBuildAction(
        "Couldn't confirm whether My Voice was approved. Checking status.",
      );
      await refreshMeetingAfterUncertainApproval();
    } finally {
      busy = false;
    }
  }

  async function playEvaluation(lineId: number): Promise<void> {
    if (busy || statusCheckUnavailable || evaluationLoadingLineId !== null || audio !== null) return;
    const reviewId = build.evaluation_review_id;
    const expectedSha256 = build.evaluation_samples.find((sample) => sample.line_id === lineId)?.sha256;
    if (!reviewId || !expectedSha256) return;
    const requestVersion = ++audioRequestVersion;
    evaluationLoadingLineId = lineId;
    let bytes: ArrayBuffer | null;
    try {
      bytes = await myVoiceBuildApi.getEvaluationAudio(lineId, reviewId, expectedSha256);
    } catch {
      if (!disposed && requestVersion === audioRequestVersion && reviewId === build.evaluation_review_id)
        onNotice("This My Voice sample couldn't be loaded. Try again.");
      return;
    } finally {
      if (!disposed && requestVersion === audioRequestVersion) evaluationLoadingLineId = null;
    }
    if (disposed || reviewId !== build.evaluation_review_id || requestVersion !== audioRequestVersion) return;
    if (!bytes) {
      onNotice("This My Voice preview is unavailable or failed its integrity check.");
      return;
    }
    stopAudio();
    const playbackVersion = audioRequestVersion;
    audioUrl = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
    const player = new Audio(audioUrl);
    audio = player;
    playingLineId = lineId;
    player.onended = () => {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion || reviewId !== build.evaluation_review_id) return;
      if (!reviewedLineIds.includes(lineId)) reviewedLineIds = [...reviewedLineIds, lineId];
      stopAudio();
    };
    player.onerror = () => {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion) return;
      stopAudio();
      onNotice("This My Voice preview couldn't be played.");
    };
    try {
      await player.play();
    } catch {
      if (disposed || audio !== player || playbackVersion !== audioRequestVersion) return;
      stopAudio();
      onNotice("This My Voice preview couldn't be played.");
    }
  }

  $effect(() => {
    const revision = refreshRevision;
    // Parent advances this revision after accepting a new guided recording.
    // Never present the old reference's WAV as a preview of the new takes.
    if (revision !== seenRefreshRevision) {
      seenRefreshRevision = revision;
      quickPreviewReady = false;
      reviewedLineIds = [];
      qualityConfirmed = false;
      stopAudio();
    }
    void refresh();
  });

  onMount(() => {
    disposed = false;
    void checkCompute();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
      timer = null;
      statusReadVersion += 1;
      computeReadVersion += 1;
      stopAudio();
    };
  });
</script>

<section class="ti-panel p-6" aria-busy={build.active || busy || quickPreviewBusy}>
  {#if statusCheckUnavailable}
    <div role="alert" class="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] p-3">
      <span class="text-sm">My Voice status is unavailable. The previous progress may be outdated.</span>
      <button type="button" class="ti-button ti-button-secondary" disabled={busy || quickPreviewBusy} onclick={() => void refresh()}>Retry status</button>
    </div>
  {/if}
  <div class="flex items-start justify-between gap-5">
    <div>
      <h4 class="mb-0 text-lg font-semibold">{build.evaluation_ready ? "Review your voice" : build.active ? activeTitle() : "Create your voice"}</h4>
      <p class="mb-0 mt-2 max-w-[680px] text-sm leading-6 text-[var(--ti-text-muted)]">{build.evaluation_ready ? "Listen to the new voice before selecting it for meetings." : "Use your accepted recordings to create a personal English voice."}</p>
    </div>
    {#if build.selected_voice_kind === "personal"}
      <span class="flex items-center gap-1.5 text-sm font-semibold text-[var(--ti-success)]"><Check size={15} />Personal voice selected</span>
    {:else if build.saved_personal_voice_available}
      <span class="flex items-center gap-1.5 text-sm font-semibold text-[var(--ti-success)]"><Check size={15} />Personal voice saved</span>
    {:else if build.approved_voice_ready}
      <span class="flex items-center gap-1.5 text-sm font-semibold text-[var(--ti-success)]"><Check size={15} />Meeting voice ready</span>
    {/if}
  </div>

  <div class="mt-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] px-4 py-3" aria-label="My Voice training hardware">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <strong class="text-sm font-semibold">Training hardware</strong>
      <button type="button" class="ti-button ti-button-secondary" disabled={computeChecking || build.active || quickPreviewBusy} onclick={() => void checkCompute()}>
        {computeChecking ? "Checking..." : "Recheck hardware"}
      </button>
    </div>
    <p class="mb-0 mt-2 text-sm font-semibold" role="status">
      {computeChecking ? "Checking GPU identity and memory..." : computeStatus?.admission_state === "admitted"
        ? "Device-aware CUDA safety — preliminary memory check passed" : computeStatus?.state === "cpu_only"
        ? "CPU-only training blocked — not verified" : computeStatus?.admission_state === "blocked_unsupported_precision"
        ? "GPU detected — this trainer needs an unsupported precision profile" : computeStatus?.admission_state === "blocked_gpu_headroom"
        ? "Insufficient free GPU memory" : "Training hardware unavailable"}
    </p>
    {#if computeStatus}
      <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">CPU: {computeStatus.cpu_name}</p>
      <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">
        GPU: {computeStatus.gpu_name ?? "No usable CUDA device reported"}{computeStatus.vram_mib !== null ? ` · Total VRAM: ${Math.round(computeStatus.vram_mib / 1024 * 10) / 10} GiB` : ""}{computeStatus.free_vram_mib !== null ? ` · Currently free: ${Math.round(computeStatus.free_vram_mib / 1024 * 10) / 10} GiB` : ""}
      </p>
      {#if computeStatus.gpu_uuid}
        <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">Verified GPU ID: {computeStatus.gpu_uuid} · Visible CUDA devices: {computeStatus.visible_cuda_devices} · Selected CUDA index: {computeStatus.cuda_index}{computeStatus.compute_capability ? ` · Compute capability: ${computeStatus.compute_capability}` : ""}{computeStatus.precision_profile === "mixed_fp16" ? " · Trainer precision: mixed FP16" : ""}</p>
      {/if}
      {#if computeStatus.safe_budget_mib !== null}
        <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">Initial per-stage budget: {Math.round(computeStatus.safe_budget_mib / 1024 * 10) / 10} GiB · Headroom reserve: {Math.round((computeStatus.reserve_mib ?? 0) / 1024 * 10) / 10} GiB · Not an absolute GPU cap.</p>
      {/if}
      <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">{computeStatus.detail}</p>
    {:else if !computeChecking}
      <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">Recheck to identify your local compute environment before creating a voice.</p>
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

  {#if quickPreviewBusy || build.preview_active}
    <div class="mt-5" role="status">
      <strong class="text-sm font-semibold">Creating Quick Preview</strong>
      <p class="mb-0 mt-1 text-sm text-[var(--ti-text-muted)]">Generating a temporary local listening sample.</p>
      <button type="button" class="ti-button ti-button-secondary mt-3" onclick={() => void cancelQuickPreview()}>
        <Square size={14} /><span>Stop Preview</span>
      </button>
    </div>
  {:else if build.active}
    <div class="mt-5 border-l-2 border-[var(--ti-border-strong)] pl-4" role="status" aria-live="polite" aria-atomic="true">
      <strong class="text-sm font-semibold">{activeTitle()}</strong>
      <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{activeDetail()}</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary mt-5" disabled={busy || statusCheckUnavailable} onclick={() => void cancelBuild()}>
      <Square size={14} /><span>{busy ? "Stopping..." : "Stop Creating"}</span>
    </button>
  {:else if build.evaluation_ready}
    <div class="mt-5">
      <strong class="text-sm font-semibold">Listen before using My Voice</strong>
      <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">Listen to these new sentences. Use My Voice only if it sounds like you.</p>
      <div class="mt-4 space-y-2">
        {#each build.evaluation_samples as sample}
          <div class="flex items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] px-4 py-3">
            <button type="button" class="ti-button ti-button-secondary shrink-0" aria-busy={evaluationLoadingLineId === sample.line_id} disabled={statusCheckUnavailable || playingLineId !== null || evaluationLoadingLineId !== null} onclick={() => void playEvaluation(sample.line_id)}>
              <Play size={14} /><span aria-live="polite">{evaluationLoadingLineId === sample.line_id ? "Loading audio..." : playingLineId === sample.line_id ? "Playing..." : "Preview"}</span>
            </button>
            <span class="min-w-0 flex-1 text-sm leading-5 text-[var(--ti-text-muted)]">{sample.exact_text}</span>
            {#if reviewedLineIds.includes(sample.line_id)}
              <span class="flex shrink-0 items-center gap-1 text-xs font-semibold text-[var(--ti-success)]"><Check size={13} />Listened</span>
            {/if}
          </div>
        {/each}
      </div>
      <p class="mb-0 mt-3 text-xs text-[var(--ti-text-muted)]">{reviewedLineIds.length} of {build.evaluation_samples.length} previews listened</p>
      <label class="mt-4 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
        <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={qualityConfirmed} disabled={!evaluationReviewComplete || busy || statusCheckUnavailable} />
        <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I listened to every preview. The voice is clear, sounds natural, resembles the authorized speaker, and I want to use it for meetings.</span>
      </label>
      {#if voiceChangesLocked}
        <p class="mb-0 mt-3 text-sm text-[var(--ti-text-muted)]" role="status">Finish the current Meeting or recording before selecting a new voice.</p>
      {/if}
      <button type="button" class="ti-button mt-4" disabled={busy || statusCheckUnavailable || voiceChangesLocked || !evaluationReviewComplete || !qualityConfirmed} onclick={() => void approve()}>
        <Check size={15} /><span>{busy ? "Saving..." : "Use My Voice"}</span>
      </button>
      <p class="mb-0 mt-4 text-xs leading-5 text-[var(--ti-text-muted)]">Not satisfied? Keep the current Meeting voice and record more samples before creating another candidate.</p>
      {#if build.can_build}
        <label class="mt-3 flex items-start gap-2 text-xs text-[var(--ti-text-muted)]">
          <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} disabled={busy} />
          I own this voice or have permission to create another version.
        </label>
        <button type="button" class="ti-button ti-button-secondary mt-3" disabled={busy || statusCheckUnavailable || voiceChangesLocked || !authorized || !computeCanStart} onclick={() => void startBuild()}>Create Another Candidate</button>
      {/if}
    </div>
  {:else if build.approved_voice_ready && build.selected_voice_kind === "personal"}
    <p class="mb-0 mt-5 text-sm text-[var(--ti-text-muted)]">My Voice is selected for meetings. Your recordings remain available if you want to create another candidate.</p>
    {#if build.can_build}
      <label class="mt-4 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
        <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} />
        <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I confirm these recordings are my voice, or a voice I have permission to create.</span>
      </label>
      <button type="button" class="ti-button ti-button-secondary mt-4" disabled={busy || statusCheckUnavailable || voiceChangesLocked || !authorized || !computeCanStart} onclick={() => void startBuild()}>Create Another Candidate</button>
    {/if}
  {:else}
    {#if !build.can_build}
      <p class="mb-0 mt-5 text-sm leading-5 text-[var(--ti-text-muted)]">{idleGuidance()}</p>
    {:else}
      <label class="mt-5 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
        <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} />
        <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I confirm these recordings are my voice, or a voice I have permission to create.</span>
      </label>
      <button type="button" class="ti-button mt-4" disabled={busy || statusCheckUnavailable || voiceChangesLocked || !authorized || !computeCanStart} onclick={() => void startBuild()}>
        <span>{busy ? "Starting..." : "Create My Voice"}</span>
      </button>
    {/if}
  {/if}

  <QuickVoicePreview
    {build}
    busy={busy || statusCheckUnavailable}
    {quickPreviewBusy}
    {quickPreviewReady}
    {quickPreviewPlaying}
    {quickPreviewLoading}
    bind:previewAuthorized
    bind:quickPreviewOptionsOpen
    onGenerate={generateQuickPreview}
    onPlay={playQuickPreview}
  />
  <p class="mb-0 mt-5 text-xs leading-5 text-[var(--ti-text-soft)]">Only a trained, evaluated and explicitly approved My Voice can be selected for Meeting. Quick Preview is not an output voice.</p>
</section>

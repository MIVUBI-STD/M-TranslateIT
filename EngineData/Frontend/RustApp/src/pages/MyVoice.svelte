<script lang="ts">
  import { Check, Circle, Mic, Play, RotateCcw, SkipForward, Square } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    myVoiceApi,
    type GuidedRecordingActionResult,
    type GuidedRecordingState,
  } from "../app/bridge/myVoiceApi";
  import { myVoiceBuildApi } from "../app/bridge/myVoiceBuildApi";
  import MyVoiceBuild from "../components/my-voice/MyVoiceBuild.svelte";
  import BuiltinVoicePicker from "../components/my-voice/BuiltinVoicePicker.svelte";

  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";

  let {
    onNotice,
    onRecordingChange,
    onMeetingVoiceChanged,
    voiceChangesLocked = false,
  }: {
    onNotice: (message: string) => void;
    onRecordingChange: (recording: boolean) => void;
    onMeetingVoiceChanged: (message?: string) => void | Promise<void>;
    voiceChangesLocked?: boolean;
  } = $props();

  let recordingState = $state<GuidedRecordingState>({ recording_line_id: null, pending_review: null, lines: [] });
  let selectedLineId = $state<number | null>(null);
  let authorized = $state(false);
  let busy = $state(false);
  let replaying = $state(false);
  let buildRefreshRevision = $state(0);
  let audio: HTMLAudioElement | null = null;
  let audioUrl: string | null = null;
  let builtinPendingId = $state<BuiltinVoiceId | null>(null);
  let builtinPreviewing = $state<BuiltinVoiceId | null>(null);
  let disposed = false;
  let selectedVoiceKind = $state("unknown");
  let selectedBuiltinVoiceId = $state<string | null>(null);
  let savedPersonalVoiceAvailable = $state(false);
  let selectedVoiceVerified = $state(false);

  async function refreshVoiceSelection(): Promise<void> {
    try {
      const status = await myVoiceBuildApi.getStatus();
      if (disposed) return;
      selectedVoiceVerified = status.phase !== "unavailable";
      if (!selectedVoiceVerified) return;
      selectedVoiceKind = status.selected_voice_kind;
      selectedBuiltinVoiceId = status.selected_builtin_voice_id;
      savedPersonalVoiceAvailable = status.saved_personal_voice_available;
    } catch {
      if (!disposed) selectedVoiceVerified = false;
    }
  }

  const currentLine = $derived(recordingState.lines.find((line) => line.line_id === selectedLineId) ?? null);
  const isRecording = $derived(recordingState.recording_line_id !== null);
  const pendingLineId = $derived(recordingState.pending_review?.line_id ?? null);

  function builtinLabel(voiceId: BuiltinVoiceId): string {
    return voiceId === "MaleVoice" ? "Built-in Male" : "Built-in Female";
  }

  async function previewBuiltin(voiceId: BuiltinVoiceId): Promise<void> {
    if (busy || replaying || builtinPreviewing) return;
    stopReplay();
    builtinPreviewing = voiceId;
    try {
      const bytes = await myVoiceBuildApi.getBuiltinPreviewAudio(voiceId);
      if (disposed) return;
      if (!bytes) {
        builtinPreviewing = null;
        onNotice("This built-in voice preview is unavailable.");
        return;
      }
      const blob = new Blob([bytes], { type: "audio/wav" });
      audioUrl = URL.createObjectURL(blob);
      audio = new Audio(audioUrl);
      replaying = true;
      audio.onended = () => {
        stopReplay();
        builtinPreviewing = null;
      };
      audio.onerror = () => {
        stopReplay();
        builtinPreviewing = null;
        onNotice("This built-in voice preview couldn't be played.");
      };
      await audio.play();
      onNotice(`Previewing ${builtinLabel(voiceId)}.`);
    } catch {
      stopReplay();
      builtinPreviewing = null;
      onNotice("This built-in voice preview couldn't be played.");
    }
  }

  async function selectBuiltin(voiceId: BuiltinVoiceId, confirmed = false): Promise<void> {
    if (busy || voiceChangesLocked || isRecording || pendingLineId !== null) return;
    busy = true;
    try {
      const result = await myVoiceBuildApi.selectBuiltin(voiceId, confirmed);
      if (disposed) return;
      if (result.state === "approval_required") {
        builtinPendingId = voiceId;
        onNotice(`Confirm before replacing the current Meeting voice with ${builtinLabel(voiceId)}.`);
        return;
      }
      builtinPendingId = null;
      if (result.ok) {
        buildRefreshRevision++;
        await refreshVoiceSelection();
        await onMeetingVoiceChanged(result.message);
      } else {
        onNotice(result.message);
      }
    } catch {
      if (!disposed) onNotice("Couldn't change the Meeting voice. Check My Voice status and try again.");
    } finally {
      busy = false;
    }
  }

  async function confirmPendingBuiltin(): Promise<void> {
    const voiceId = builtinPendingId;
    if (!voiceId) return;
    await selectBuiltin(voiceId, true);
  }

  async function selectStoredPersonalVoice(): Promise<void> {
    if (busy || voiceChangesLocked || isRecording || pendingLineId !== null) return;
    busy = true;
    try {
      const result = await myVoiceBuildApi.selectSavedPersonalVoice();
      if (disposed) return;
      if (result.ok) {
        buildRefreshRevision++;
        await refreshVoiceSelection();
        await onMeetingVoiceChanged(result.message);
      } else {
        onNotice(result.message);
      }
    } catch {
      if (!disposed) onNotice("Could not switch to your saved My Voice. Try again.");
    } finally {
      busy = false;
    }
  }

  function chooseDefaultLine(next: GuidedRecordingState): void {
    if (next.recording_line_id !== null) {
      selectedLineId = next.recording_line_id;
      return;
    }
    if (next.pending_review) {
      selectedLineId = next.pending_review.line_id;
      return;
    }
    if (selectedLineId !== null && next.lines.some((line) => line.line_id === selectedLineId)) return;
    selectedLineId = next.lines.find((line) => !line.accepted)?.line_id ?? next.lines[0]?.line_id ?? null;
  }

  function applyState(next: GuidedRecordingState): void {
    if (disposed) return;
    recordingState = next;
    chooseDefaultLine(next);
    onRecordingChange(next.recording_line_id !== null);
  }

  function recordingQualityGuidance(blocker: string): string {
    switch (blocker) {
      case "voice_lab:take_signal_unusable:rejected_silence":
        return "No clear speech was captured. Check your selected microphone, then record this line again.";
      case "voice_lab:take_signal_unusable:severe_clipping":
        return "The recording is distorted or too loud. Lower the microphone level or move farther away, then retry.";
      case "voice_lab:take_signal_unusable:signal_too_low":
        return "The recorded voice is too quiet. Check the microphone level or move closer, then retry.";
      case "voice_lab:take_signal_unusable:dc_offset_too_high":
        return "The microphone signal is unsuitable. Check the microphone connection or select another input, then retry.";
      default:
        return "This recording failed its audio check. Replay it if useful, then record the line again.";
    }
  }

  function productMessage(result: GuidedRecordingActionResult): string {
    switch (result.state) {
      case "recording":
        return "Recording started. Read the line naturally, then press Stop.";
      case "needs_review":
        return "Recording stopped. Replay it, then accept it or try the line again.";
      case "accepted":
        return "Recording accepted.";
      case "ready":
        return "Ready for another recording.";
      case "authorization_required":
        return "Confirm that this is your voice, or that you have permission to use it.";
      case "build_active":
        return "Finish or stop My Voice creation before recording more lines.";
      case "review_pending":
        return "Review the current recording before starting another line.";
      case "microphone_in_use":
      case "owner_conflict":
        return "Another TranslateIT action is using the microphone. Finish it, then try again.";
      case "runtime_unavailable":
        return "My Voice can't check the microphone right now. Try again.";
      case "capture_unavailable":
      case "capture_failed":
        return "My Voice couldn't use the microphone. Check the microphone and try again.";
      case "stop_failed":
      case "cleanup_unverified":
        return "My Voice couldn't stop the microphone cleanly. Try again.";
      case "take_unusable":
        return "This recording isn't usable yet. Record the line again.";
      case "draft_write_failed":
      case "draft_state_unavailable":
      case "save_failed":
        return "My Voice couldn't save this recording. Try again.";
      case "invalid_line":
      case "line_mismatch":
      case "no_review":
        return "This recording action is no longer current. Choose the line again and try again.";
      case "frontend_bridge_error":
        return "My Voice is unavailable right now. Try again.";
      default:
        return result.ok ? "My Voice action completed." : "My Voice couldn't complete this action. Try again.";
    }
  }

  function applyResult(result: GuidedRecordingActionResult): void {
    if (disposed) return;
    applyState(result.recording);
    onNotice(productMessage(result));
  }

  async function refresh(): Promise<void> {
    try {
      const next = await myVoiceApi.getState();
      if (!disposed) applyState(next);
    } catch {
      if (!disposed) onNotice("Couldn't check your recordings. Open My Voice again or try another action.");
    }
  }

  async function startRecording(): Promise<void> {
    if (busy || selectedLineId === null) return;
    if (!authorized) {
      onNotice("Confirm that this is your voice, or that you have permission to use it.");
      return;
    }
    busy = true;
    try {
      applyResult(await myVoiceApi.startTake(selectedLineId, authorized));
    } catch {
      if (!disposed) {
        onNotice("Couldn't confirm whether recording started. Check the recording status.");
        await refresh();
      }
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    if (busy || recordingState.recording_line_id === null) return;
    busy = true;
    try {
      applyResult(await myVoiceApi.stopTake(recordingState.recording_line_id));
    } catch {
      if (!disposed) {
        onNotice("Couldn't confirm whether recording stopped. Check the recording status.");
        await refresh();
      }
    } finally {
      busy = false;
    }
  }

  async function retryTake(): Promise<void> {
    if (busy || pendingLineId === null) return;
    stopReplay();
    busy = true;
    try {
      applyResult(await myVoiceApi.retryTake(pendingLineId));
    } catch {
      if (!disposed) onNotice("Couldn't retry this recording. Try again.");
    } finally {
      busy = false;
    }
  }

  async function acceptTake(): Promise<void> {
    if (busy || pendingLineId === null || recordingState.pending_review?.quality_blocker) return;
    stopReplay();
    busy = true;
    try {
      const result = await myVoiceApi.acceptTake(pendingLineId);
      applyResult(result);
      if (result.ok) {
        buildRefreshRevision += 1;
        selectedLineId = result.recording.lines.find((line) => !line.accepted)?.line_id ?? pendingLineId;
      }
    } catch {
      if (!disposed) onNotice("Couldn't confirm that this recording was saved. Check your recordings before trying again.");
    } finally {
      busy = false;
    }
  }

  function skipLine(): void {
    if (busy || replaying || isRecording || recordingState.pending_review !== null || selectedLineId === null) return;

    const currentIndex = recordingState.lines.findIndex((line) => line.line_id === selectedLineId);
    if (currentIndex < 0) return;

    const laterLines = recordingState.lines.slice(currentIndex + 1);
    const nextLine = laterLines.find((line) => !line.accepted) ?? laterLines[0] ?? null;
    if (!nextLine) {
      onNotice("This is the last recording line. Choose another line from the list if you want to continue.");
      return;
    }

    selectedLineId = nextLine.line_id;
    onNotice("Skipped this line. You can come back to it anytime.");
  }

  function stopReplay(): void {
    audio?.pause();
    audio = null;
    replaying = false;
    builtinPreviewing = null;
    if (audioUrl) URL.revokeObjectURL(audioUrl);
    audioUrl = null;
  }

  async function replayTake(): Promise<void> {
    if (busy || replaying || selectedLineId === null) return;
    let bytes: ArrayBuffer | null;
    try {
      bytes = await myVoiceApi.getTakeAudio(selectedLineId);
    } catch {
      if (!disposed) onNotice("This recording couldn't be loaded.");
      return;
    }
    if (disposed) return;
    if (!bytes) {
      onNotice("This recording isn't available for replay yet.");
      return;
    }
    stopReplay();
    const blob = new Blob([bytes], { type: "audio/wav" });
    audioUrl = URL.createObjectURL(blob);
    audio = new Audio(audioUrl);
    replaying = true;
    audio.onended = stopReplay;
    audio.onerror = () => {
      stopReplay();
      onNotice("The recorded take couldn't be played.");
    };
    try {
      await audio.play();
      onNotice("Playing the recorded take.");
    } catch {
      stopReplay();
      onNotice("The recorded take couldn't be played.");
    }
  }

  onMount(() => {
    disposed = false;
    void refresh();
    void refreshVoiceSelection();
    return () => {
      disposed = true;
      stopReplay();
      onRecordingChange(false);
    };
  });
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">My Voice</h2>
      <p class="ti-page-copy">Choose a voice to use immediately, or record and create a personalized voice later.</p>
      <p class="mb-0 mt-3 text-xs leading-5 text-[var(--ti-text-muted)]">1. Choose a voice · 2. Record samples (optional) · 3. Create and review My Voice (optional)</p>
    </div>
  </header>

  <BuiltinVoicePicker
    {busy}
    {builtinPreviewing}
    {builtinPendingId}
    {selectedVoiceKind}
    {selectedBuiltinVoiceId}
    {savedPersonalVoiceAvailable}
    {selectedVoiceVerified}
    voiceChangesLocked={voiceChangesLocked || isRecording || pendingLineId !== null}
    onPreview={previewBuiltin}
    onSelect={selectBuiltin}
    onSelectPersonal={selectStoredPersonalVoice}
    onConfirm={confirmPendingBuiltin}
    onCancel={() => { builtinPendingId = null; }}
  />

  <div class="mb-5 border-t border-[var(--ti-border)] pt-5">
    <span class="ti-kicker">Optional</span>
    <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Personalize your voice (optional)</h3>
    <p class="mb-0 mt-1.5 text-sm leading-6 text-[var(--ti-text-muted)]">Record varied English lines to prepare a personalized voice. Your accepted recordings remain on this computer, and you review the result before using it.</p>
  </div>

  <div class="grid grid-cols-[minmax(0,1fr)_300px] gap-5">
    <article class="ti-panel p-6">
      <div class="flex items-start justify-between gap-4">
        <div>
          <span class="ti-kicker">My Voice</span>
          <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Step 2 · Record the sample lines</h3>
          <p class="mb-0 mt-2 max-w-[680px] text-sm leading-6 text-[var(--ti-text-muted)]">Use a quiet room and read each line naturally. You can skip any line and return to it later.</p>
        </div>
      </div>

      <label class="mt-6 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] p-4">
        <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} disabled={isRecording} />
        <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I confirm this is my voice, or I have permission to use it.</span>
      </label>

      {#if currentLine}
        <div class="mt-6 rounded-[var(--ti-radius-lg)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] p-6">
          <div class="flex items-center justify-between gap-3">
            <span class="ti-field-label">Line {currentLine.line_id}</span>
            {#if currentLine.accepted}
              <span class="flex items-center gap-1.5 text-xs font-semibold text-[var(--ti-success)]"><Check size={14} />Accepted</span>
            {/if}
          </div>
          <p class="mb-0 mt-5 text-[20px] font-medium leading-8 tracking-[-0.015em]">{currentLine.text}</p>
        </div>

        {#if recordingState.pending_review && pendingLineId === currentLine.line_id}
          <div class="mt-5 border-l-2 border-[var(--ti-border-strong)] pl-4">
            <strong class="text-sm font-semibold">Review this recording</strong>
            <p class={`mb-0 mt-1 text-sm leading-5 ${recordingState.pending_review.quality_blocker ? "text-[var(--ti-danger)]" : "text-[var(--ti-text-muted)]"}`}>
              {recordingState.pending_review.quality_blocker
                ? recordingQualityGuidance(recordingState.pending_review.quality_blocker)
                : "Listen once. Accept it if the recording sounds clear and natural."}
            </p>
          </div>
        {/if}

        <div class="mt-6 flex flex-wrap items-center gap-3">
          {#if isRecording && recordingState.recording_line_id === currentLine.line_id}
            <button type="button" class="ti-button ti-button-danger" disabled={busy} onclick={() => void stopRecording()}>
              <Square size={15} fill="currentColor" /><span>{busy ? "Stopping..." : "Stop"}</span>
            </button>
            <span class="flex items-center gap-2 text-sm text-[var(--ti-danger)]"><span class="size-2 animate-pulse rounded-full bg-current"></span>Recording</span>
          {:else if recordingState.pending_review && pendingLineId === currentLine.line_id}
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || replaying} onclick={() => void replayTake()}>
              <Play size={15} /><span>{replaying ? "Playing..." : "Replay"}</span>
            </button>
            <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void retryTake()}>
              <RotateCcw size={15} /><span>Retry</span>
            </button>
            {#if !recordingState.pending_review.quality_blocker}
              <button type="button" class="ti-button" disabled={busy} onclick={() => void acceptTake()}>
                <Check size={15} /><span>{busy ? "Saving..." : "Accept"}</span>
              </button>
            {/if}
          {:else}
            <button type="button" class="ti-button" disabled={busy || !authorized} onclick={() => void startRecording()}>
              <Mic size={15} /><span>{currentLine.accepted ? "Record Again" : "Record"}</span>
            </button>
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || replaying} onclick={skipLine}>
              <SkipForward size={15} /><span>Skip</span>
            </button>
            {#if currentLine.accepted}
              <button type="button" class="ti-button ti-button-secondary" disabled={busy || replaying} onclick={() => void replayTake()}>
                <Play size={15} /><span>{replaying ? "Playing..." : "Replay Accepted"}</span>
              </button>
            {/if}
          {/if}
        </div>
      {:else}
        <div class="mt-6 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-5 text-sm text-[var(--ti-text-muted)]">My Voice recording lines are unavailable right now.</div>
      {/if}

      <p class="mb-0 mt-7 text-xs leading-5 text-[var(--ti-text-soft)]">Accepted recordings stay on this computer and are only used to create My Voice.</p>
    </article>

    <aside class="ti-panel overflow-hidden">
      <div class="border-b border-[var(--ti-border)] px-4 py-3.5">
        <strong class="text-sm font-semibold">Your sample lines</strong>
        <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">{recordingState.lines.filter((line) => line.accepted).length} of {recordingState.lines.length} accepted · Select a line to record or replay.</p>
      </div>
      <div class="max-h-[590px] overflow-y-auto p-2">
        {#each recordingState.lines as line}
          <button
            type="button"
            class={`flex w-full items-center gap-3 rounded-[9px] px-3 py-2.5 text-left text-sm transition-colors ${selectedLineId === line.line_id ? "bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}`}
            disabled={isRecording || recordingState.pending_review !== null}
            aria-pressed={selectedLineId === line.line_id}
            onclick={() => { selectedLineId = line.line_id; }}
          >
            {#if line.accepted}
              <Check size={15} class="shrink-0 text-[var(--ti-success)]" />
            {:else}
              <Circle size={13} class="shrink-0 text-[var(--ti-text-soft)]" />
            {/if}
            <span class="min-w-0 flex-1 truncate">Line {line.line_id}</span>
          </button>
        {/each}
      </div>
    </aside>
  </div>

  <MyVoiceBuild {onNotice}
    onMeetingVoiceChanged={async (message) => {
      await refreshVoiceSelection();
      await onMeetingVoiceChanged(message);
    }}
    refreshRevision={buildRefreshRevision} />
</section>
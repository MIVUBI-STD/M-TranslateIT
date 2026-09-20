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

  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";

  let {
    onNotice,
    onRecordingChange,
    onMeetingVoiceChanged,
  }: {
    onNotice: (message: string) => void;
    onRecordingChange: (recording: boolean) => void;
    onMeetingVoiceChanged: (message?: string) => void | Promise<void>;
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
      if (!bytes) {
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
    if (busy) return;
    busy = true;
    try {
      const result = await myVoiceBuildApi.selectBuiltin(voiceId, confirmed);
      if (result.state === "approval_required") {
        builtinPendingId = voiceId;
        onNotice(`Confirm before replacing the current Meeting voice with ${builtinLabel(voiceId)}.`);
        return;
      }
      builtinPendingId = null;
      if (result.ok) {
        buildRefreshRevision++;
        await onMeetingVoiceChanged(result.message);
      } else {
        onNotice(result.message);
      }
    } finally {
      busy = false;
    }
  }

  async function confirmPendingBuiltin(): Promise<void> {
    const voiceId = builtinPendingId;
    if (!voiceId) return;
    await selectBuiltin(voiceId, true);
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
    recordingState = next;
    chooseDefaultLine(next);
    onRecordingChange(next.recording_line_id !== null);
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
    applyState(result.recording);
    onNotice(productMessage(result));
  }

  async function refresh(): Promise<void> {
    applyState(await myVoiceApi.getState());
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
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    if (busy || recordingState.recording_line_id === null) return;
    busy = true;
    try {
      applyResult(await myVoiceApi.stopTake(recordingState.recording_line_id));
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
    const bytes = await myVoiceApi.getTakeAudio(selectedLineId);
    if (!bytes) {
      onNotice("This take isn't available for replay yet.");
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
    void refresh();
    return () => {
      stopReplay();
      onRecordingChange(false);
    };
  });
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">Voice for meetings</h2>
      <p class="ti-page-copy">Choose what other people hear when TranslateIT speaks the English translation.</p>
    </div>
  </header>

  <section class="ti-panel mb-6 p-5">
    <span class="ti-kicker">Ready now</span>
    <strong class="mt-2 block text-base font-semibold">Choose a ready-made voice</strong>
    <p class="mb-0 mt-1.5 max-w-[720px] text-sm leading-6 text-[var(--ti-text-muted)]">Pick one of these to start immediately. You can make a personalized voice later.</p>
    <div class="mt-4 grid grid-cols-2 gap-3">
      {#each [
        { id: "MaleVoice" as BuiltinVoiceId, label: "Built-in Male", detail: "Ready-made English voice" },
        { id: "FemaleVoice" as BuiltinVoiceId, label: "Built-in Female", detail: "Ready-made English voice" },
      ] as voice (voice.id)}
        <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface)] p-4">
          <strong class="block text-sm font-semibold">{voice.label}</strong>
          <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">{voice.detail}</p>
          <div class="ti-action-row mt-4">
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || builtinPreviewing !== null} onclick={() => void previewBuiltin(voice.id)}>
              <Play size={14} /> {builtinPreviewing === voice.id ? "Playing..." : "Preview"}
            </button>
            <button type="button" class="ti-button" disabled={busy || builtinPreviewing !== null} onclick={() => void selectBuiltin(voice.id)}>Select</button>
          </div>
        </div>
      {/each}
    </div>

    {#if builtinPendingId}
      <div class="mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
        <strong class="text-sm font-semibold">Replace the current Meeting voice?</strong>
        <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{builtinLabel(builtinPendingId)} will replace the voice currently selected for Meeting.</p>
        <div class="ti-action-row mt-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => { builtinPendingId = null; }}>Cancel</button>
          <button type="button" class="ti-button" disabled={busy} onclick={() => void confirmPendingBuiltin()}>Replace Voice</button>
        </div>
      </div>
    {/if}
  </section>

  <div class="mb-5 border-t border-[var(--ti-border)] pt-5">
    <span class="ti-kicker">Optional</span>
    <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Create a voice that sounds like you</h3>
    <p class="mb-0 mt-1.5 text-sm leading-6 text-[var(--ti-text-muted)]">Record a few English lines so TranslateIT can create your personalized meeting voice.</p>
  </div>

  <div class="grid grid-cols-[minmax(0,1fr)_300px] gap-5">
    <article class="ti-panel p-6">
      <div class="flex items-start justify-between gap-4">
        <div>
          <span class="ti-kicker">My Voice</span>
          <h3 class="mb-0 mt-2 text-xl font-semibold tracking-[-0.02em]">Record the sample lines</h3>
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
            <strong class="text-sm font-semibold">Review this take</strong>
            <p class={`mb-0 mt-1 text-sm leading-5 ${recordingState.pending_review.quality_blocker ? "text-[var(--ti-danger)]" : "text-[var(--ti-text-muted)]"}`}>
              {recordingState.pending_review.quality_blocker
                ? "This recording isn't clear enough yet. Replay it if useful, then try this line again."
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
      </div>
      <div class="max-h-[590px] overflow-y-auto p-2">
        {#each recordingState.lines as line}
          <button
            type="button"
            class={`flex w-full items-center gap-3 rounded-[9px] px-3 py-2.5 text-left text-sm transition-colors ${selectedLineId === line.line_id ? "bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}`}
            disabled={isRecording || recordingState.pending_review !== null}
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

  <MyVoiceBuild {onNotice} {onMeetingVoiceChanged} refreshRevision={buildRefreshRevision} />
</section>
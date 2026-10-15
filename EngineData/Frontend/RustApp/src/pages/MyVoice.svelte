<script lang="ts">
  import { Check, ChevronRight, Circle, Mic, Play, RotateCcw, SkipForward, Square } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import {
    myVoiceApi,
    type GuidedRecordingActionResult,
    type GuidedRecordingState,
    type LegacyProtectionStatus,
    type SavedRecordingPage,
    type SavedRecording,
  } from "../app/bridge/myVoiceApi";
  import { myVoiceBuildApi, type MyVoiceBuildStatus } from "../app/bridge/myVoiceBuildApi";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import { probeProductAudioDevice } from "../app/bridge/productAudioFacade";
  import MyVoiceBuild from "../components/my-voice/MyVoiceBuild.svelte";
  import BuiltinVoicePicker from "../components/my-voice/BuiltinVoicePicker.svelte";
  import VoiceVersionBrowser from "../components/my-voice/VoiceVersionBrowser.svelte";
  import VoiceBackupRestore from "../components/my-voice/VoiceBackupRestore.svelte";

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
  let selectedVoiceReadVersion = 0;
  let recordingStatusUnknown = $state(false);
  let microphoneCheck = $state<"idle" | "checking" | "ready" | "unavailable">("idle");
  let legacyProtection = $state<LegacyProtectionStatus | null>(null);
  let legacyProtectionUnknown = $state(false);
  let legacyProtectionBusy = $state(false);
  let savedLibraryOpen = $state(false);
  let savedLibraryBusy = $state(false);
  let savedLibraryError = $state(false);
  let savedLibraryPage = $state<SavedRecordingPage | null>(null);
  let savedPlayingId = $state<string | null>(null);
  let improvementTakes = $state<Record<number, string>>({});
  let improvementAuthorized = $state(false);
  let improvementStarting = $state(false);
  let trainingComputeAllowed = $state(false);
  const improvementIds = $derived(Object.values(improvementTakes));
  let microphoneCheckMessage = $state("");
  // Presentation-only copy of the build owner's VERIFIED snapshot. No second polling loop.
  let buildSnapshot = $state<MyVoiceBuildStatus | null>(null);
  let buildStatusUnavailable = $state(false);
  let selectedPane = $state<"auto" | "record" | "create" | "review">("auto");
  let creatorOpen = $state(false);
  let browseLinesOpen = $state(false);
  let browseLevel = $state("all");
  let browseFocus = $state("all");
  let browseQuery = $state("");
  let browsePage = $state(0);
  const browsePageSize = 35;
  const levelOptions = [
    { value: "all", label: "All categories" },
    { value: "short", label: "Short Conversations" },
    { value: "natural", label: "Natural Conversations" },
    { value: "detailed", label: "Detailed Explanations" },
  ];
  const browseFocusOptions = $derived(
    [...new Set(recordingState.lines
      .filter((line) => browseLevel === "all" || line.level === browseLevel)
      .map((line) => line.focus))].sort(),
  );
  const browseFiltered = $derived(recordingState.lines.filter((line) =>
    (browseLevel === "all" || line.level === browseLevel) &&
    (browseFocus === "all" || line.focus === browseFocus) &&
    (browseQuery.trim() === "" ||
      line.text.toLowerCase().includes(browseQuery.trim().toLowerCase()) ||
      String(line.line_id).includes(browseQuery.trim())),
  ));
  const browseVisible = $derived(browseFiltered.slice(browsePage * browsePageSize, (browsePage + 1) * browsePageSize));
  // Focus targets are used only after explicit recording or line-selection actions.
  // Polling and background status reconciliation never move keyboard focus.
  let lineHeading: HTMLHeadingElement | undefined;
  let recordActionButton: HTMLButtonElement | undefined;
  let reviewReplayButton: HTMLButtonElement | undefined;

  const activeStage = $derived(
    recordingStatusUnknown || isRecording || pendingLineId !== null
      ? "record"
      : buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active
        ? "create"
        : selectedPane === "record"
          ? "record"
          : buildSnapshot?.evaluation_ready
            ? "review"
            : selectedPane === "create" || buildSnapshot?.can_build
              ? "create"
              : "record",
  );
  // Keep recovery and Stop visible even if a second subsystem's status is unknown.
  // Normal operation still shows only the relevant task pane.
  const showRecordPanel = $derived(activeStage === "record" || isRecording || recordingStatusUnknown);
  const showBuildPanel = $derived(
    activeStage !== "record" || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true,
  );
  const acceptedCount = $derived(recordingState.lines.filter((line) => line.accepted).length);
  const minimumProgress = $derived(
    buildSnapshot && buildSnapshot.minimum_duration_ms > 0 && !buildStatusUnavailable
      ? Math.min(100, Math.round(buildSnapshot.accepted_duration_ms / buildSnapshot.minimum_duration_ms * 100))
      : 0,
  );

  function formatRecordingDuration(milliseconds: number): string {
    const seconds = Math.max(0, Math.floor(milliseconds / 1000));
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  }



  async function loadSavedLibrary(more = false): Promise<void> {
    if (savedLibraryBusy || (more && !savedLibraryPage?.next_cursor)) return;
    savedLibraryBusy = true;
    try {
      const next = await myVoiceApi.listSavedRecordings(more ? savedLibraryPage?.next_cursor ?? null : null);
      if (disposed) return;
      if (next === null) {
        savedLibraryError = true;
        return;
      }
      savedLibraryPage = more && savedLibraryPage
        ? { ...next, items: [...savedLibraryPage.items, ...next.items] }
        : next;
      savedLibraryError = false;
    } catch {
      if (!disposed) savedLibraryError = true;
    } finally {
      savedLibraryBusy = false;
    }
  }

  function toggleSavedLibrary(): void {
    savedLibraryOpen = !savedLibraryOpen;
    if (savedLibraryOpen) void loadSavedLibrary();
  }


  function selectSavedTake(take: SavedRecording): void {
    if (improvementStarting || busy || buildSnapshot?.active || buildSnapshot?.preview_active) return;
    const next = { ...improvementTakes };
    if (next[take.line_id] === take.archive_id) delete next[take.line_id];
    else next[take.line_id] = take.archive_id;
    improvementTakes = next;
  }

  async function startImprovement(): Promise<void> {
    if (improvementStarting || busy || !improvementAuthorized || !trainingComputeAllowed || !savedLibraryOpen ||
        improvementIds.length < 2 || improvementIds.length > 128 ||
        recordingStatusUnknown || isRecording || pendingLineId !== null ||
        buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active ||
        buildSnapshot?.evaluation_ready || activeStage === "review" || voiceChangesLocked) return;
    improvementStarting = true;
    stopReplay();
    try {
      const result = await myVoiceBuildApi.improve([...improvementIds], improvementAuthorized);
      if (disposed) return;
      if (result.build.phase === "unavailable" || result.state === "frontend_bridge_error") {
        onNotice("Couldn't confirm whether improvement started. Check My Voice status before trying again.");
        buildStatusUnavailable = true;
        return;
      }
      onNotice(result.message);
      buildSnapshot = result.build;
      if (result.ok) {
        improvementAuthorized = false;
        selectedPane = "create";
        buildRefreshRevision += 1;
      }
    } catch {
      if (!disposed) {
        buildStatusUnavailable = true;
        onNotice("Couldn't verify the training request. Check status before starting another build.");
      }
    } finally {
      improvementStarting = false;
    }
  }

  async function playSavedRecording(id: string): Promise<void> {
    if (busy || replaying || savedPlayingId || voiceChangesLocked ||
        isRecording || activeStage === "review" || buildSnapshot?.active ||
        buildSnapshot?.preview_active) return;
    stopReplay();
    savedPlayingId = id;
    try {
      const bytes = await myVoiceApi.getSavedRecordingAudio(id);
      if (disposed) return;
      if (!bytes) {
        onNotice("Couldn't verify this saved recording. The file may need attention.");
        stopReplay();
        return;
      }
      audioUrl = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
      audio = new Audio(audioUrl);
      replaying = true;
      audio.onended = stopReplay;
      audio.onerror = () => {
        stopReplay();
        onNotice("This saved recording couldn't be played.");
      };
      await audio.play();
    } catch {
      if (!disposed) onNotice("Couldn't play this saved recording. Try again.");
      stopReplay();
    }
  }

  async function refreshLegacyProtection(): Promise<void> {
    try {
      const status = await myVoiceApi.getLegacyProtection();
      if (disposed) return;
      if (status === null) {
        legacyProtectionUnknown = true;
        return;
      }
      legacyProtection = status;
      legacyProtectionUnknown = false;
    } catch {
      if (!disposed) legacyProtectionUnknown = true;
    }
  }

  async function protectLegacyRecordings(): Promise<void> {
    if (legacyProtectionBusy || busy || voiceChangesLocked || isRecording ||
        pendingLineId !== null || recordingStatusUnknown || buildStatusUnavailable ||
        buildSnapshot?.active || buildSnapshot?.preview_active) return;
    legacyProtectionBusy = true;
    try {
      const result = await myVoiceApi.protectLegacyRecordings();
      if (disposed) return;
      if (!result) {
        legacyProtectionUnknown = true;
        onNotice("Couldn't verify recording protection. Retry status.");
        return;
      }
      legacyProtection = result.report;
      legacyProtectionUnknown = false;
      if (savedLibraryOpen) void loadSavedLibrary();
      onNotice(result.message);
    } catch {
      if (!disposed) {
        legacyProtectionUnknown = true;
        onNotice("Couldn't verify recording protection. Your older files were not intentionally removed.");
      }
    } finally {
      legacyProtectionBusy = false;
    }
  }

  async function refreshVoiceSelection(): Promise<boolean> {
    const requestVersion = ++selectedVoiceReadVersion;
    try {
      const status = await myVoiceBuildApi.getStatus();
      if (disposed || requestVersion !== selectedVoiceReadVersion) return false;
      const verified = status.phase !== "unavailable" && status.selected_voice_kind !== "unknown";
      selectedVoiceVerified = verified;
      if (!verified) return false;
      selectedVoiceKind = status.selected_voice_kind;
      selectedBuiltinVoiceId = status.selected_builtin_voice_id;
      savedPersonalVoiceAvailable = status.saved_personal_voice_available;
      return true;
    } catch {
      if (!disposed && requestVersion === selectedVoiceReadVersion) selectedVoiceVerified = false;
      return false;
    }
  }

  async function verifySelectedVersion(versionId: string): Promise<boolean | null> {
    if (!await refreshVoiceSelection()) return null;
    if (selectedVoiceKind !== "personal") return false;
    // A personal-kind status is not proof of this specific archived version.
    try {
      return await myVoiceBuildApi.verifySelectedVoiceVersion(versionId);
    } catch {
      return null;
    }
  }

  const currentLine = $derived(recordingState.lines.find((line) => line.line_id === selectedLineId) ?? null);
  const isRecording = $derived(recordingState.recording_line_id !== null);
  const pendingLineId = $derived(recordingState.pending_review?.line_id ?? null);
  // Keep the creator mounted while hidden: its build status owns Stop/Review recovery.
  const creatorNeedsAttention = $derived(Boolean(
    recordingStatusUnknown || isRecording || pendingLineId !== null
    || improvementStarting || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active
    || buildSnapshot?.evaluation_ready,
  ));
  $effect(() => {
    if (creatorNeedsAttention) creatorOpen = true;
  });

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
    if (busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active || recordingStatusUnknown || isRecording || pendingLineId !== null) return;
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
    if (busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active || recordingStatusUnknown || isRecording || pendingLineId !== null) return;
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

  async function checkMicrophone(): Promise<void> {
    if (busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active || recordingStatusUnknown || isRecording || pendingLineId !== null) return;
    busy = true;
    microphoneCheck = "checking";
    microphoneCheckMessage = "Checking the selected microphone...";
    try {
      const settings = await runtimeApi.loadSettings();
      if (disposed) return;
      if (!settings) {
        microphoneCheck = "unavailable";
        microphoneCheckMessage = "Couldn't read the selected microphone. Check Voice & Audio in Settings.";
        return;
      }
      const result = await probeProductAudioDevice("microphone", settings.audio.input_device_id);
      if (disposed) return;
      microphoneCheck = result.ok ? "ready" : "unavailable";
      microphoneCheckMessage = result.ok
        ? `${result.deviceName} responds to audio capture. Record a sample to check its sound quality.`
        : `${result.deviceName} could not be checked. Select a working microphone in Settings → Voice & Audio.`;
    } catch {
      if (!disposed) {
        microphoneCheck = "unavailable";
        microphoneCheckMessage = "Couldn't check the microphone. Check Voice & Audio in Settings.";
      }
    } finally {
      busy = false;
    }
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

  function recordingReviewAdvice(code: string): string {
    switch (code) {
      case "voice_lab:review:possible_clipping":
        return "Some peaks may sound distorted. Replay the recording; if it sounds harsh, lower the microphone level and retry.";
      case "voice_lab:review:verify_all_words":
        return "This recording is unusually short for this line. Replay it and check that no words were skipped.";
      case "voice_lab:review:long_edge_pause":
        return "There is a long pause at the start or end. Replay it; next time, begin speaking promptly and stop shortly after finishing.";
      case "voice_lab:review:possible_room_sound":
        return "Sound may be noticeable between words. Replay with headphones; if you hear a fan, hum, or room noise, choose a quieter place and retry.";
      default:
        return "Replay this recording carefully before accepting it.";
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

  // A lost mutation response does not prove the backend operation failed.
  // Keep the recording guard closed until a fresh Rust snapshot is verified.
  async function reconcileUncertainRecordingAction(
    message: string,
    acceptedMayHaveChanged = false,
  ): Promise<void> {
    if (disposed) return;
    recordingStatusUnknown = true;
    onRecordingChange(true);
    onNotice(message);
    await refresh();
    if (!disposed && acceptedMayHaveChanged && !recordingStatusUnknown) {
      // Accept might have reached Rust. Refresh the read-only consumers;
      // never repeat the mutation to guess whether a take was saved.
      buildRefreshRevision += 1;
      void refreshLegacyProtection();
      if (savedLibraryOpen) void loadSavedLibrary();
    }
  }

  async function applyResult(
    result: GuidedRecordingActionResult,
    acceptedMayHaveChanged = false,
  ): Promise<void> {
    if (disposed) return;
    if (result.state === "frontend_bridge_error" || !result.recording) {
      await reconcileUncertainRecordingAction(
        "Couldn't verify the recording action. Checking recording status.",
        acceptedMayHaveChanged,
      );
      return;
    }
    recordingStatusUnknown = false;
    applyState(result.recording);
    onNotice(productMessage(result));
  }

  async function refresh(): Promise<void> {
    try {
      const next = await myVoiceApi.getState();
      if (disposed) return;
      if (!next) {
        recordingStatusUnknown = true;
        onRecordingChange(true);
        onNotice("Couldn't check recording status. Select Retry status.");
        return;
      }
      recordingStatusUnknown = false;
      applyState(next);
    } catch {
      if (!disposed) {
        recordingStatusUnknown = true;
        onRecordingChange(true);
        onNotice("Couldn't check recording status. Select Retry status.");
      }
    }
  }

  async function startRecording(): Promise<void> {
    if (busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active || recordingStatusUnknown || selectedLineId === null) return;
    microphoneCheck = "idle";
    microphoneCheckMessage = "";
    if (!authorized) {
      onNotice("Confirm that this is your voice, or that you have permission to use it.");
      return;
    }
    busy = true;
    try {
      selectedPane = "record";
      await applyResult(await myVoiceApi.startTake(selectedLineId, authorized));
    } catch {
      await reconcileUncertainRecordingAction(
        "Couldn't confirm whether recording started. Checking recording status.",
      );
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    if (busy || recordingState.recording_line_id === null) return;
    busy = true;
    try {
      const result = await myVoiceApi.stopTake(recordingState.recording_line_id);
      await applyResult(result);
      if (result.recording?.pending_review && result.state === "needs_review") {
        // Stop is replaced by Replay; give keyboard users a stable next action.
        busy = false;
        await tick();
        if (!disposed) reviewReplayButton?.focus();
      }
    } catch {
      await reconcileUncertainRecordingAction(
        "Couldn't confirm whether recording stopped. Checking recording status.",
      );
    } finally {
      busy = false;
    }
  }

  async function retryTake(): Promise<void> {
    if (busy || recordingStatusUnknown || pendingLineId === null) return;
    stopReplay();
    busy = true;
    try {
      const result = await myVoiceApi.retryTake(pendingLineId);
      await applyResult(result);
      if (result.ok && result.recording && !result.recording.pending_review) {
        busy = false;
        await tick();
        if (!disposed) recordActionButton?.focus();
      }
    } catch {
      await reconcileUncertainRecordingAction(
        "Couldn't confirm whether the pending recording was discarded. Checking recording status.",
      );
    } finally {
      busy = false;
    }
  }

  async function acceptTake(): Promise<void> {
    if (busy || buildStatusUnavailable || buildSnapshot?.active || buildSnapshot?.preview_active || recordingStatusUnknown || pendingLineId === null || recordingState.pending_review?.quality_blocker) return;
    stopReplay();
    busy = true;
    try {
      const acceptedLineId = pendingLineId;
      const result = await myVoiceApi.acceptTake(acceptedLineId);
      await applyResult(result, true);
      if (result.ok && result.recording) {
        // Keep the recording workspace stable after an explicit Accept.
        // The backend may already permit Create; the user controls that transition.
        selectedPane = "record";
        buildRefreshRevision += 1;
        void refreshLegacyProtection();
        if (savedLibraryOpen) void loadSavedLibrary();
        selectedLineId = result.recording.lines.find((line) => !line.accepted)?.line_id ?? acceptedLineId;
        busy = false;
        await tick();
        if (!disposed) recordActionButton?.focus();
      }
    } catch {
      await reconcileUncertainRecordingAction(
        "Couldn't confirm whether the recording was saved. Checking recording status.",
        true,
      );
    } finally {
      busy = false;
    }
  }

  async function skipLine(): Promise<void> {
    if (busy || recordingStatusUnknown || replaying || isRecording || recordingState.pending_review !== null || selectedLineId === null) return;

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
    await tick();
    if (!disposed) lineHeading?.focus();
  }

  async function chooseBrowseLine(lineId: number): Promise<void> {
    if (busy || recordingStatusUnknown || replaying || isRecording || recordingState.pending_review !== null) return;
    selectedLineId = lineId;
    browseLinesOpen = false;
    // The selected row disappears when the disclosure closes.
    await tick();
    if (!disposed) lineHeading?.focus();
  }

  function stopReplay(): void {
    audio?.pause();
    audio = null;
    replaying = false;
    builtinPreviewing = null;
    savedPlayingId = null;
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
    void refreshLegacyProtection();
    return () => {
      disposed = true;
      selectedVoiceReadVersion += 1;
      stopReplay();
      onRecordingChange(false);
    };
  });
</script>

<section class="ti-page ti-page-wide">
  <header class="ti-page-header">
    <div>
      <h2 class="ti-page-title">My Voice</h2>
      <p class="ti-page-copy">Choose a voice for meetings or create your own.</p>
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
    voiceChangesLocked={voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || recordingStatusUnknown || isRecording || pendingLineId !== null}
    onPreview={previewBuiltin}
    onSelect={selectBuiltin}
    onSelectPersonal={selectStoredPersonalVoice}
    onConfirm={confirmPendingBuiltin}
    onCancel={() => { builtinPendingId = null; }}
    onRetry={refreshVoiceSelection}
  />
  {#if recordingStatusUnknown}
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] p-4" role="alert">
      <p class="m-0 text-sm text-[var(--ti-warning)]">Couldn't verify recording status. The last confirmed state was kept.</p>
      <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={() => void refresh()}>Retry status</button>
    </div>
  {/if}

  <div class="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-[var(--ti-border)] pt-5">
    <div class="min-w-0">
      <h3 class="mb-0 text-base font-semibold">Create your own voice</h3>
      <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">Optional. Built-in voices work without recording or training.</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary"
      aria-controls="my-voice-create-workspace" aria-expanded={creatorOpen}
      disabled={creatorNeedsAttention}
      onclick={() => { creatorOpen = !creatorOpen; }}>
      {creatorNeedsAttention ? "Creation in progress" : creatorOpen ? "Hide creation steps" : "Create My Voice"}
    </button>
  </div>

  <section id="my-voice-create-workspace" hidden={!creatorOpen}
    class="border-t border-[var(--ti-border)] pt-5" aria-labelledby="my-voice-create-title">
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div>
        <h3 id="my-voice-create-title" class="mb-0 text-xl font-semibold tracking-[-0.02em]">Create My Voice</h3>
        <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">Record natural English samples, create your voice, then listen before using it.</p>
      </div>
    </div>
    <nav class="mt-4 flex flex-wrap items-center gap-2 text-xs font-semibold" aria-label="My Voice creation stages">
      <button type="button" aria-current={activeStage === "record" ? "step" : undefined}
        class={activeStage === "record" ? "rounded-full border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] px-3 py-2 text-[var(--ti-text)]" : "rounded-full px-3 py-2 text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}
        disabled={busy || recordingStatusUnknown || isRecording || pendingLineId !== null}
        onclick={() => { selectedPane = "record"; }}>1. Record</button>
      <ChevronRight size={14} class="text-[var(--ti-text-soft)]" aria-hidden="true" />
      <button type="button" aria-current={activeStage === "create" ? "step" : undefined}
        class={activeStage === "create" ? "rounded-full border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] px-3 py-2 text-[var(--ti-text)]" : "rounded-full px-3 py-2 text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}
        disabled={busy || recordingStatusUnknown || buildStatusUnavailable || isRecording || pendingLineId !== null || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || buildSnapshot?.evaluation_ready === true}
        onclick={() => { selectedPane = "create"; }}>2. Create</button>
      <ChevronRight size={14} class="text-[var(--ti-text-soft)]" aria-hidden="true" />
      <button type="button" aria-current={activeStage === "review" ? "step" : undefined}
        class={activeStage === "review" ? "rounded-full border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] px-3 py-2 text-[var(--ti-text)]" : "rounded-full px-3 py-2 text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}
        disabled={busy || recordingStatusUnknown || buildStatusUnavailable || isRecording || pendingLineId !== null || buildSnapshot?.active === true || !buildSnapshot?.evaluation_ready}
        onclick={() => { selectedPane = "review"; }}>3. Review</button>
    </nav>

    {#if showRecordPanel && showBuildPanel}
      <p role="status" class="mb-0 mt-4 text-xs leading-5 text-[var(--ti-warning)]">A status check needs attention. Use Stop or Retry status where available before continuing.</p>
    {/if}
    <div class="mt-4" hidden={!showRecordPanel}>
      <div class="grid grid-cols-1 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_280px]">
        <article class="ti-panel min-w-0 p-5 sm:p-6" aria-labelledby="my-voice-record-title">
          <h4 id="my-voice-record-title" class="mb-0 text-lg font-semibold">Record your voice</h4>
          <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">Use a quiet room and read every word exactly as shown, in your natural voice.</p>

          <div class="mt-5 flex flex-wrap items-center justify-between gap-3 rounded-[var(--ti-radius-md)] bg-[var(--ti-surface-soft)] p-4">
            <div class="min-w-0 flex-1">
              <strong class="text-sm font-semibold">Microphone</strong>
              <p class="mb-0 mt-1 text-xs leading-5 text-[var(--ti-text-muted)]">Check the connection before recording. Sound quality is checked after each take.</p>
            </div>
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || recordingStatusUnknown || isRecording || pendingLineId !== null} onclick={() => void checkMicrophone()}>
              <Mic size={15} /><span>{microphoneCheck === "checking" ? "Checking..." : "Check microphone"}</span>
            </button>
            {#if microphoneCheck !== "idle"}
              <p role="status" class={microphoneCheck === "unavailable" ? "mb-0 w-full text-xs leading-5 text-[var(--ti-danger)]" : "mb-0 w-full text-xs leading-5 text-[var(--ti-text-muted)]"}>{microphoneCheckMessage}</p>
            {/if}
          </div>

          <label class="mt-4 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
            <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox" bind:checked={authorized} disabled={isRecording} />
            <span class="text-sm leading-5 text-[var(--ti-text-muted)]">I confirm this is my voice, or I have permission to use it.</span>
          </label>

          {#if buildSnapshot?.evaluation_ready}
            <p class="mb-0 mt-4 text-xs leading-5 text-[var(--ti-warning)]">Replacing an accepted recording will invalidate the current voice review. You can create and review another candidate afterward.</p>
          {/if}

          {#if currentLine}
            <div class="mt-5 rounded-[var(--ti-radius-lg)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] p-5 sm:p-6">
              <div class="flex items-center justify-between gap-3">
                <h5 class="ti-field-label m-0" tabindex="-1" bind:this={lineHeading} aria-live="polite">Line {currentLine.line_id}</h5>
                {#if currentLine.accepted}
                  <span class="flex items-center gap-1.5 text-xs font-semibold text-[var(--ti-success)]"><Check size={14} />Saved</span>
                {/if}
              </div>
              <p class="mb-0 mt-4 text-[20px] font-medium leading-8 tracking-[-0.015em]">{currentLine.text}</p>
            </div>
            {#if recordingState.pending_review && pendingLineId === currentLine.line_id}
              <div class={recordingState.pending_review.quality_blocker
                ? "mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-danger-border)] p-4"
                : "mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4"}>
                <strong class="text-sm font-semibold">{recordingState.pending_review.quality_blocker ? "Recording needs another take" : (recordingState.pending_review.review_advice?.length ? "Check this recording" : "Listen before saving")}</strong>
                <p class={recordingState.pending_review.quality_blocker ? "mb-0 mt-1 text-sm leading-5 text-[var(--ti-danger)]" : "mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]"}>
                  {recordingState.pending_review.quality_blocker
                    ? recordingQualityGuidance(recordingState.pending_review.quality_blocker)
                    : "Listen once. Keep it only if the voice sounds clear and natural."}
                </p>
                {#if !recordingState.pending_review.quality_blocker}
                  {#each recordingState.pending_review.review_advice ?? [] as advice}
                    <p class="mb-0 mt-2 text-sm leading-5 text-[var(--ti-warning)]">{recordingReviewAdvice(advice)}</p>
                  {/each}
                {/if}
              </div>
            {/if}
            <div class="mt-5 flex flex-wrap items-center gap-3">
              {#if isRecording && recordingState.recording_line_id === currentLine.line_id}
                <button type="button" class="ti-button ti-button-danger" disabled={busy} onclick={() => void stopRecording()}>
                  <Square size={15} fill="currentColor" /><span>{busy ? "Stopping..." : "Stop recording"}</span>
                </button>
                <span role="status" class="flex items-center gap-2 text-sm text-[var(--ti-danger)]"><span class="size-2 animate-pulse rounded-full bg-current"></span>Recording</span>
              {:else if recordingState.pending_review && pendingLineId === currentLine.line_id}
                <button type="button" class="ti-button ti-button-secondary" bind:this={reviewReplayButton} disabled={busy || recordingStatusUnknown || replaying} onclick={() => void replayTake()}>
                  <Play size={15} /><span>{replaying ? "Playing..." : "Replay"}</span>
                </button>
                <button type="button" class={recordingState.pending_review.quality_blocker ? "ti-button" : "ti-button ti-button-secondary"} disabled={busy || recordingStatusUnknown} onclick={() => void retryTake()}>
                  <RotateCcw size={15} /><span>Record again</span>
                </button>
                {#if !recordingState.pending_review.quality_blocker}
                  <button type="button" class="ti-button" disabled={busy || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || recordingStatusUnknown} onclick={() => void acceptTake()}>
                    <Check size={15} /><span>{busy ? "Saving..." : "Save this recording"}</span>
                  </button>
                {/if}
              {:else}
                <button type="button" class="ti-button" bind:this={recordActionButton} disabled={busy || voiceChangesLocked || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || recordingStatusUnknown || !authorized} onclick={() => void startRecording()}>
                  <Mic size={15} /><span>{currentLine.accepted ? "Record again" : "Record"}</span>
                </button>
                <button type="button" class="ti-button ti-button-secondary" disabled={busy || recordingStatusUnknown || replaying} onclick={() => void skipLine()}>
                  <SkipForward size={15} /><span>Skip</span>
                </button>
                {#if currentLine.accepted}
                  <button type="button" class="ti-button ti-button-secondary" disabled={busy || recordingStatusUnknown || replaying} onclick={() => void replayTake()}>
                    <Play size={15} /><span>{replaying ? "Playing..." : "Replay saved"}</span>
                  </button>
                {/if}
              {/if}
            </div>
          {:else}
            <div class="mt-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-5 text-sm text-[var(--ti-text-muted)]">Recording lines are unavailable. Retry status.</div>
          {/if}
          <p class="mb-0 mt-6 text-xs leading-5 text-[var(--ti-text-soft)]">Your accepted recordings stay on this computer.</p>
        </article>

        <aside class="ti-panel min-w-0 overflow-hidden" aria-label="Recording progress and lines">
          <div class="p-4">
            <h4 class="mb-0 text-sm font-semibold">Recording progress</h4>
            {#if buildSnapshot && !buildStatusUnavailable}
              <p class="mb-0 mt-3 text-2xl font-semibold tabular-nums">{formatRecordingDuration(buildSnapshot.accepted_duration_ms)}</p>
              <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">of {formatRecordingDuration(buildSnapshot.minimum_duration_ms)} minimum</p>
              <div class="mt-3 h-1.5 overflow-hidden rounded-full bg-[var(--ti-border)]" role="progressbar" aria-label="Minimum recording duration" aria-valuemin="0" aria-valuemax="100" aria-valuenow={minimumProgress} aria-valuetext={formatRecordingDuration(buildSnapshot.accepted_duration_ms) + " of " + formatRecordingDuration(buildSnapshot.minimum_duration_ms) + " minimum"}>
                <div class="h-full rounded-full bg-[var(--ti-accent)]" style:width={minimumProgress + "%"}></div>
              </div>
              <p class="mb-0 mt-3 text-xs text-[var(--ti-text-muted)]">{buildSnapshot.accepted_take_count} accepted recordings</p>
              {#if buildSnapshot.missing_coverage && buildSnapshot.accepted_duration_ms >= buildSnapshot.minimum_duration_ms}
                <p class="mb-0 mt-3 text-xs leading-5 text-[var(--ti-text-muted)]">Next: Add a sentence covering {buildSnapshot.missing_coverage.label}.</p>

              {:else if !buildSnapshot.can_build}
                <p class="mb-0 mt-3 text-xs leading-5 text-[var(--ti-text-muted)]">{buildSnapshot.message}</p>
              {/if}
              {#if buildSnapshot.recording_suggestions?.length > 0 && !buildSnapshot.active && !isRecording && pendingLineId === null}
                <div class="mt-4" aria-label="Optional recording suggestions">
                  <p class="mb-1 text-xs font-semibold">Suggested next lines</p>
                  <p class="mb-2 text-xs leading-5 text-[var(--ti-text-muted)]">
                    Optional examples of different English sound patterns. These are text-based estimates, not ratings of your pronunciation or voice quality.
                  </p>
                  <div class="space-y-2">
                    {#each buildSnapshot.recording_suggestions as suggestion (suggestion.line_id)}
                      {@const suggestedLine = recordingState.lines.find((line) => line.line_id === suggestion.line_id)}
                      {#if suggestedLine}
                        <button type="button"
                          class="w-full rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] px-3 py-2 text-left hover:bg-[var(--ti-surface-soft)]"
                          disabled={busy || voiceChangesLocked || recordingStatusUnknown || isRecording || pendingLineId !== null}
                          onclick={() => void chooseBrowseLine(suggestion.line_id)}>
                          <span class="block text-xs font-semibold">Line {suggestion.line_id} · {suggestedLine.text}</span>
                          <span class="mt-1 block text-xs text-[var(--ti-text-muted)]">{suggestion.reason}</span>
                        </button>
                      {/if}
                    {/each}
                  </div>
                </div>
              {/if}
              {#if buildSnapshot.evaluation_ready && !isRecording && pendingLineId === null}
                <button type="button" class="ti-button mt-4 w-full" disabled={buildStatusUnavailable || recordingStatusUnknown || busy} onclick={() => { selectedPane = "review"; }}>Return to Review</button>
              {:else if buildSnapshot.can_build && !buildSnapshot.active && !isRecording && pendingLineId === null}
                <button type="button" class="ti-button mt-4 w-full" disabled={voiceChangesLocked || buildStatusUnavailable || recordingStatusUnknown} onclick={() => { selectedPane = "create"; }}>Continue to Create</button>
              {/if}
            {:else}
              <p role="status" class="mb-0 mt-3 text-sm text-[var(--ti-text-muted)]">Checking recording progress…</p>
            {/if}
            {#if acceptedCount > 0 && !buildSnapshot?.active && !buildSnapshot?.evaluation_ready}
              <button type="button" class="mt-4 text-left text-xs font-medium text-[var(--ti-text-muted)] underline underline-offset-4" disabled={busy || buildStatusUnavailable || recordingStatusUnknown || isRecording || pendingLineId !== null} onclick={() => { selectedPane = "create"; }}>Optional Quick Preview</button>
            {/if}
          </div>
          <div class="border-t border-[var(--ti-border)] px-4 py-3">
            <details bind:open={browseLinesOpen}>
              <summary class="cursor-pointer text-sm font-medium">Browse lines <span class="ml-1 text-xs font-normal text-[var(--ti-text-muted)]">({acceptedCount} saved)</span></summary>
              <p class="mb-2 mt-2 text-xs leading-5 text-[var(--ti-text-muted)]">Choose what feels natural. You don't have to record all {recordingState.lines.length} sentences.</p>
              <div class="space-y-2">
                <label class="block text-xs text-[var(--ti-text-muted)]">
                  Category
                  <select class="ti-input mt-1 w-full" bind:value={browseLevel}
                    onchange={() => { browsePage = 0; browseFocus = "all"; }}>
                    {#each levelOptions as option}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                </label>
                <label class="block text-xs text-[var(--ti-text-muted)]">
                  Focus
                  <select class="ti-input mt-1 w-full" bind:value={browseFocus}
                    onchange={() => { browsePage = 0; }}>
                    <option value="all">All topics</option>
                    {#each browseFocusOptions as focus}
                      <option value={focus}>{focus.replaceAll("-", " ")}</option>
                    {/each}
                  </select>
                </label>
                <label class="block text-xs text-[var(--ti-text-muted)]">
                  Search line number or words
                  <input class="ti-input mt-1 w-full" type="search" bind:value={browseQuery}
                    oninput={() => { browsePage = 0; }} placeholder="Search sentences" />
                </label>
              </div>
              <p class="mt-3 text-xs text-[var(--ti-text-muted)]">{browseFiltered.length} matching lines · up to {browsePageSize} per page</p>
              <div class="max-h-[320px] overflow-y-auto">
                {#each browseVisible as line (line.line_id)}
                  <button type="button"
                    class={selectedLineId === line.line_id ? "mt-1 flex w-full items-center gap-2 rounded-[var(--ti-radius-sm)] bg-[var(--ti-surface-raised)] px-2 py-2 text-left text-sm" : "mt-1 flex w-full items-center gap-2 rounded-[var(--ti-radius-sm)] px-2 py-2 text-left text-sm text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"}
                    disabled={isRecording || recordingStatusUnknown || recordingState.pending_review !== null}
                    aria-pressed={selectedLineId === line.line_id}
                    aria-label={`Line ${line.line_id}, ${line.accepted ? "saved" : "not recorded"}`}
                    onclick={() => void chooseBrowseLine(line.line_id)}
                  >
                    {#if line.accepted}
                      <Check size={14} class="shrink-0 text-[var(--ti-success)]" />
                    {:else}
                      <Circle size={12} class="shrink-0 text-[var(--ti-text-soft)]" />
                    {/if}
                    <span class="min-w-0 truncate">Line {line.line_id} · {line.text}</span>
                  </button>
                {/each}
              </div>
              <div class="mt-3 flex justify-between gap-3">
                <button type="button" class="ti-button ti-button-secondary" disabled={browsePage === 0}
                  onclick={() => { browsePage = Math.max(0, browsePage - 1); }}>Previous</button>
                <button type="button" class="ti-button ti-button-secondary" disabled={(browsePage + 1) * browsePageSize >= browseFiltered.length}
                  onclick={() => { browsePage += 1; }}>Next</button>
              </div>
            </details>
          </div>
        </aside>
      </div>
    </div>

    <div class="mt-4" hidden={!showBuildPanel}>
      <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
        <p class="mb-0 text-xs leading-5 text-[var(--ti-text-muted)]">
          {activeStage === "review" ? "Review every generated sample before selecting your new voice." : "Your current Meeting voice stays unchanged until you approve a new one."}
        </p>
        <button type="button" class="ti-button ti-button-secondary" disabled={recordingStatusUnknown || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || busy} onclick={() => { selectedPane = "record"; }}>
          Record more
        </button>
      </div>
      <MyVoiceBuild {onNotice}
        voiceChangesLocked={voiceChangesLocked || recordingStatusUnknown || isRecording || pendingLineId !== null}
        onMeetingVoiceChanged={async (message) => {
          await refreshVoiceSelection();
          await onMeetingVoiceChanged(message);
        }}
        onBuildStatus={(status) => { buildSnapshot = status; buildStatusUnavailable = false; }}
        onBuildStatusUnavailable={() => { buildStatusUnavailable = true; }}
        onTrainingComputeEligibility={(allowed) => { trainingComputeAllowed = allowed; }}
        refreshRevision={buildRefreshRevision} />
    </div>
  </section>
  <details class="mt-6 overflow-hidden rounded-[var(--ti-radius-md)] border border-[var(--ti-border)]" aria-label="Saved voice management">
    <summary class="cursor-pointer px-5 py-4 text-sm font-semibold">
      Saved voices &amp; backups
      {#if legacyProtectionUnknown || (legacyProtection?.attention_sources ?? 0) > 0}
        <span class="ml-2 text-xs font-medium text-[var(--ti-warning)]">Recording storage needs attention</span>
      {/if}
    </summary>
    <div class="border-t border-[var(--ti-border)] px-5 pb-5">
      <p class="mb-0 mt-4 text-sm leading-5 text-[var(--ti-text-muted)]">Advanced tools for earlier recordings, saved voice versions and backups. Your current Meeting voice is unchanged by opening these tools.</p>
  <section class="mt-4" aria-label="Recording library">
    <button type="button" class="text-left text-sm font-medium text-[var(--ti-text-muted)] underline underline-offset-4"
      aria-expanded={savedLibraryOpen}
      onclick={toggleSavedLibrary}>Saved recordings</button>
    {#if savedLibraryOpen}
      <div class="ti-panel mt-3 p-4 sm:p-5">
        <p class="mb-2 text-sm leading-5 text-[var(--ti-text-muted)]">
          Your saved recording versions remain on this computer. Listen to a saved take without changing the one used for training.
        </p>
        {#if savedLibraryError}
          <p role="alert" class="text-sm text-[var(--ti-warning)]">
            Couldn't verify saved recordings. Some files may need attention.
          </p>
          <button type="button" class="ti-button ti-button-secondary" disabled={savedLibraryBusy}
            onclick={() => void loadSavedLibrary()}>Retry library</button>
        {:else if savedLibraryBusy && !savedLibraryPage}
          <p role="status" class="text-sm text-[var(--ti-text-muted)]">Checking saved recordings...</p>
        {:else if savedLibraryPage}
          {#if savedLibraryPage.attention_entries > 0}
            <p role="alert" class="mb-3 text-sm text-[var(--ti-warning)]">
              {savedLibraryPage.attention_entries} archive entries need attention and are not shown as verified.
            </p>
          {/if}
          {#if savedLibraryPage.items.length === 0}
            <p class="text-sm text-[var(--ti-text-muted)]">No verified saved recordings in this page yet.</p>
          {:else}
            <div class="space-y-2">
              {#each savedLibraryPage.items as take (take.archive_id)}
                <div class="flex flex-wrap items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
                  <div class="min-w-0 flex-1">
                    <p class="m-0 text-xs font-semibold">Line {take.line_id} · {formatRecordingDuration(take.duration_ms)}</p>
                    <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{take.text}</p>
                  </div>

                  <label class="flex shrink-0 items-center gap-2 text-xs text-[var(--ti-text-muted)]">
                    <input type="checkbox" class="size-4 accent-[var(--ti-accent)]"
                      checked={improvementTakes[take.line_id] === take.archive_id}
                      disabled={improvementStarting || busy || isRecording || buildSnapshot?.active === true || buildSnapshot?.preview_active === true}
                      onchange={() => selectSavedTake(take)} />
                    Include
                  </label>
                  <button type="button" class="ti-button ti-button-secondary"
                    disabled={busy || replaying || voiceChangesLocked || isRecording || activeStage === "review" || buildSnapshot?.active === true || buildSnapshot?.preview_active === true}
                    onclick={() => void playSavedRecording(take.archive_id)}>
                    <Play size={14} /><span>{savedPlayingId === take.archive_id ? "Playing..." : "Listen"}</span>
                  </button>
                </div>
              {/each}
            </div>
          {/if}

          <div class="mt-4 border-t border-[var(--ti-border)] pt-4">
            <strong class="text-sm font-semibold">Improve My Voice</strong>
            <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">
              Choose one saved recording per sentence. You can mix earlier and newer recordings.
              Your current Meeting voice and all saved takes will be kept.
            </p>
            <p class="mt-2 text-sm font-medium">{improvementIds.length} recordings chosen · maximum 128 per build</p>
            {#if !trainingComputeAllowed}
              <p class="mb-0 mt-2 text-xs leading-5 text-[var(--ti-text-muted)]" role="status">Before improving, open step 2 (Create) to check local training hardware. CPU-only use requires acknowledging the warning there.</p>
            {/if}
            <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">
              The training check requires at least one minute of speech, a short reference recording and varied sentence categories. You don't need to finish every line.
            </p>
            <label class="mt-3 flex items-start gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
              <input type="checkbox" class="mt-0.5 size-4 accent-[var(--ti-accent)]"
                bind:checked={improvementAuthorized} disabled={improvementStarting} />
              <span class="text-sm text-[var(--ti-text-muted)]">I own these recordings or have permission to create this personal voice.</span>
            </label>
            <button type="button" class="ti-button mt-3"
              disabled={improvementStarting || busy || !improvementAuthorized || !trainingComputeAllowed || improvementIds.length < 2 || improvementIds.length > 128 || voiceChangesLocked || recordingStatusUnknown || isRecording || pendingLineId !== null || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true || buildSnapshot?.evaluation_ready === true || activeStage === "review"}
              onclick={() => void startImprovement()}>
              {improvementStarting ? "Starting..." : "Create improved version"}
            </button>
          </div>
          {#if savedLibraryPage.next_cursor}
            <button type="button" class="ti-button ti-button-secondary mt-3" disabled={savedLibraryBusy}
              onclick={() => void loadSavedLibrary(true)}>
              {savedLibraryBusy ? "Loading..." : "Show more recordings"}
            </button>
          {/if}
        {/if}
      </div>
    {/if}
  </section>

  <VoiceVersionBrowser
    voiceChangesLocked={voiceChangesLocked || busy || !selectedVoiceVerified || buildStatusUnavailable ||
      buildSnapshot?.active === true || buildSnapshot?.preview_active === true ||
      recordingStatusUnknown || isRecording || pendingLineId !== null}
    {onNotice}
    onVoiceActivated={async (versionId, message) => {
      if (await verifySelectedVersion(versionId) !== true) {
        throw new Error("selected_meeting_voice_unverified");
      }
      buildRefreshRevision += 1;
      await onMeetingVoiceChanged(message);
    }}
    onRecheck={verifySelectedVersion}
  />


  <VoiceBackupRestore
    voiceChangesLocked={voiceChangesLocked || busy || improvementStarting || recordingStatusUnknown ||
      isRecording || pendingLineId !== null || buildStatusUnavailable ||
      buildSnapshot?.active === true || buildSnapshot?.preview_active === true}
    {onNotice}
    onRestored={async () => {
      savedLibraryPage = null;
      if (savedLibraryOpen) await loadSavedLibrary();
      await refreshLegacyProtection();
    }}
  />

  {#if legacyProtectionUnknown || (legacyProtection && (legacyProtection.total_sources > 0 || legacyProtection.attention_sources > 0))}
    <section class="ti-panel mt-4 p-4 sm:p-5" aria-label="Saved recording protection">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <strong class="text-sm font-semibold">Protect earlier recordings</strong>
          {#if legacyProtectionUnknown}
            <p role="status" class="mb-0 mt-1 text-sm text-[var(--ti-warning)]">
              Couldn't verify older recordings. Their original files were not intentionally removed.
            </p>
          {:else if legacyProtection}
            <p class="mb-0 mt-1 text-sm text-[var(--ti-text-muted)]">
              {legacyProtection.protected_sources} of {legacyProtection.total_sources} older recordings verified in saved storage. Original files stay on this computer.
            </p>
            {#if legacyProtection.attention_sources > 0}
              <p role="alert" class="mb-0 mt-2 text-sm text-[var(--ti-warning)]">
                {legacyProtection.attention_sources} recording file issues need attention. Files have not been deleted.
              </p>
            {/if}
          {/if}
        </div>
        {#if legacyProtectionUnknown}
          <button type="button" class="ti-button ti-button-secondary" disabled={legacyProtectionBusy} onclick={() => void refreshLegacyProtection()}>Retry status</button>
        {:else if legacyProtection && (legacyProtection.unprotected_sources > 0 || legacyProtection.attention_sources > 0)}
          <button type="button" class="ti-button ti-button-secondary"
            disabled={legacyProtectionBusy || busy || voiceChangesLocked || isRecording || pendingLineId !== null || recordingStatusUnknown || buildStatusUnavailable || buildSnapshot?.active === true || buildSnapshot?.preview_active === true}
            onclick={() => void protectLegacyRecordings()}>
            {legacyProtectionBusy ? "Protecting..." : "Save protected copies"}
          </button>
        {/if}
      </div>
    </section>
  {/if}

    </div>
  </details>

</section>

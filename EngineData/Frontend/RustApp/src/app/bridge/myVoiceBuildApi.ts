import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { runCommand } from "../shared/tauriBridge";

export type QuickVoicePreviewResult = {
  ok: boolean;
  state: string;
  message: string;
};

export type MyVoiceEvaluationSample = {
  line_id: number;
  exact_text: string;
  wav_file: string;
  sha256: string;
  speaker_similarity: number;
  intelligibility_text: string;
  intelligibility_wer: number;
  artifact_flags: string[];
};

export type MyVoiceRecordingSuggestion = {
  line_id: number;
  reason: string;
};

export type MyVoiceCoverageGuidance = {
  start_line_id: number;
  end_line_id: number;
  label: string;
  suggested_line_id: number | null;
};

export type MyVoiceComputeStatus = {
  state: "cuda_detected" | "cpu_only" | "unavailable";
  cpu_name: string;
  gpu_name: string | null;
  vram_mib: number | null;
  free_vram_mib: number | null;
  reserve_mib: number | null;
  safe_budget_mib: number | null;
  admission_state: "admitted" | "blocked_gpu_headroom" | "unknown";
  profile: string;
  detail: string;
};

export type MyVoiceBuildStatus = {
  active: boolean;
  preview_active: boolean;
  generation: number | null;
  phase: string;
  message: string;
  accepted_take_count: number;
  accepted_duration_ms: number;
  minimum_duration_ms: number;
  missing_coverage: MyVoiceCoverageGuidance | null;
  recording_suggestions: MyVoiceRecordingSuggestion[];
  can_build: boolean;
  evaluation_ready: boolean;
  evaluation_review_id: string | null;
  evaluation_samples: MyVoiceEvaluationSample[];
  approved_voice_ready: boolean;
  selected_voice_kind: "built_in" | "personal" | "none" | "unknown";
  selected_builtin_voice_id: "MaleVoice" | "FemaleVoice" | null;
  saved_personal_voice_available: boolean;
};

export type ActorVersionItem = {
  version_id: string;
  engine_revision: string;
  verification_required: boolean;
};

export type ActorVersionPage = {
  items: ActorVersionItem[];
  next_cursor: string | null;
  attention_entries: number;
};

export type MyVoiceBuildActionResult = {
  ok: boolean;
  state: string;
  message: string;
  build: MyVoiceBuildStatus;
};

export type MyVoiceBuildRuntimeEvent = {
  revision: number;
  reason: string;
  generation: number;
};

function unavailableStatus(): MyVoiceBuildStatus {
  return {
    active: false,
    preview_active: false,
    generation: null,
    phase: "unavailable",
    message: "My Voice build status is unavailable.",
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
  };
}

function productMessage(message: string): string {
  return message.replace(/\bVoiceLab\b/g, "My Voice");
}

function normalizeStatus(status: MyVoiceBuildStatus): MyVoiceBuildStatus {
  return {
    ...status,
    missing_coverage: status.missing_coverage ?? null,
    recording_suggestions: status.recording_suggestions ?? [],
    message: productMessage(status.message),
  };
}

function unavailableAction(message: string): MyVoiceBuildActionResult {
  return { ok: false, state: "frontend_bridge_error", message, build: unavailableStatus() };
}

function normalizeAction(action: MyVoiceBuildActionResult): MyVoiceBuildActionResult {
  return {
    ...action,
    message: productMessage(action.message),
    build: normalizeStatus(action.build),
  };
}

export const myVoiceBuildApi = {
  // Explicit and bounded. Never invoke this from build-status polling.
  async getComputeStatus(): Promise<MyVoiceComputeStatus | null> {
    return runCommand<MyVoiceComputeStatus>("get_my_voice_compute_status");
  },

  async listActorVersions(after: string | null = null): Promise<ActorVersionPage | null> {
    return runCommand<ActorVersionPage>("get_voice_lab_actor_versions", { after });
  },

  async verifySelectedVoiceVersion(versionId: string): Promise<boolean | null> {
    // Explicit-only streaming SHA-256 of the current selected model, not
    // part of the routine status poll or selected voice kind check.
    if (!/^[0-9a-f]{64}$/.test(versionId)) return null;
    return runCommand<boolean>("get_voice_lab_selected_version_match", { versionId });
  },
  async selectActorVersion(versionId: string, replaceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("select_product_voice_version", {
      versionId, replaceConfirmed,
    });
    return action ? normalizeAction(action) : unavailableAction("Could not verify the saved version selection.");
  },
  async getStatus(): Promise<MyVoiceBuildStatus> {
    const status = await runCommand<MyVoiceBuildStatus>("get_voice_lab_build_status");
    return status ? normalizeStatus(status) : unavailableStatus();
  },

  async start(authorizedVoiceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("start_product_voice_build", { authorizedVoiceConfirmed });
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not start creating your voice.");
  },

  async improve(archiveIds: string[], authorizedVoiceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("start_product_voice_improvement", {
      archiveIds, authorizedVoiceConfirmed,
    });
    return action ? normalizeAction(action) : unavailableAction(
      "Couldn't verify whether My Voice improvement started. Check status before trying again.",
    );
  },

  async cancel(): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("cancel_product_voice_build");
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not confirm that creation stopped.");
  },

  async approve(reviewedLineIds: number[], qualityConfirmed: boolean, reviewId: string): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("approve_product_voice_candidate", {
      reviewedLineIds,
      qualityConfirmed,
      reviewId,
    });
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not approve the new voice.");
  },

  // These actions are for My Voice creation only. No selection or Meeting mutation.
  async generateQuickPreview(): Promise<QuickVoicePreviewResult> {
    return (await runCommand<QuickVoicePreviewResult>("generate_voice_lab_quick_preview"))
      ?? { ok: false, state: "unavailable", message: "Quick Preview is unavailable." };
  },

  async cancelQuickPreview(): Promise<QuickVoicePreviewResult> {
    return (await runCommand<QuickVoicePreviewResult>("cancel_voice_lab_quick_preview"))
      ?? { ok: false, state: "unavailable", message: "Quick Preview cancellation is unavailable." };
  },

  async getQuickPreviewAudio(): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_voice_lab_quick_preview_audio");
  },

  async getBuiltinPreviewAudio(voiceId: string): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_builtin_voice_preview_audio", { voiceId });
  },

  async selectBuiltin(voiceId: string, authorizedVoiceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("select_product_builtin_voice", {
      voiceId,
      authorizedVoiceConfirmed,
    });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not switch to that built-in voice.");
  },

  async selectSavedPersonalVoice(): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("select_product_saved_personal_voice");
    return action ? normalizeAction(action) : unavailableAction("Could not select your saved My Voice.");
  },

  async getEvaluationAudio(lineId: number, reviewId: string, expectedSha256: string): Promise<ArrayBuffer | null> {
    if (!/^[0-9a-f]{64}$/.test(expectedSha256)) return null;
    const bytes = await runCommand<ArrayBuffer>("get_voice_lab_evaluation_audio", { lineId, reviewId });
    const subtle = globalThis.crypto?.subtle;
    if (!bytes || bytes.byteLength < 44 || !subtle) return null;
    try {
      const digest = await subtle.digest("SHA-256", bytes);
      const actual = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
      return actual === expectedSha256 ? bytes : null;
    } catch {
      return null;
    }
  },

  subscribeRuntime(
    listener: (event: MyVoiceBuildRuntimeEvent) => void,
  ): Promise<UnlistenFn> {
    return listen<MyVoiceBuildRuntimeEvent>("translateit://voice-build-runtime", (event) => {
      listener(event.payload);
    });
  },
};

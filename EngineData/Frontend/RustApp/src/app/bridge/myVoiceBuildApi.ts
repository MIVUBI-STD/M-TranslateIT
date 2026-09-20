import { runCommand } from "../shared/tauriBridge";

export type MyVoiceEvaluationSample = {
  line_id: number;
  exact_text: string;
  wav_file: string;
  speaker_similarity: number;
};

export type MyVoiceCoverageGuidance = {
  start_line_id: number;
  end_line_id: number;
  label: string;
};

export type MyVoiceBuildStatus = {
  active: boolean;
  generation: number | null;
  phase: string;
  message: string;
  accepted_take_count: number;
  accepted_duration_ms: number;
  minimum_duration_ms: number;
  missing_coverage: MyVoiceCoverageGuidance | null;
  can_build: boolean;
  evaluation_ready: boolean;
  evaluation_samples: MyVoiceEvaluationSample[];
  approved_voice_ready: boolean;
};

export type MyVoiceBuildActionResult = {
  ok: boolean;
  state: string;
  message: string;
  build: MyVoiceBuildStatus;
};

function unavailableStatus(): MyVoiceBuildStatus {
  return {
    active: false,
    generation: null,
    phase: "unavailable",
    message: "My Voice build status is unavailable.",
    accepted_take_count: 0,
    accepted_duration_ms: 0,
    minimum_duration_ms: 60_000,
    missing_coverage: null,
    can_build: false,
    evaluation_ready: false,
    evaluation_samples: [],
    approved_voice_ready: false,
  };
}

function productMessage(message: string): string {
  return message.replace(/\bVoiceLab\b/g, "My Voice");
}

function normalizeStatus(status: MyVoiceBuildStatus): MyVoiceBuildStatus {
  return {
    ...status,
    missing_coverage: status.missing_coverage ?? null,
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
  async getStatus(): Promise<MyVoiceBuildStatus> {
    const status = await runCommand<MyVoiceBuildStatus>("get_voice_lab_build_status");
    return status ? normalizeStatus(status) : unavailableStatus();
  },

  async start(authorizedVoiceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("start_voice_lab_build", { authorizedVoiceConfirmed });
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not start creating your voice.");
  },

  async cancel(): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("cancel_voice_lab_build");
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not confirm that creation stopped.");
  },

  async approve(): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("approve_voice_lab_candidate");
    return action
      ? normalizeAction(action)
      : unavailableAction("My Voice could not approve the new voice.");
  },

  async getBuiltinPreviewAudio(voiceId: string): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_builtin_voice_preview_audio", { voiceId });
  },

  async selectBuiltin(voiceId: string, authorizedVoiceConfirmed: boolean): Promise<MyVoiceBuildActionResult> {
    const action = await runCommand<MyVoiceBuildActionResult>("select_builtin_voice", {
      voiceId,
      authorizedVoiceConfirmed,
    });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not switch to that built-in voice.");
  },

  async getEvaluationAudio(lineId: number): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_voice_lab_evaluation_audio", { lineId });
  },
};

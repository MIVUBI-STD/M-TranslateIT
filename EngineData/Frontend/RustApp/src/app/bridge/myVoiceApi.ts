import { runCommand } from "../shared/tauriBridge";

export type GuidedTakeReview = {
  line_id: number;
  duration_ms: number;
  quality_blocker: string;
};

export type GuidedLineStatus = {
  line_id: number;
  text: string;
  accepted: boolean;
};

export type GuidedRecordingState = {
  recording_line_id: number | null;
  pending_review: GuidedTakeReview | null;
  lines: GuidedLineStatus[];
};

export type GuidedRecordingActionResult = {
  ok: boolean;
  state: string;
  message: string;
  recording: GuidedRecordingState;
};

function unavailableState(): GuidedRecordingState {
  return { recording_line_id: null, pending_review: null, lines: [] };
}

function unavailableAction(message: string): GuidedRecordingActionResult {
  return { ok: false, state: "frontend_bridge_error", message, recording: unavailableState() };
}

function productMessage(message: string): string {
  return message.replace(/\bVoiceLab\b/g, "My Voice");
}

function normalizeAction(action: GuidedRecordingActionResult): GuidedRecordingActionResult {
  return { ...action, message: productMessage(action.message) };
}

export const myVoiceApi = {
  async getState(): Promise<GuidedRecordingState> {
    return (await runCommand<GuidedRecordingState>("get_voice_lab_guided_recording_state")) ?? unavailableState();
  },

  async startTake(lineId: number, authorizedVoiceConfirmed: boolean): Promise<GuidedRecordingActionResult> {
    const action = await runCommand<GuidedRecordingActionResult>("start_product_voice_recording", {
      lineId,
      authorizedVoiceConfirmed,
    });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not start recording.");
  },

  async stopTake(lineId: number): Promise<GuidedRecordingActionResult> {
    const action = await runCommand<GuidedRecordingActionResult>("stop_product_voice_recording", { lineId });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not stop recording safely.");
  },

  async retryTake(lineId: number): Promise<GuidedRecordingActionResult> {
    const action = await runCommand<GuidedRecordingActionResult>("retry_voice_lab_guided_take", { lineId });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not discard the review take.");
  },

  async acceptTake(lineId: number): Promise<GuidedRecordingActionResult> {
    const action = await runCommand<GuidedRecordingActionResult>("accept_voice_lab_guided_take", { lineId });
    return action ? normalizeAction(action) : unavailableAction("My Voice could not save the accepted take.");
  },

  async getTakeAudio(lineId: number): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_voice_lab_guided_take_audio", { lineId });
  },
};

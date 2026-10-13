import { runCommand } from "../shared/tauriBridge";

export type GuidedTakeReview = {
  line_id: number;
  duration_ms: number;
  quality_blocker: string;
  review_advice: string[];
};

export type GuidedLineStatus = {
  line_id: number;
  text: string;
  accepted: boolean;
  level: "short" | "natural" | "detailed";
  focus: string;
  coverage: "conversation" | "intonation" | "natural" | "details" | "explanations";
};

export type GuidedRecordingState = {
  recording_line_id: number | null;
  pending_review: GuidedTakeReview | null;
  lines: GuidedLineStatus[];
};

export type SavedRecording = {
  archive_id: string;
  line_id: number;
  text: string;
  duration_ms: number;
  native_rate_available: boolean;
};

export type SavedRecordingPage = {
  items: SavedRecording[];
  next_cursor: string | null;
  attention_entries: number;
};

export type LegacyProtectionStatus = {
  total_sources: number;
  protected_sources: number;
  unprotected_sources: number;
  attention_sources: number;
};

export type LegacyMigrationResult = {
  ok: boolean;
  newly_archived: number;
  report: LegacyProtectionStatus;
  message: string;
};

export type TrainingRecoveryStatus = {
  preserved_checkpoint_candidates: number;
  attention_entries: number;
  resume_available: boolean;
};

export type MyVoiceBackupReport = {
  ok: boolean;
  message: string;
  backup_folder: string | null;
  file_count: number;
  total_bytes: number;
  newly_imported: number;
  already_present: number;
};

export type GuidedRecordingActionResult = {
  ok: boolean;
  state: string;
  message: string;
  recording: GuidedRecordingState | null;
};

function unavailableAction(message: string): GuidedRecordingActionResult {
  // A failed mutation may have reached Rust. Null means no verified recording state.
  return { ok: false, state: "frontend_bridge_error", message, recording: null };
}

function productMessage(message: string): string {
  return message.replace(/\bVoiceLab\b/g, "My Voice");
}

function normalizeAction(action: GuidedRecordingActionResult): GuidedRecordingActionResult {
  return { ...action, message: productMessage(action.message) };
}

export const myVoiceApi = {
  async getTrainingRecoveryStatus(): Promise<TrainingRecoveryStatus | null> {
    return runCommand<TrainingRecoveryStatus>("get_voice_lab_training_recovery_status");
  },
  async createBackup(destinationDir: string): Promise<MyVoiceBackupReport | null> {
    return runCommand<MyVoiceBackupReport>("create_voice_lab_backup", { destinationDir });
  },

  async verifyBackup(backupFolder: string): Promise<MyVoiceBackupReport | null> {
    return runCommand<MyVoiceBackupReport>("verify_voice_lab_backup", { backupFolder });
  },

  async restoreBackup(backupFolder: string, confirmed: boolean): Promise<MyVoiceBackupReport | null> {
    return runCommand<MyVoiceBackupReport>("restore_voice_lab_backup", { backupFolder, confirmed });
  },
  async listSavedRecordings(after: string | null = null): Promise<SavedRecordingPage | null> {
    return runCommand<SavedRecordingPage>("list_voice_lab_saved_recordings", { after });
  },

  async getSavedRecordingAudio(archiveId: string): Promise<ArrayBuffer | null> {
    return runCommand<ArrayBuffer>("get_voice_lab_saved_recording_audio", { archiveId });
  },
  async getLegacyProtection(): Promise<LegacyProtectionStatus | null> {
    return runCommand<LegacyProtectionStatus>("get_voice_lab_legacy_archive_status");
  },

  async protectLegacyRecordings(): Promise<LegacyMigrationResult | null> {
    return runCommand<LegacyMigrationResult>("protect_voice_lab_legacy_recordings");
  },
  async getState(): Promise<GuidedRecordingState | null> {
    // A failed bridge read is unknown, never evidence that recording stopped.
    return runCommand<GuidedRecordingState>("get_voice_lab_guided_recording_state");
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

import {
  applicationRuntimeApi,
  type ApplicationSnapshot,
} from "./applicationRuntimeApi";
import { myVoiceBuildApi } from "./myVoiceBuildApi";
import { runtimeApi, type MeetingSessionActionResult } from "./runtimeApi";
import { compact } from "../shared/state";
import type { RuntimeSettings } from "../shared/types";
import {
  mapProductMeetingState,
  mapProductReadiness,
} from "./runtimeProductState";
import type {
  ProductMeetingState,
  ProductRuntimeSnapshot,
} from "./runtimeProductTypes";
import {
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
} from "./productAudioFacade";
import {
  runProductTranslation,
  runProductTranslationAlternative,
} from "./productTranslationFacade";
import {
  runProductRecoveryAction,
  runProductSetupAction,
} from "./productSetupFacade";

export {
  mapProductMeetingState,
  mapProductReadiness,
  meetingBridgeUnavailable,
  parseWorkerCapabilities,
} from "./runtimeProductState";
export {
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
} from "./productAudioFacade";
export {
  runProductTranslation,
  runProductTranslationAlternative,
} from "./productTranslationFacade";
export {
  runProductRecoveryAction,
  runProductSetupAction,
} from "./productSetupFacade";

export type {
  ProductAudioDeviceKind,
  ProductAudioDeviceProbe,
  ProductAudioDeviceSelectionResult,
} from "./productAudioFacade";
export type { ProductTranslationResult } from "./productTranslationFacade";
export type {
  ProductRecoveryAction,
  ProductSetupAction,
} from "./productSetupFacade";
export type {
  ProductMeetingState,
  ProductReadiness,
  ProductReadinessLevel,
  ProductRuntimeSnapshot,
  WorkerCapabilitySnapshot,
} from "./runtimeProductTypes";

export type ProductMeetingAction = "start" | "stop";

export type ProductMeetingActionResult = {
  ok: boolean;
  action: ProductMeetingAction;
  state: string;
  message: string;
  meeting: ProductMeetingState;
  status: MeetingSessionActionResult["status"];
};

async function loadApprovedVoiceReady(): Promise<boolean | null> {
  try {
    const build = await myVoiceBuildApi.getStatus();
    if (build.phase === "unavailable") return null;
    return build.approved_voice_ready;
  } catch {
    return null;
  }
}

const WORKER_REUSE_SAFE_REASONS = new Set([
  "stop_meeting",
  "start_mic_test",
  "stop_mic_test",
  "select_audio_device",
  "save_settings",
  "apply_meeting_preset",
  "start_voice_recording",
  "stop_voice_recording",
  "start_voice_build",
  "cancel_voice_build",
  "voice_build_completed",
  "voice_build_cancelled",
  "voice_build_failed",
]);

const VOICE_REUSE_SAFE_REASONS = new Set([
  "start_meeting",
  "stop_meeting",
  "start_mic_test",
  "stop_mic_test",
  "fix_setup",
  "check_readiness",
  "ensure_runtime_ready",
  "select_audio_device",
  "save_settings",
  "apply_meeting_preset",
  "start_voice_recording",
  "stop_voice_recording",
]);

async function mapApplicationSnapshotToProduct(
  initialApplication: ApplicationSnapshot,
  knownSettings?: RuntimeSettings,
  allowRuntimeBootstrap = false,
  previous?: ProductRuntimeSnapshot | null,
  reason?: string,
): Promise<ProductRuntimeSnapshot> {
  let application = initialApplication;
  if (application.revision <= 0 || application.lifecycle === "unavailable") {
    throw new Error("TranslateIT application runtime is unavailable.");
  }

  const settings = knownSettings ?? application.settings;
  if (!settings) throw new Error("TranslateIT settings are unavailable.");

  if (
    allowRuntimeBootstrap
    && settings.meeting_setup_state !== "new"
    && application.summaries.worker.state === "not_started"
  ) {
    application = (await applicationRuntimeApi.dispatchIntent("ensure_runtime_ready")).snapshot;
  }

  const helper = application.helper;
  const helperIdentityChanged = Boolean(
    previous
      && (
        previous.helper?.generation_token !== helper.generation_token
        || previous.helper?.state !== helper.state
      ),
  );
  const refreshWorker = !previous
    || helperIdentityChanged
    || !WORKER_REUSE_SAFE_REASONS.has(reason ?? "");
  const refreshVoice = !previous
    || previous.voiceBuildActive !== application.summaries.voice.build_active
    || !VOICE_REUSE_SAFE_REASONS.has(reason ?? "");

  const [approvedVoiceReady, workerStatus] = await Promise.all([
    refreshVoice
      ? loadApprovedVoiceReady()
      : Promise.resolve(previous.readiness.approvedVoiceReady),
    refreshWorker
      ? (helper.state === "ready" ? runtimeApi.helperBridgeWorkerStatus() : Promise.resolve(null))
      : Promise.resolve(previous.workerStatus),
  ]);
  const meetingSession = application.meeting;
  const inputStatus = application.input;
  const meeting = mapProductMeetingState(meetingSession);
  const readiness = mapProductReadiness({
    settings,
    helper,
    workerStatus,
    inputStatus,
    meetingSession,
    approvedVoiceReady,
  });

  return {
    applicationRevision: application.revision,
    settings,
    readiness,
    meeting,
    meetingSession,
    helper,
    workerStatus,
    inputStatus,
    resources: application.resources,
    voiceBuildActive: application.summaries.voice.build_active,
  };
}

export async function loadProductRuntimeSnapshot(
  knownSettings?: RuntimeSettings,
): Promise<ProductRuntimeSnapshot> {
  return mapApplicationSnapshotToProduct(
    await applicationRuntimeApi.getSnapshot(),
    knownSettings,
    true,
  );
}

export async function loadProductRuntimeSnapshotFromApplication(
  application: ApplicationSnapshot,
  previous: ProductRuntimeSnapshot | null,
  reason: string,
  knownSettings?: RuntimeSettings,
): Promise<ProductRuntimeSnapshot> {
  return mapApplicationSnapshotToProduct(
    application,
    knownSettings,
    false,
    previous,
    reason,
  );
}

export async function runProductMeetingAction(
  action: ProductMeetingAction,
): Promise<ProductMeetingActionResult> {
  const result = await applicationRuntimeApi.dispatchIntent(
    action === "start" ? "start_meeting" : "stop_meeting",
  );
  const status = result.snapshot.meeting;
  const fallbackMessage = action === "start"
    ? "Start Translation finished."
    : "Stop Translation finished.";

  return {
    ok: Boolean(result.ok),
    action,
    state: compact(result.state, result.ok ? "completed" : "blocked"),
    message: compact(result.message, fallbackMessage),
    meeting: mapProductMeetingState(status),
    status,
  };
}

export const runtimeProductFacade = {
  loadProductRuntimeSnapshot,
  loadProductRuntimeSnapshotFromApplication,
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
  mapProductMeetingState,
  mapProductReadiness,
  runProductMeetingAction,
  runProductTranslation,
  runProductTranslationAlternative,
  runProductSetupAction,
  runProductRecoveryAction,
};

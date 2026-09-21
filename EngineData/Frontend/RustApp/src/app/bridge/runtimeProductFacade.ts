import {
  runtimeApi,
  type AudioDeviceProbeReport,
  type MeetingSessionActionResult,
} from "./runtimeApi";
import { applicationRuntimeApi } from "./applicationRuntimeApi";
import { compact, errorMessage } from "../shared/state";
import {
  shouldAutoStartHelper,
  shouldExplicitlyRestartHelper,
} from "../runtime/helperLifecyclePolicy";
import type {
  AudioDeviceListReport,
  HelperBridgeStatus,
  RuntimeSettings,
} from "../shared/types";
import { myVoiceBuildApi } from "./myVoiceBuildApi";
import {
  helperBridgeUnavailable,
  mapProductMeetingState,
  mapProductReadiness,
  parseWorkerCapabilities,
} from "./runtimeProductState";
import type {
  ProductMeetingState,
  ProductRuntimeSnapshot,
} from "./runtimeProductTypes";

export {
  mapProductMeetingState,
  mapProductReadiness,
  meetingBridgeUnavailable,
  parseWorkerCapabilities,
} from "./runtimeProductState";
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

export type ProductTranslationResult = {
  ok: boolean;
  source: string;
  translated: string;
  status: string;
  message: string;
  blocker: string;
  needsReview: boolean;
  reviewHints: string[];
};

export type ProductAudioDeviceKind = "microphone" | "meeting-sound";

export type ProductAudioDeviceProbe = {
  ok: boolean;
  kind: ProductAudioDeviceKind;
  deviceId: string | null;
  deviceName: string;
  message: string;
};

export type ProductAudioDeviceSelectionResult = ProductAudioDeviceProbe & {
  settings: RuntimeSettings;
};

export type ProductSetupAction = "check-readiness" | "verify-models";
export type ProductRecoveryAction = "fix-setup";

async function ensurePostSetupHelperLifecycle(settings: RuntimeSettings): Promise<HelperBridgeStatus> {
  const helper = await runtimeApi.getHelperBridgeStatus();
  if (
    settings.meeting_setup_state === "new" ||
    helperBridgeUnavailable(helper) ||
    !shouldAutoStartHelper(helper.state)
  ) {
    return helper;
  }

  // Normal post-setup product use should not require a manual Check Setup after
  // every app restart. Only known inactive states are restarted automatically.
  await runtimeApi.startHelperBridge();
  return runtimeApi.getHelperBridgeStatus();
}

async function loadApprovedVoiceReady(): Promise<boolean | null> {
  try {
    const build = await myVoiceBuildApi.getStatus();
    if (build.phase === "unavailable") return null;
    return build.approved_voice_ready;
  } catch {
    return null;
  }
}

export async function loadProductRuntimeSnapshot(knownSettings?: RuntimeSettings): Promise<ProductRuntimeSnapshot> {
  const settings = knownSettings ?? await runtimeApi.loadSettings();
  if (!settings) throw new Error("TranslateIT settings are unavailable.");

  // Fresh setup remains Python-free. Input and selected-voice checks are
  // independent of helper startup, so overlap them with the helper lifecycle.
  // Meeting preflight waits for the final helper state so readiness cannot be
  // derived from a stale pre-start helper snapshot.
  const helperPromise = ensurePostSetupHelperLifecycle(settings);
  const inputPromise = runtimeApi.getInputStatus();
  const approvedVoicePromise = loadApprovedVoiceReady();

  const helper = await helperPromise;
  const [meetingSession, workerStatus, inputStatus, approvedVoiceReady] = await Promise.all([
    runtimeApi.getMeetingSessionStatus(),
    helper.state === "ready" ? runtimeApi.helperBridgeWorkerStatus() : Promise.resolve(null),
    inputPromise,
    approvedVoicePromise,
  ]);
  const meeting = mapProductMeetingState(meetingSession);
  const readiness = mapProductReadiness({
    settings,
    helper,
    workerStatus,
    inputStatus,
    meetingSession,
    approvedVoiceReady,
  });
  return { settings, readiness, meeting, meetingSession, helper, workerStatus, inputStatus };
}

export async function runProductMeetingAction(action: ProductMeetingAction): Promise<ProductMeetingActionResult> {
  const result = await applicationRuntimeApi.dispatchIntent(
    action === "start" ? "start_meeting" : "stop_meeting",
  );
  const status = result.snapshot.meeting;
  const fallbackMessage = action === "start" ? "Start Translation finished." : "Stop Translation finished.";
  return {
    ok: Boolean(result.ok),
    action,
    state: compact(result.state, result.ok ? "completed" : "blocked"),
    message: compact(result.message, fallbackMessage),
    meeting: mapProductMeetingState(status),
    status,
  };
}

export async function loadProductAudioDevices(): Promise<AudioDeviceListReport> {
  return runtimeApi.listAudioDevices();
}

export async function probeProductAudioDevice(
  kind: ProductAudioDeviceKind,
  deviceId: string | null,
): Promise<ProductAudioDeviceProbe> {
  const normalizedDeviceId = String(deviceId ?? "").trim() || null;
  if (kind === "microphone") {
    const status = await runtimeApi.probeInputDeviceCandidate(normalizedDeviceId);
    const ok = status.functional_verified === true;
    return {
      ok,
      kind,
      deviceId: normalizedDeviceId,
      deviceName: compact(
        status.selected_device_name ?? normalizedDeviceId,
        normalizedDeviceId ? "Selected microphone" : "Windows Default",
      ),
      message: ok
        ? "Microphone is available."
        : "This microphone can't be used right now. Choose another microphone or Windows Default.",
    };
  }

  const status: AudioDeviceProbeReport = await runtimeApi.probeOutputDeviceCandidate(normalizedDeviceId);
  return {
    ok: Boolean(status.ok),
    kind,
    deviceId: normalizedDeviceId,
    deviceName: compact(
      status.resolved_device_name ?? normalizedDeviceId,
      normalizedDeviceId ? "Selected meeting sound" : "Windows Default",
    ),
    message: status.ok
      ? "Meeting sound is available."
      : "That sound device can't be used right now. Choose another device or Windows Default.",
  };
}

export async function selectProductAudioDevice(
  kind: ProductAudioDeviceKind,
  deviceId: string | null,
  currentSettings: RuntimeSettings,
): Promise<ProductAudioDeviceSelectionResult> {
  const normalizedDeviceId = String(deviceId ?? "").trim() || null;
  const result = await runtimeApi.selectAudioDevice(kind, normalizedDeviceId);
  if (!result) {
    return {
      ok: false,
      kind,
      deviceId: normalizedDeviceId,
      deviceName: normalizedDeviceId ?? "Windows Default",
      message: "Audio settings are unavailable right now. The previous device preference was kept.",
      settings: currentSettings,
    };
  }

  return {
    ok: result.ok,
    kind,
    deviceId: result.device_id,
    deviceName: compact(result.device_name, result.device_id ?? "Windows Default"),
    message: compact(result.message, result.ok ? "Audio device saved." : "Audio device was not changed."),
    settings: result.settings,
  };
}

export async function runProductTranslation(source: string): Promise<ProductTranslationResult> {
  const cleaned = source.trim();
  if (!cleaned) {
    return {
      ok: false,
      source,
      translated: "",
      status: "empty",
      message: "Type or paste something to translate.",
      blocker: "text_translation:empty_input",
      needsReview: false,
      reviewHints: [],
    };
  }
  try {
    const result = await runtimeApi.translateText(cleaned);
    return {
      ok: Boolean(result.ok),
      source: cleaned,
      translated: result.translated_text,
      status: result.state,
      message: result.user_message,
      blocker: result.blocker,
      needsReview: Boolean(result.needs_review),
      reviewHints: Array.isArray(result.review_hints) ? result.review_hints : [],
    };
  } catch (error) {
    return {
      ok: false,
      source: cleaned,
      translated: "",
      status: "frontend_bridge_error",
      message: "Translation is unavailable right now. Try again in a moment.",
      blocker: errorMessage(error),
      needsReview: false,
      reviewHints: [],
    };
  }
}

export async function runProductTranslationAlternative(
  source: string,
  currentTranslation: string,
): Promise<ProductTranslationResult> {
  const cleaned = source.trim();
  const current = currentTranslation.trim();
  if (!cleaned || !current) {
    return {
      ok: false,
      source: cleaned,
      translated: "",
      status: "alternative_unavailable",
      message: "Translate the text first, then request another wording.",
      blocker: "text_translation:alternative_requires_current_translation",
      needsReview: false,
      reviewHints: [],
    };
  }
  try {
    const result = await runtimeApi.translateTextAlternative(cleaned, current);
    return {
      ok: Boolean(result.ok),
      source: cleaned,
      translated: result.translated_text,
      status: result.state,
      message: result.user_message,
      blocker: result.blocker,
      needsReview: Boolean(result.needs_review),
      reviewHints: Array.isArray(result.review_hints) ? result.review_hints : [],
    };
  } catch (error) {
    return {
      ok: false,
      source: cleaned,
      translated: "",
      status: "frontend_bridge_error",
      message: "Another wording is unavailable right now.",
      blocker: errorMessage(error),
      needsReview: false,
      reviewHints: [],
    };
  }
}

export async function runProductSetupAction(action: ProductSetupAction): Promise<string> {
  if (action === "check-readiness") {
    const result = await runtimeApi.verifyRequiredOutboundAiReadiness().catch(() => null);
    return result?.ok
      ? "TranslateIT is ready."
      : "TranslateIT still needs attention. Try Check Again, then open Help if the problem continues.";
  }

  const result = await runtimeApi.verifyModels().catch(() => null);
  const blockers = Array.isArray(result?.blockers) ? result.blockers.join("; ") : "";
  return compact(
    result?.note ?? blockers,
    result?.ok ? "All required AI files are available." : "Some required AI files still need attention.",
  );
}

export async function runProductRecoveryAction(action: ProductRecoveryAction): Promise<string> {
  if (action !== "fix-setup") return "No product recovery action was selected.";

  let helper = await runtimeApi.getHelperBridgeStatus().catch(() => null);
  if (helper && shouldExplicitlyRestartHelper(helper.state)) {
    const started = await runtimeApi.startHelperBridge().catch(() => null);
    if (!started?.ok) return "Setup still needs attention. Try again, then open Help if the problem continues.";
    helper = await runtimeApi.getHelperBridgeStatus().catch(() => null);
  }
  const readiness = helper?.state === "ready"
    ? await runtimeApi.verifyRequiredOutboundAiReadiness().catch(() => null)
    : null;
  const input = await runtimeApi.getInputStatus().catch(() => null);
  const workerStatus = helper?.state === "ready"
    ? await runtimeApi.helperBridgeWorkerStatus().catch(() => null)
    : null;
  const worker = parseWorkerCapabilities(workerStatus);
  const hasProblem = Boolean(
    !helper ||
    helper.state !== "ready" ||
    !readiness?.ok ||
    input?.blocker ||
    (worker.responseAvailable && !worker.translationIdEnReady),
  );

  if (hasProblem) return "Setup still needs attention. Try Check Again, then open Help if needed.";
  return "TranslateIT is ready. Check Meeting again; the TranslateIT microphone may still need attention.";
}

export const runtimeProductFacade = {
  loadProductRuntimeSnapshot,
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

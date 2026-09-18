import {
  runtimeApi,
  type AudioDeviceProbeReport,
  type MeetingSessionActionResult,
} from "./runtimeApi";
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
  const result = action === "start"
    ? await runtimeApi.startMeetingTranslation()
    : await runtimeApi.stopMeetingTranslation();
  const fallbackMessage = action === "start" ? "Start Translation finished." : "Stop Translation finished.";
  return {
    ok: Boolean(result.ok),
    action,
    state: compact(result.state, result.ok ? "completed" : "blocked"),
    message: compact(result.message, fallbackMessage),
    meeting: mapProductMeetingState(result.status),
    status: result.status,
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
      normalizedDeviceId ? "Selected Meeting sound" : "Windows Default",
    ),
    message: status.ok
      ? "Meeting sound is available."
      : "This meeting sound device can't be used right now. Choose another device or Windows Default.",
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
    };
  } catch (error) {
    return {
      ok: false,
      source: cleaned,
      translated: "",
      status: "frontend_bridge_error",
      message: "Translation is unavailable right now. Try again or check Diagnostics.",
      blocker: errorMessage(error),
    };
  }
}

export async function runProductSetupAction(action: ProductSetupAction): Promise<string> {
  if (action === "check-readiness") {
    const result = await runtimeApi.verifyRequiredOutboundAiReadiness().catch(() => null);
    return result?.ok
      ? "The final local translation check passed."
      : "The final local translation check still needs attention. Open Diagnostics if this continues.";
  }

  const result = await runtimeApi.verifyModels().catch(() => null);
  const blockers = Array.isArray(result?.blockers) ? result.blockers.join("; ") : "";
  return compact(
    result?.note ?? blockers,
    result?.ok ? "Full product release asset inventory is complete." : "Release asset inventory inspection finished with blockers.",
  );
}

export async function runProductRecoveryAction(action: ProductRecoveryAction): Promise<string> {
  if (action !== "fix-setup") return "No product recovery action was selected.";

  let helper = await runtimeApi.getHelperBridgeStatus().catch(() => null);
  if (helper && shouldExplicitlyRestartHelper(helper.state)) {
    const started = await runtimeApi.startHelperBridge().catch(() => null);
    if (!started?.ok) return "Setup still needs attention. Open Diagnostics for technical details.";
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

  if (hasProblem) return "Setup still needs attention. Open Diagnostics for technical details.";
  return "The local translation check passed. Check Meeting again; the Meeting microphone may still need attention.";
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
  runProductSetupAction,
  runProductRecoveryAction,
};

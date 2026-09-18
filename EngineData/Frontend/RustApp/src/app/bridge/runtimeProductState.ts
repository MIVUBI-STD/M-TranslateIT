import type { MeetingSessionStatus } from "./runtimeApi";
import { resolveMeetingVoiceGate } from "../runtime/meetingVoiceGate";
import { compact, defaultSettings } from "../shared/state";
import { APPLICATION_MEETING_OWNER_ID } from "../shared/types";
import type {
  HelperBridgeStatus,
  HelperBridgeWorkerResponse,
  InputPreparationStatus,
  RuntimeSettings,
} from "../shared/types";

import { parseWorkerCapabilities } from "./workerCapabilities";
import type {
  ProductMeetingState,
  ProductReadiness,
  ProductReadinessLevel,
  WorkerCapabilitySnapshot,
} from "./runtimeProductTypes";

export { parseWorkerCapabilities } from "./workerCapabilities";

type MeetingPreflightSnapshot = {
  readyForStart: boolean;
  startEligible: boolean;
  functionalOutboundReady: boolean;
  microphoneReady: boolean;
  modelsReady: boolean;
  helperReady: boolean;
  providerReady: boolean;
  meetingRouteReady: boolean;
  blockers: string[];
  summary: string;
};

type TranslationDirection = "id->en" | "en->id" | "unsupported";

const FRONTEND_BRIDGE_UNAVAILABLE = "frontend_bridge_unavailable";

function unique(values: Array<string | null | undefined>): string[] {
  return Array.from(new Set(values.map((value) => compact(value, "")).filter(Boolean)));
}

function normalizeProductLanguage(value: unknown): string {
  const text = String(value ?? "").trim().toLowerCase().replace("_latn", "");
  if (text === "id" || text.startsWith("ind")) return "id";
  if (text === "en" || text.startsWith("eng")) return "en";
  return text;
}

function selectedTextDirection(settings: RuntimeSettings): TranslationDirection {
  const source = normalizeProductLanguage(settings.source_language);
  const target = normalizeProductLanguage(settings.target_language);
  if (source === "id" && target === "en") return "id->en";
  if (source === "en" && target === "id") return "en->id";
  return "unsupported";
}

function directionLabel(direction: TranslationDirection): string {
  if (direction === "id->en") return "Indonesian → English";
  if (direction === "en->id") return "English → Indonesian";
  return "Selected";
}

export function helperBridgeUnavailable(helper: HelperBridgeStatus | null): boolean {
  return helper?.runtime_claim === FRONTEND_BRIDGE_UNAVAILABLE || helper?.state === "frontend_bridge_error";
}

export function meetingBridgeUnavailable(status: MeetingSessionStatus | null): boolean {
  return status?.runtime_claim === FRONTEND_BRIDGE_UNAVAILABLE || status?.lifecycle === "unavailable";
}

function inputBridgeUnavailable(status: InputPreparationStatus | null): boolean {
  return status?.blocker === FRONTEND_BRIDGE_UNAVAILABLE;
}

function meetingPreflight(meetingSession: MeetingSessionStatus | null): MeetingPreflightSnapshot {
  const preflight = meetingSession?.preflight ?? null;
  if (!preflight) {
    return {
      readyForStart: false,
      startEligible: false,
      functionalOutboundReady: false,
      microphoneReady: false,
      modelsReady: false,
      helperReady: false,
      providerReady: false,
      meetingRouteReady: false,
      blockers: [],
      summary: "Meeting preflight has not been checked yet.",
    };
  }

  return {
    readyForStart: preflight.ready_for_start === true,
    startEligible: preflight.start_eligible === true,
    functionalOutboundReady: preflight.functional_outbound_ready === true,
    microphoneReady: preflight.microphone_ready === true,
    modelsReady: preflight.models_ready === true,
    helperReady: preflight.helper_ready === true,
    providerReady: preflight.provider_ready === true,
    meetingRouteReady: preflight.meeting_route_ready === true,
    blockers: Array.isArray(preflight.blockers) ? preflight.blockers.map(String) : [],
    summary: compact(preflight.summary, "Meeting preflight checked."),
  };
}

function liveMeetingMessage(stage: string): string {
  if (stage === "transcribing" || stage === "translating" || stage === "synthesizing") {
    return "Translation is live and preparing the English voice for your meeting.";
  }
  if (stage === "delivering") return "Translation is live and speaking English to your meeting.";
  if (stage === "attention_needed") {
    return "Translation is live, but the latest phrase needs attention. Check Diagnostics if this continues.";
  }
  return "Translation is live and listening for your next phrase.";
}

export function mapProductMeetingState(status: MeetingSessionStatus | null): ProductMeetingState {
  const preflight = meetingPreflight(status);
  const unavailable = meetingBridgeUnavailable(status);
  const hasSession = status?.has_session === true;
  const applicationOwned = hasSession && status?.owner_id === APPLICATION_MEETING_OWNER_ID;
  const authorityActive = applicationOwned && status?.authority_active === true;
  const rawLifecycle = unavailable ? "unavailable" : compact(status?.lifecycle, hasSession ? "active" : "idle");
  const lifecycle = unavailable ? "unavailable" : applicationOwned ? rawLifecycle : hasSession ? "runtime_conflict" : "idle";
  const live = applicationOwned && authorityActive && lifecycle === "live";
  const starting = applicationOwned && authorityActive && lifecycle === "starting";
  const stopping = applicationOwned && lifecycle === "stopping";
  const busy = starting || stopping;
  const canStart = !unavailable && !hasSession && preflight.startEligible;
  const canStop = !unavailable && applicationOwned && hasSession && !starting && !stopping;
  const outboundStage = compact(status?.outbound?.stage, unavailable ? "unavailable" : "idle");
  const blocker = compact(
    status?.blocker || preflight.blockers[0],
    unavailable
      ? FRONTEND_BRIDGE_UNAVAILABLE
      : hasSession && !applicationOwned
        ? "meeting_session:active_runtime_conflict"
        : "",
  );

  let label = "Setup Needed";
  let message = "Meeting setup needs attention.";
  if (unavailable) {
    label = "Unavailable";
    message = "Meeting status is unavailable. Try the check again when TranslateIT is available.";
  } else if (live) {
    label = "Live";
    message = liveMeetingMessage(outboundStage);
  } else if (starting) {
    label = "Starting";
    message = "Translation is starting.";
  } else if (stopping) {
    label = "Stopping";
    message = "Translation is stopping safely.";
  } else if (applicationOwned && lifecycle === "cleanup_incomplete") {
    label = "Stop Needed";
    message = "Translation output is stopped, but cleanup still needs attention. Try Stop again.";
  } else if (hasSession && !applicationOwned) {
    label = "In Use";
    message = "Meeting audio is already in use. Finish that operation before starting Translation.";
  } else if (applicationOwned && hasSession) {
    label = "Active";
    message = "Meeting translation is still active. Stop it before starting a new session.";
  } else if (canStart) {
    label = "Ready";
    message = "Ready to translate. Start when your meeting is open.";
  }

  return {
    lifecycle,
    hasSession,
    applicationOwned,
    authorityActive,
    captureActive: applicationOwned && status?.capture_active === true,
    live,
    busy,
    canStart,
    canStop,
    sessionId: applicationOwned ? status?.session_id ?? null : null,
    generation: applicationOwned ? status?.generation ?? null : null,
    outboundStage,
    label,
    message,
    blocker,
  };
}

function collectBlockers(input: {
  helper: HelperBridgeStatus | null;
  worker: WorkerCapabilitySnapshot;
  inputStatus: InputPreparationStatus | null;
  meeting: MeetingPreflightSnapshot;
  textReady: boolean;
  textDirection: TranslationDirection;
  meetingReady: boolean;
  voiceBlocker: string | null;
}): string[] {
  const { helper, worker, inputStatus, meeting, textReady, textDirection, meetingReady, voiceBlocker } = input;
  return unique([
    ...(!meetingReady ? meeting.blockers : []),
    ...(!meetingReady && voiceBlocker ? [voiceBlocker] : []),
    ...(!textReady && worker.blocker ? [worker.blocker] : []),
    ...(!textReady && textDirection !== "unsupported" ? ["text_translation:selected_direction_not_ready"] : []),
    ...(textDirection === "unsupported" ? ["text_translation:unsupported_direction"] : []),
    helper?.state !== "ready" ? helper?.last_error ?? null : null,
    inputStatus?.blocker ?? null,
  ]).slice(0, 8);
}

export function mapProductReadiness(input: {
  settings?: RuntimeSettings | null;
  helper: HelperBridgeStatus | null;
  workerStatus?: HelperBridgeWorkerResponse | null;
  inputStatus: InputPreparationStatus | null;
  meetingSession?: MeetingSessionStatus | null;
  approvedVoiceReady?: boolean | null;
}): ProductReadiness {
  const helper = input.helper;
  const inputStatus = input.inputStatus;
  const settings = input.settings ?? defaultSettings();
  const worker = parseWorkerCapabilities(input.workerStatus ?? null);
  const meeting = meetingPreflight(input.meetingSession ?? null);
  const productMeeting = mapProductMeetingState(input.meetingSession ?? null);

  const helperUnavailable = helperBridgeUnavailable(helper);
  const meetingUnavailable = meetingBridgeUnavailable(input.meetingSession ?? null);
  const inputUnavailable = inputBridgeUnavailable(inputStatus);
  const runtimeUnavailable = helperUnavailable && meetingUnavailable && inputUnavailable;

  const workerHelperReady = helper?.state === "ready";
  const helperReady = meeting.helperReady;
  const microphoneReady = meeting.microphoneReady;
  const asrReady = workerHelperReady && worker.asrReady;
  const translationIdEnReady = workerHelperReady && worker.translationIdEnReady;
  const translationEnIdReady = workerHelperReady && worker.translationEnIdReady;
  const ttsReady = workerHelperReady && worker.ttsReady;
  const providerReady = meeting.providerReady;
  const modelsReady = meeting.modelsReady;
  const functionalOutboundReady = meeting.functionalOutboundReady;

  const textDirection = selectedTextDirection(settings);
  const textReady = textDirection === "id->en"
    ? translationIdEnReady
    : textDirection === "en->id"
      ? translationEnIdReady
      : false;
  const canTranslateText = textReady;

  const meetingRouteReady = meeting.meetingRouteReady;
  const approvedVoiceReady = typeof input.approvedVoiceReady === "boolean" ? input.approvedVoiceReady : null;
  const voiceGate = resolveMeetingVoiceGate({
    live: productMeeting.live,
    preflightReady: meeting.readyForStart,
    selectedVoiceReady: approvedVoiceReady,
  });
  const meetingReady = voiceGate.meetingReady;
  const recording = productMeeting.captureActive;
  const blockers = collectBlockers({
    helper,
    worker,
    inputStatus,
    meeting,
    textReady,
    textDirection,
    meetingReady,
    voiceBlocker: voiceGate.blocker,
  });

  const hasRuntimeEvidence = Boolean(helper || worker.responseAvailable || inputStatus || input.meetingSession);
  const level: ProductReadinessLevel = runtimeUnavailable
    ? "unavailable"
    : meetingReady
      ? "ready"
      : textReady
        ? "partial"
        : hasRuntimeEvidence
          ? "blocked"
          : "checking";
  const textDirectionLabel = directionLabel(textDirection);
  const nextAction = runtimeUnavailable
    ? "TranslateIT is unavailable right now. Try the status check again before using translation."
    : productMeeting.live
      ? "Translation is live. Stop the Meeting session when you are finished."
      : voiceGate.nextAction
        ? voiceGate.nextAction
        : meeting.readyForStart
          ? "Meeting Translation is ready to start."
          : productMeeting.canStart
            ? "Start Translation will run a quick final translation check before going live."
            : textReady
              ? "Text translation is available. Meeting setup still needs attention."
              : textDirection === "unsupported"
                ? "Choose Indonesian → English or English → Indonesian for Text translation."
                : "The selected Text translation direction is not ready. Check Setup or Diagnostics if needed.";
  const summary = runtimeUnavailable
    ? "TranslateIT is unavailable right now. Try the status check again."
    : productMeeting.live
      ? "Meeting Translation is live."
      : voiceGate.summary
        ? voiceGate.summary
        : meeting.readyForStart
          ? "Meeting Translation is ready."
          : productMeeting.canStart
            ? "Meeting setup is available; the final local translation check has not passed for this helper session yet."
            : textReady
              ? `${textDirectionLabel} Text translation is available. Meeting Translation is not ready yet.`
              : hasRuntimeEvidence
                ? `${textDirectionLabel} Text translation is not ready. Meeting Translation is not ready yet.`
                : "Product readiness is still checking.";

  return {
    level,
    textReady,
    helperReady,
    microphoneReady,
    asrReady,
    translationIdEnReady,
    translationEnIdReady,
    ttsReady,
    meetingRouteReady,
    meetingReady,
    approvedVoiceReady,
    canTranslateText,
    recording,
    nextAction,
    blockers,
    summary,
    textStatus: helperUnavailable
      ? "Unavailable"
      : textReady
        ? "Ready"
        : level === "checking"
          ? "Checking"
          : "Setup Needed",
    helperStatus: helperUnavailable
      ? "Unavailable"
      : helperReady
        ? worker.responseAvailable
          ? "Local worker running"
          : "Worker running; capability check unavailable"
        : compact(helper?.state ?? helper?.message, "Local worker not running"),
    modelStatus: helperUnavailable
      ? "Unavailable"
      : modelsReady && functionalOutboundReady
        ? "Required outbound translation check passed"
        : modelsReady
          ? "Final local translation check pending"
          : worker.responseAvailable
            ? "Required outbound model runtime needs setup"
            : "Worker capability not checked",
    microphoneStatus: inputUnavailable && meetingUnavailable
      ? "Unavailable"
      : microphoneReady
        ? compact(inputStatus?.selected_device_name, "Microphone ready")
        : "Setup Needed",
    voiceStatus: runtimeUnavailable
      ? "Unavailable"
      : microphoneReady && providerReady
        ? "Required local outbound AI capabilities available"
        : "Local voice runtime needs setup",
    meetingStatus: productMeeting.label === "Unavailable"
      ? "Unavailable"
      : productMeeting.live
        ? "Live"
        : productMeeting.busy
          ? productMeeting.label
          : meeting.readyForStart
            ? voiceGate.status
            : level === "checking"
              ? "Checking"
              : "Setup Needed",
    runtimeStatus: runtimeUnavailable
      ? "Unavailable"
      : productMeeting.lifecycle !== "idle"
        ? productMeeting.lifecycle
        : compact(helper?.state, "Checking"),
  };
}

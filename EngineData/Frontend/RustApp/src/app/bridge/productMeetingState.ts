import type { MeetingSessionStatus } from "./runtimeApi";
import { compact } from "../shared/state";
import { APPLICATION_MEETING_OWNER_ID } from "../shared/types";
import type { ProductMeetingState } from "./runtimeProductTypes";

export type MeetingPreflightSnapshot = {
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

const FRONTEND_BRIDGE_UNAVAILABLE = "frontend_bridge_unavailable";

export function meetingBridgeUnavailable(status: MeetingSessionStatus | null): boolean {
  return status?.runtime_claim === FRONTEND_BRIDGE_UNAVAILABLE || status?.lifecycle === "unavailable";
}

export function meetingPreflight(meetingSession: MeetingSessionStatus | null): MeetingPreflightSnapshot {
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

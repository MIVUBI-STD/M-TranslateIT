import type { ApplicationInputStatus, ResourceArbitration } from "./applicationRuntimeApi";
import type { MeetingSessionStatus } from "./runtimeApi";
import type {
  HelperBridgeStatus,
  HelperBridgeWorkerResponse,
  RuntimeSettings,
} from "../shared/types";

export type ProductReadinessLevel = "ready" | "partial" | "blocked" | "checking" | "unavailable";

export type ProductReadiness = {
  level: ProductReadinessLevel;
  textReady: boolean;
  helperReady: boolean;
  microphoneReady: boolean;
  asrReady: boolean;
  translationIdEnReady: boolean;
  translationEnIdReady: boolean;
  ttsReady: boolean;
  meetingRouteReady: boolean;
  meetingReady: boolean;
  approvedVoiceReady: boolean | null;
  canTranslateText: boolean;
  recording: boolean;
  nextAction: string;
  blockers: string[];
  summary: string;
  textStatus: string;
  helperStatus: string;
  modelStatus: string;
  microphoneStatus: string;
  voiceStatus: string;
  meetingStatus: string;
  runtimeStatus: string;
};

export type ProductMeetingState = {
  lifecycle: string;
  hasSession: boolean;
  applicationOwned: boolean;
  authorityActive: boolean;
  captureActive: boolean;
  live: boolean;
  busy: boolean;
  canStart: boolean;
  canStop: boolean;
  sessionId: string | null;
  generation: number | null;
  outboundStage: string;
  label: string;
  message: string;
  blocker: string;
};

export type ProductRuntimeSnapshot = {
  settings: RuntimeSettings;
  readiness: ProductReadiness;
  meeting: ProductMeetingState;
  meetingSession: MeetingSessionStatus | null;
  helper: HelperBridgeStatus | null;
  workerStatus: HelperBridgeWorkerResponse | null;
  inputStatus: ApplicationInputStatus | null;
  resources: ResourceArbitration;
  voiceBuildActive: boolean;
};

export type WorkerCapabilitySnapshot = {
  responseAvailable: boolean;
  asrReady: boolean;
  translationIdEnReady: boolean;
  translationEnIdReady: boolean;
  ttsReady: boolean;
  blocker: string;
  note: string;
  asrDisplay: string;
  translationDisplay: string;
  voiceDisplay: string;
  executionDisplay: string;
};

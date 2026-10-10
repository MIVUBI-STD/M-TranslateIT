import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { runCommand } from "../shared/tauriBridge";
import { meetingSessionStatusFallback } from "../runtime/meetingBridgeFallback";
import { defaultSettings } from "../shared/state";
import type {
  HelperBridgeStatus,
  RuntimeSettings,
} from "../shared/types";
import type { MeetingSessionStatus } from "./runtimeApi";

export type ProductIntent =
  | "start_meeting"
  | "stop_meeting"
  | "start_mic_test"
  | "stop_mic_test"
  | "fix_setup"
  | "check_readiness"
  | "ensure_runtime_ready";

export type ApplicationProblem = {
  code: string;
  domain: string;
  severity: "warning" | "blocking" | string;
  recoverable: boolean;
  action: string;
  message: string;
};

export type ApplicationCapabilities = {
  meeting_translation: boolean;
  mic_test: boolean;
};

export type MeetingSummary = {
  lifecycle: string;
  has_session: boolean;
  application_owned: boolean;
  ready_for_start: boolean;
  owner_id: string | null;
  blocker: string;
  note: string;
  preflight_blockers: string[];
};

export type WorkerSummary = {
  state: string;
  process_ready: boolean;
  degraded: boolean;
  message: string;
};

export type AudioSummary = {
  ready: boolean;
  running: boolean;
  blocker: string;
  note: string;
};

export type ApplicationInputStatus = {
  ready: boolean;
  prepared: boolean;
  functional_verified: boolean;
  callback_frames_observed: number;
  selected_device_name: string | null;
  input_device_name: string | null;
  blocker: string;
  note: string;
};

export type VoiceSummary = {
  recording_active: boolean;
  build_active: boolean;
  build_phase: string;
};

export type ApplicationSubsystemSummaries = {
  meeting: MeetingSummary;
  worker: WorkerSummary;
  audio: AudioSummary;
  voice: VoiceSummary;
};

export type ResourceArbitration = {
  audio_locked: boolean;
  microphone_available: boolean;
  active_owner: string | null;
  owner_kind: "meeting" | "mic_test" | "voice_recording" | "other" | "none" | string;
  blocker: string;
};

export type ApplicationSnapshot = {
  // Monotonic snapshot sequence; not a semantic state-change revision.
  revision: number;
  lifecycle: string;
  active_owner: string | null;
  settings: RuntimeSettings;
  meeting: MeetingSessionStatus;
  helper: HelperBridgeStatus;
  input: ApplicationInputStatus;
  summaries: ApplicationSubsystemSummaries;
  resources: ResourceArbitration;
  capabilities: ApplicationCapabilities;
  problems: ApplicationProblem[];
};

export type ApplicationIntentResult = {
  ok: boolean;
  intent: ProductIntent | string;
  state: string;
  message: string;
  snapshot: ApplicationSnapshot;
};

export type ApplicationRuntimeEvent = {
  reason: string;
  snapshot: ApplicationSnapshot;
};

type VoiceBuildRuntimeEvent = {
  revision: number;
  reason: string;
  generation: number;
};

function unavailableSnapshot(): ApplicationSnapshot {
  return {
    revision: 0,
    lifecycle: "unavailable",
    active_owner: null,
    // These are safe placeholders, never evidence of a saved user configuration.
    settings: defaultSettings(),
    // The bridge has not proven that a prior Meeting session is idle.
    // Reuse the existing fail-closed, fully shaped Meeting status contract.
    meeting: meetingSessionStatusFallback("Application runtime is unavailable."),
    // Fail closed: no helper readiness or owned process is implied.
    helper: {
      state: "frontend_bridge_error",
      message: "Application runtime is unavailable.",
      cuda_ready: false,
      provider_ready: false,
      functional_outbound_ready: false,
      functional_outbound_verified_unix_ms: null,
      degraded_mode: false,
      active_task: null,
      active_request_id: null,
      active_meeting_generation: null,
      active_meeting_session_id: null,
      active_meeting_lane: null,
      generation_token: 0,
      last_error: "frontend_bridge_unavailable",
      stderr_log_path: null,
      updated_unix_ms: Date.now(),
      runtime_claim: "frontend_bridge_unavailable",
    },
    input: {
      ready: false,
      prepared: false,
      functional_verified: false,
      callback_frames_observed: 0,
      selected_device_name: null,
      input_device_name: null,
      blocker: "application_runtime:unavailable",
      note: "Application runtime is unavailable.",
    },
    summaries: {
      meeting: {
        lifecycle: "unavailable",
        has_session: false,
        application_owned: false,
        ready_for_start: false,
        owner_id: null,
        blocker: "application_runtime:unavailable",
        note: "Application runtime is unavailable.",
        preflight_blockers: [],
      },
      worker: {
        state: "unavailable",
        process_ready: false,
        degraded: false,
        message: "Application runtime is unavailable.",
      },
      audio: {
        ready: false,
        running: false,
        blocker: "application_runtime:unavailable",
        note: "Application runtime is unavailable.",
      },
      voice: {
        recording_active: false,
        build_active: false,
        build_phase: "unavailable",
      },
    },
    resources: {
      audio_locked: true,
      microphone_available: false,
      active_owner: null,
      owner_kind: "other",
      blocker: "application_runtime:unavailable",
    },
    capabilities: {
      meeting_translation: false,
      mic_test: false,
    },
    problems: [{
      code: "application_runtime:unavailable",
      domain: "runtime",
      severity: "blocking",
      recoverable: true,
      action: "refresh",
      message: "Application runtime is unavailable.",
    }],
  };
}

export async function getApplicationSnapshot(): Promise<ApplicationSnapshot> {
  return await runCommand<ApplicationSnapshot>("get_application_snapshot") ?? unavailableSnapshot();
}

export async function dispatchProductIntent(intent: ProductIntent): Promise<ApplicationIntentResult> {
  const result = await runCommand<ApplicationIntentResult>("dispatch_product_intent", { intent });
  if (result) return result;

  return {
    ok: false,
    intent,
    state: "frontend_bridge_error",
    message: "The product action could not reach the application runtime.",
    snapshot: unavailableSnapshot(),
  };
}

export async function subscribeApplicationRuntime(
  listener: (event: ApplicationRuntimeEvent) => void,
): Promise<UnlistenFn> {
  let disposed = false;
  const applicationStop = await listen<ApplicationRuntimeEvent>(
    "translateit://application-runtime",
    (event) => {
      if (!disposed) listener(event.payload);
    },
  );
  let voiceBuildStop: UnlistenFn;
  try {
    voiceBuildStop = await listen<VoiceBuildRuntimeEvent>(
      "translateit://voice-build-runtime",
      (event) => {
        void getApplicationSnapshot().then((snapshot) => {
          if (disposed) return;
          if (snapshot.revision <= 0 || snapshot.lifecycle === "unavailable") return;
          listener({ reason: event.payload.reason, snapshot });
        });
      },
    );
  } catch (error) {
    disposed = true;
    applicationStop();
    throw error;
  }

  return () => {
    disposed = true;
    applicationStop();
    voiceBuildStop();
  };
}

export const applicationRuntimeApi = {
  getSnapshot: getApplicationSnapshot,
  dispatchIntent: dispatchProductIntent,
  subscribe: subscribeApplicationRuntime,
};

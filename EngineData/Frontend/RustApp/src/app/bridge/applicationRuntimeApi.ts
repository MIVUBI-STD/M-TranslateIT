import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { runCommand } from "../shared/tauriBridge";
import type {
  HelperBridgeStatus,
  HelperBridgeWorkerResponse,
  InputPreparationStatus,
  RuntimeSettings,
} from "../shared/types";
import type { MeetingSessionStatus } from "./runtimeApi";

export type ProductIntent =
  | "start_meeting"
  | "stop_meeting"
  | "start_mic_test"
  | "stop_mic_test"
  | "refresh";

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
  text_translation: boolean;
};

export type ApplicationSnapshot = {
  revision: number;
  lifecycle: string;
  active_owner: string | null;
  settings: RuntimeSettings;
  meeting: MeetingSessionStatus;
  helper: HelperBridgeStatus;
  worker: HelperBridgeWorkerResponse | null;
  input: InputPreparationStatus;
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

function unavailableSnapshot(): ApplicationSnapshot {
  return {
    revision: 0,
    lifecycle: "unavailable",
    active_owner: null,
    settings: {} as RuntimeSettings,
    meeting: {} as MeetingSessionStatus,
    helper: {} as HelperBridgeStatus,
    worker: null,
    input: {
      ready: false,
      selected_device_name: null,
      device_count: 0,
      blocker: "application_runtime:unavailable",
      note: "Application runtime is unavailable.",
    },
    capabilities: {
      meeting_translation: false,
      mic_test: false,
      text_translation: false,
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
  return listen<ApplicationRuntimeEvent>("translateit://application-runtime", (event) => {
    listener(event.payload);
  });
}

export const applicationRuntimeApi = {
  getSnapshot: getApplicationSnapshot,
  dispatchIntent: dispatchProductIntent,
  subscribe: subscribeApplicationRuntime,
};

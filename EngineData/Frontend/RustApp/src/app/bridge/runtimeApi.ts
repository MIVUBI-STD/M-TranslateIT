import { getRuntimeCommandErrors, runCommand } from "../shared/tauriBridge";
import { meetingSessionStatusFallback } from "../runtime/meetingBridgeFallback";
import type {
  AudioDeviceListReport,
  CommandResult,
  HelperBridgeActionResult,
  HelperBridgeStatus,
  HelperBridgeWorkerResponse,
  InputPreparationStatus,
  ModelInventoryReport,
  RuntimeCommandError,
  RuntimeSettings,
} from "../shared/types";

export type AudioDeviceProbeReport = {
  ok: boolean;
  device_kind: string;
  requested_device_id: string | null;
  resolved_device_name: string | null;
  is_default: boolean;
  sample_rate_hz: number | null;
  channels: number | null;
  blocker: string;
  note: string;
};

export type AudioDeviceSelectionCommandResult = {
  ok: boolean;
  kind: string;
  device_id: string | null;
  device_name: string;
  message: string;
  settings: RuntimeSettings;
};

export type VirtualMicRouteContractStatus = {
  ok: boolean;
  route_ready: boolean;
  route_pair_id: string | null;
  selected_output_device: string | null;
  selected_input_device: string | null;
  output_device_found: boolean;
  input_device_found: boolean;
  evidence_path: string | null;
  route_output_contract_json: string;
  available_output_devices: string[];
  available_input_devices: string[];
  blocker: string;
  next_action: string;
  runtime_claim: string;
  updated_unix_ms: number;
};

export type TextTranslationCommandResult = {
  ok: boolean;
  state: string;
  translated_text: string;
  user_message: string;
  blocker: string;
  needs_review: boolean;
  review_hints: string[];
};

export type MeetingSessionPreflightStatus = {
  ready_for_start: boolean;
  start_eligible: boolean;
  functional_outbound_ready: boolean;
  functional_outbound_verified_unix_ms: number | null;
  microphone_ready: boolean;
  models_ready: boolean;
  helper_ready: boolean;
  provider_ready: boolean;
  meeting_route_ready: boolean;
  blockers: string[];
  summary: string;
  runtime_claim: string;
};

export type MeetingOutboundTiming = {
  finalized_unix_ms: number;
  first_playback_unix_ms: number | null;
  speech_boundary_ms: number;
  finalization_ms: number;
  queue_ms: number;
  audio_prepare_ms: number;
  asr_ms: number | null;
  translation_ms: number | null;
  translation_tokenization_ms: number | null;
  translation_inference_ms: number | null;
  translation_decode_ms: number | null;
  translation_tokens_per_second: number | null;
  tts_ms: number | null;
  delivery_ms: number | null;
  outbound_latency_ms: number | null;
};

export type MeetingOutboundRuntimeStatus = {
  generation: number | null;
  session_id: string | null;
  stage: string;
  utterance_sequence: number;
  output_active: boolean;
  last_stage_ok: boolean;
  timing: MeetingOutboundTiming | null;
  overflow_dropped_utterance_count: number;
  evicted_pending_utterance_count: number;
  blocker: string;
  note: string;
  updated_unix_ms: number;
  runtime_claim: string;
};

export type MeetingIncomingRuntimeStatus = {
  session_id: string | null;
  stage: string;
  capture_active: boolean;
  suppressed: boolean;
  degraded: boolean;
  blocker: string;
  note: string;
  updated_unix_ms: number;
  runtime_claim: string;
};

export type MeetingSessionStatus = {
  lifecycle: string;
  has_session: boolean;
  authority_active: boolean;
  session_id: string | null;
  generation: number | null;
  started_unix_ms: number | null;
  active_age_ms: number | null;
  capture_active: boolean;
  owner_id: string | null;
  blocker: string;
  note: string;
  preflight: MeetingSessionPreflightStatus;
  outbound: MeetingOutboundRuntimeStatus;
  incoming: MeetingIncomingRuntimeStatus;
  runtime_claim: string;
};

export type MeetingSessionActionResult = {
  ok: boolean;
  state: string;
  message: string;
  status: MeetingSessionStatus;
};

export type MeetingCommittedTurn = {
  session_id: string;
  sequence: number;
  generation: number | null;
  utterance_id: number;
  lane: "you" | "incoming" | string;
  source_text: string;
  translated_text: string;
  delivery_state: "preparing_voice" | "speaking" | "output_complete" | "output_failed" | "interrupted" | string | null;
  outbound_timing: MeetingOutboundTiming | null;
  created_unix_ms: number;
  updated_unix_ms: number;
};

export type MeetingCommittedTurnsSnapshot = {
  ok: boolean;
  has_session: boolean;
  session_id: string | null;
  turns: MeetingCommittedTurn[];
  dropped_turn_count: number;
  truncated: boolean;
  blocker: string;
  note: string;
  runtime_claim: string;
};

const MAX_COMMAND_ERRORS = 12;

type NativeInputPreparationStatus = InputPreparationStatus & {
  backend_id?: string;
  input_device_name?: string | null;
  prepared?: boolean;
  running?: boolean;
  target_sample_rate_hz?: number;
  target_channels?: number;
};

function commandFallback(message: string, state = "frontend_bridge_error"): CommandResult {
  return { ok: false, state, message };
}

function textTranslationFallback(): TextTranslationCommandResult {
  return {
    ok: false,
    state: "frontend_bridge_error",
    translated_text: "",
    user_message: "Translation is unavailable right now. Try again or check Diagnostics.",
    blocker: "frontend_bridge_unavailable",
    needs_review: false,
    review_hints: [],
  };
}

function helperActionFallback(message: string): HelperBridgeActionResult {
  return {
    ok: false,
    state: "frontend_bridge_error",
    message,
    generation_token: 0,
    runtime_claim: "frontend_bridge_unavailable",
  };
}

function helperWorkerFallback(task: string, message: string): HelperBridgeWorkerResponse {
  return {
    ok: false,
    state: "frontend_bridge_error",
    task,
    request_id: "",
    scheduler_priority: "diagnostic",
    message,
    generation_token: 0,
    runtime_claim: "frontend_bridge_unavailable",
    worker_response_json: JSON.stringify({
      ok: false,
      stage: task,
      blocker: "frontend_bridge_unavailable",
      note: message,
    }),
  };
}

function bridgeStatusFallback(message: string): HelperBridgeStatus {
  return {
    state: "frontend_bridge_error",
    message,
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
  };
}

function inputStatusFallback(message: string): InputPreparationStatus {
  return {
    ready: false,
    selected_device_name: null,
    device_count: 0,
    blocker: "frontend_bridge_unavailable",
    note: message,
  };
}

function normalizeInputStatus(status: NativeInputPreparationStatus): InputPreparationStatus {
  return {
    ...status,
    ready: status.ready ?? status.prepared ?? false,
    selected_device_name: status.selected_device_name ?? status.input_device_name ?? null,
    device_count: status.device_count ?? (status.input_device_name ? 1 : 0),
    blocker: status.blocker ?? (status.prepared === false ? "audio_input:not_prepared" : undefined),
    note: status.note ?? "Audio input status checked.",
  };
}

function audioDevicesFallback(message: string): AudioDeviceListReport {
  return {
    ok: false,
    input_devices: [],
    output_devices: [],
    blocker: "frontend_bridge_unavailable",
    note: message,
  };
}

function audioDeviceProbeFallback(message: string): AudioDeviceProbeReport {
  return {
    ok: false,
    device_kind: "meeting_sound",
    requested_device_id: null,
    resolved_device_name: null,
    is_default: false,
    sample_rate_hz: null,
    channels: null,
    blocker: "frontend_bridge_unavailable",
    note: message,
  };
}

function virtualMicRouteFallback(): VirtualMicRouteContractStatus {
  return {
    ok: false,
    route_ready: false,
    route_pair_id: null,
    selected_output_device: null,
    selected_input_device: null,
    output_device_found: false,
    input_device_found: false,
    evidence_path: null,
    route_output_contract_json: "",
    available_output_devices: [],
    available_input_devices: [],
    blocker: "frontend_bridge_unavailable",
    next_action: "retry_route_status",
    runtime_claim: "frontend_bridge_unavailable",
    updated_unix_ms: Date.now(),
  };
}

function meetingSessionActionFallback(message: string): MeetingSessionActionResult {
  return {
    ok: false,
    state: "frontend_bridge_error",
    message,
    status: meetingSessionStatusFallback(message),
  };
}

function meetingCommittedTurnsFallback(message: string): MeetingCommittedTurnsSnapshot {
  return {
    ok: false,
    has_session: false,
    session_id: null,
    turns: [],
    dropped_turn_count: 0,
    truncated: false,
    blocker: "frontend_bridge_unavailable",
    note: message,
    runtime_claim: "frontend_bridge_unavailable",
  };
}

async function invokeOr<T>(command: string, args: Record<string, unknown> | undefined, fallback: T): Promise<T> {
  const result = await runCommand<T>(command, args);
  return result ?? fallback;
}

async function invokeNullable<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  return runCommand<T>(command, args);
}

async function loadRuntimeSettings(): Promise<RuntimeSettings | null> {
  return invokeNullable<RuntimeSettings>("load_runtime_settings");
}

export const runtimeApi = {
  getCommandErrors(): RuntimeCommandError[] {
    return getRuntimeCommandErrors().slice(0, MAX_COMMAND_ERRORS);
  },

  async getMeetingSessionStatus(): Promise<MeetingSessionStatus> {
    return invokeOr<MeetingSessionStatus>(
      "get_meeting_session_status",
      undefined,
      meetingSessionStatusFallback("Meeting session status is unavailable because the frontend bridge could not call Tauri."),
    );
  },

  async getMeetingCommittedTurns(): Promise<MeetingCommittedTurnsSnapshot> {
    return invokeOr<MeetingCommittedTurnsSnapshot>(
      "get_meeting_committed_turns",
      undefined,
      meetingCommittedTurnsFallback("Meeting transcript is temporarily unavailable because the frontend bridge could not call Tauri."),
    );
  },

  async startMeetingTranslation(): Promise<MeetingSessionActionResult> {
    return invokeOr<MeetingSessionActionResult>(
      "start_meeting_translation",
      undefined,
      meetingSessionActionFallback("Start Translation failed before reaching the Tauri Meeting session command."),
    );
  },

  async stopMeetingTranslation(): Promise<MeetingSessionActionResult> {
    return invokeOr<MeetingSessionActionResult>(
      "stop_meeting_translation",
      undefined,
      meetingSessionActionFallback("Stop Translation failed before reaching the Tauri Meeting session command."),
    );
  },

  async getVirtualMicRouteStatus(): Promise<VirtualMicRouteContractStatus> {
    return invokeOr<VirtualMicRouteContractStatus>(
      "get_virtual_mic_route_contract_status",
      undefined,
      virtualMicRouteFallback(),
    );
  },

  async getHelperBridgeStatus(): Promise<HelperBridgeStatus> {
    return invokeOr<HelperBridgeStatus>(
      "get_helper_bridge_status",
      undefined,
      bridgeStatusFallback("Helper bridge status is unavailable because the frontend bridge could not call Tauri."),
    );
  },

  async startHelperBridge(): Promise<HelperBridgeActionResult> {
    return invokeOr<HelperBridgeActionResult>(
      "start_helper_bridge",
      undefined,
      helperActionFallback("Start Helper failed before reaching the Tauri command bridge."),
    );
  },

  async verifyRequiredOutboundAiReadiness(): Promise<HelperBridgeActionResult> {
    return invokeOr<HelperBridgeActionResult>(
      "verify_required_outbound_ai_readiness",
      undefined,
      helperActionFallback("The final local translation check failed before reaching the Tauri runtime."),
    );
  },

  async helperBridgeWorkerStatus(): Promise<HelperBridgeWorkerResponse> {
    return invokeOr<HelperBridgeWorkerResponse>(
      "helper_bridge_worker_status",
      undefined,
      helperWorkerFallback("status", "Worker status failed before reaching the Tauri command bridge."),
    );
  },

  async startCapture(): Promise<CommandResult> {
    return invokeOr<CommandResult>(
      "start_capture",
      undefined,
      commandFallback("Start capture failed before reaching the Tauri command bridge."),
    );
  },

  async stopCapture(): Promise<CommandResult> {
    return invokeOr<CommandResult>(
      "stop_capture",
      undefined,
      commandFallback("Stop capture failed before reaching the Tauri command bridge."),
    );
  },

  async getInputStatus(): Promise<InputPreparationStatus> {
    const status = await invokeOr<NativeInputPreparationStatus>(
      "get_input_status",
      undefined,
      inputStatusFallback("Microphone status is unavailable because the frontend bridge could not call Tauri."),
    );
    return normalizeInputStatus(status);
  },

  async listAudioDevices(): Promise<AudioDeviceListReport> {
    return invokeOr<AudioDeviceListReport>(
      "list_audio_devices",
      undefined,
      audioDevicesFallback("Audio device list is unavailable because the frontend bridge could not call Tauri."),
    );
  },

  async probeInputDeviceCandidate(deviceId: string | null): Promise<InputPreparationStatus> {
    const status = await invokeOr<NativeInputPreparationStatus>(
      "probe_input_device_candidate",
      { deviceId },
      inputStatusFallback("Microphone candidate could not be checked because the frontend bridge could not call Tauri."),
    );
    return normalizeInputStatus(status);
  },

  async probeOutputDeviceCandidate(deviceId: string | null): Promise<AudioDeviceProbeReport> {
    return invokeOr<AudioDeviceProbeReport>(
      "probe_output_device_candidate",
      { deviceId },
      audioDeviceProbeFallback("Meeting sound candidate could not be checked because the frontend bridge could not call Tauri."),
    );
  },

  async loadSettings(): Promise<RuntimeSettings | null> {
    return loadRuntimeSettings();
  },

  async saveSettings(settings: RuntimeSettings): Promise<CommandResult> {
    return invokeOr<CommandResult>(
      "save_runtime_settings",
      { settings },
      commandFallback("Settings could not be saved because the frontend bridge could not call Tauri."),
    );
  },

  async selectAudioDevice(kind: "microphone" | "meeting-sound", deviceId: string | null): Promise<AudioDeviceSelectionCommandResult | null> {
    return invokeNullable<AudioDeviceSelectionCommandResult>("select_audio_device", { kind, deviceId });
  },

  async translateText(source: string): Promise<TextTranslationCommandResult> {
    return invokeOr<TextTranslationCommandResult>(
      "translate_text",
      { source },
      textTranslationFallback(),
    );
  },

  async translateTextAlternative(source: string, currentTranslation: string): Promise<TextTranslationCommandResult> {
    return invokeOr<TextTranslationCommandResult>(
      "translate_text_alternative",
      { source, currentTranslation },
      textTranslationFallback(),
    );
  },

  async verifyModels(): Promise<ModelInventoryReport | null> {
    return invokeNullable<ModelInventoryReport>("verify_models");
  },
};

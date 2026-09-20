export const APPLICATION_MEETING_OWNER_ID = "translateit_application_meeting";

export type AppRoute = "meeting" | "text" | "my-voice" | "settings";

export type CommandResult = {
  ok: boolean;
  state: string;
  message: string;
  [key: string]: unknown;
};

export type RuntimeCommandError = {
  command: string;
  message: string;
  occurred_at: string;
};

export type TerminologyEntry = {
  indonesian: string;
  english: string;
};

export type SpokenTermEntry = {
  term: string;
  aliases: string[];
};

export type RuntimeSettings = {
  schema_version: number;
  source_language: string;
  target_language: string;
  meeting_listen_source_language: string;
  meeting_listen_target_language: string;
  meeting_setup_state: "new" | "deferred" | "completed" | string;
  meeting_setup_checkpoint: number;
  terminology: TerminologyEntry[];
  spoken_terms: SpokenTermEntry[];
  audio: {
    input_device_id: string | null;
    output_device_id: string | null;
  };
};

export type HelperBridgeStatus = {
  state: string;
  message: string;
  cuda_ready: boolean;
  provider_ready: boolean;
  functional_outbound_ready: boolean;
  functional_outbound_verified_unix_ms: number | null;
  degraded_mode: boolean;
  active_task: string | null;
  active_request_id: string | null;
  active_meeting_generation: number | null;
  active_meeting_session_id: string | null;
  active_meeting_lane: string | null;
  generation_token: number;
  last_error: string | null;
  stderr_log_path: string | null;
  updated_unix_ms: number;
  runtime_claim: string;
};

export type HelperBridgeActionResult = {
  ok: boolean;
  state: string;
  message: string;
  generation_token: number;
  runtime_claim: string;
};

export type HelperBridgeWorkerResponse = {
  ok: boolean;
  state: string;
  task: string;
  request_id: string;
  scheduler_priority: string;
  message: string;
  generation_token: number;
  runtime_claim: string;
  worker_response_json: string;
};

export type AudioDeviceSummary = {
  id?: string;
  name: string;
  is_default?: boolean;
  [key: string]: unknown;
};

export type AudioDeviceListReport = {
  ok: boolean;
  input_devices: AudioDeviceSummary[];
  output_devices: AudioDeviceSummary[];
  blocker?: string;
  note?: string;
  [key: string]: unknown;
};

export type InputPreparationStatus = {
  ready: boolean;
  prepared?: boolean;
  functional_verified?: boolean;
  callback_frames_observed?: number;
  selected_device_name: string | null;
  input_device_name?: string | null;
  device_count: number;
  blocker?: string | undefined;
  note: string;
  [key: string]: unknown;
};

export type ModelInventoryItem = {
  required: boolean;
  found: boolean;
  [key: string]: unknown;
};

export type ModelInventoryReport = {
  ok: boolean;
  status: string;
  items: ModelInventoryItem[];
  blockers: string[];
  note?: string;
  [key: string]: unknown;
};

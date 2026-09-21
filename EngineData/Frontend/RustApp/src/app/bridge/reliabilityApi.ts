import { runCommand } from "../shared/tauriBridge";

export type LongSessionHealthStatus = {
  state: string;
  healthy: boolean;
  session_age_ms: number;
  outbound_overflow_dropped: number;
  outbound_evicted_pending: number;
  deferred_incoming_depth: number;
  deferred_incoming_dropped_overflow: number;
  deferred_incoming_dropped_stale: number;
  transcript_dropped_turns: number;
  transcript_truncated: boolean;
  meeting_temp_file_count: number;
  incident_count: number;
  warning_count: number;
  note: string;
  updated_unix_ms: number;
};

export type RuntimeIncident = {
  category: string;
  component: string;
  blocker: string;
  note: string;
  occurred_unix_ms: number;
};

export type RuntimeIncidentSnapshot = {
  incidents: RuntimeIncident[];
  count: number;
  truncated: boolean;
  note: string;
};

export type DiagnosticSupportBundleResult = {
  ok: boolean;
  file_path: string | null;
  message: string;
  blocker: string;
};

export type DeviceLossGuardStatus = {
  state: string;
  healthy: boolean;
  action_required: boolean;
  required_device_lost: boolean;
  optional_device_lost: boolean;
  component: string;
  blocker: string;
  note: string;
  updated_unix_ms: number;
};

export type RuntimeWatchdogStatus = {
  state: string;
  healthy: boolean;
  action_required: boolean;
  component: string;
  stage: string;
  age_ms: number;
  threshold_ms: number;
  blocker: string;
  note: string;
  updated_unix_ms: number;
};

export type StartupRecoveryReport = {
  previous_unclean_shutdown: boolean;
  another_instance_detected: boolean;
  cleanup_attempted: boolean;
  cleanup_ok: boolean;
  removed_files: number;
  blocker: string;
  note: string;
  checked_unix_ms: number;
};

export async function getLongSessionHealthStatus(): Promise<LongSessionHealthStatus> {
  return (await runCommand<LongSessionHealthStatus>("get_long_session_health_status")) ?? {
    state: "unavailable",
    healthy: false,
    session_age_ms: 0,
    outbound_overflow_dropped: 0,
    outbound_evicted_pending: 0,
    deferred_incoming_depth: 0,
    deferred_incoming_dropped_overflow: 0,
    deferred_incoming_dropped_stale: 0,
    transcript_dropped_turns: 0,
    transcript_truncated: false,
    meeting_temp_file_count: 0,
    incident_count: 0,
    warning_count: 0,
    note: "Long-session health is unavailable right now.",
    updated_unix_ms: Date.now(),
  };
}

export async function getRecentRuntimeIncidents(): Promise<RuntimeIncidentSnapshot> {
  return (await runCommand<RuntimeIncidentSnapshot>("get_recent_runtime_incidents")) ?? {
    incidents: [],
    count: 0,
    truncated: false,
    note: "Recent incident history is unavailable right now.",
  };
}

export async function exportDiagnosticSupportBundle(): Promise<DiagnosticSupportBundleResult> {
  return (await runCommand<DiagnosticSupportBundleResult>("export_diagnostic_support_bundle")) ?? {
    ok: false,
    file_path: null,
    message: "Diagnostic support export is unavailable right now.",
    blocker: "frontend_bridge_unavailable",
  };
}

export async function getDeviceLossGuardStatus(): Promise<DeviceLossGuardStatus> {
  return (await runCommand<DeviceLossGuardStatus>("get_device_loss_guard_status")) ?? {
    state: "unavailable",
    healthy: false,
    action_required: false,
    required_device_lost: false,
    optional_device_lost: false,
    component: "",
    blocker: "frontend_bridge_unavailable",
    note: "Device-loss status is unavailable right now.",
    updated_unix_ms: Date.now(),
  };
}

export async function getRuntimeWatchdogStatus(): Promise<RuntimeWatchdogStatus> {
  return (await runCommand<RuntimeWatchdogStatus>("get_runtime_watchdog_status")) ?? {
    state: "unavailable",
    healthy: false,
    action_required: false,
    component: "",
    stage: "",
    age_ms: 0,
    threshold_ms: 0,
    blocker: "frontend_bridge_unavailable",
    note: "Runtime watchdog status is unavailable right now.",
    updated_unix_ms: Date.now(),
  };
}

export async function getStartupRecoveryStatus(): Promise<StartupRecoveryReport> {
  return (await runCommand<StartupRecoveryReport>("get_startup_recovery_status")) ?? {
    previous_unclean_shutdown: false,
    another_instance_detected: false,
    cleanup_attempted: false,
    cleanup_ok: false,
    removed_files: 0,
    blocker: "frontend_bridge_unavailable",
    note: "Startup recovery status is unavailable right now.",
    checked_unix_ms: Date.now(),
  };
}


export type MeetingReliabilitySnapshot = {
  watchdog: RuntimeWatchdogStatus;
  devices: DeviceLossGuardStatus;
};

export async function getMeetingReliabilitySnapshot(): Promise<MeetingReliabilitySnapshot> {
  return (await runCommand<MeetingReliabilitySnapshot>("get_meeting_reliability_snapshot")) ?? {
    watchdog: {
      state: "unavailable",
      healthy: false,
      action_required: false,
      component: "",
      stage: "",
      age_ms: 0,
      threshold_ms: 0,
      blocker: "frontend_bridge_unavailable",
      note: "Runtime watchdog status is unavailable right now.",
      updated_unix_ms: Date.now(),
    },
    devices: {
      state: "unavailable",
      healthy: false,
      action_required: false,
      required_device_lost: false,
      optional_device_lost: false,
      component: "",
      blocker: "frontend_bridge_unavailable",
      note: "Device-loss status is unavailable right now.",
      updated_unix_ms: Date.now(),
    },
  };
}

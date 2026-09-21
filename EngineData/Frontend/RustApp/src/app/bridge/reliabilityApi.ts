import { runCommand } from "../shared/tauriBridge";

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

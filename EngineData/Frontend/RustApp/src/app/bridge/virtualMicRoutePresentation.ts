import type { VirtualMicRouteContractStatus } from "./runtimeApi";

export type VirtualMicRouteCheck = "ready" | "setup_needed" | "checking" | "unavailable";

// A native route failure (ok=false) is evidence of a missing/invalid device,
// not a failed bridge. Only the explicit frontend bridge claim is "unknown".
export function classifyVirtualMicRoute(
  status: VirtualMicRouteContractStatus | null,
  preflightReady = false,
): VirtualMicRouteCheck {
  if (!status) return preflightReady ? "ready" : "checking";
  if (status.runtime_claim === "frontend_bridge_unavailable"
    || status.blocker === "frontend_bridge_unavailable") return "unavailable";
  return status.route_ready ? "ready" : "setup_needed";
}

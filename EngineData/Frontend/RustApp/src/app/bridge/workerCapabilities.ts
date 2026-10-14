import { compact } from "../shared/state";
import type { HelperBridgeWorkerResponse } from "../shared/types";
import type { WorkerCapabilitySnapshot } from "./runtimeProductTypes";

function jsonRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function safeMiB(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : null;
}

function torchMemoryDisplay(gpu: Record<string, unknown>, validStatus: boolean): string {
  if (!validStatus) return "Not checked";
  const allocator = jsonRecord(gpu["torch_allocator"]);
  if (allocator["state"] === "not_initialized") return "CUDA allocator not initialized";
  if (allocator["state"] !== "measured") return "Not available";
  const allocated = safeMiB(allocator["allocated_mib"]);
  const reserved = safeMiB(allocator["reserved_mib"]);
  const peakAllocated = safeMiB(allocator["peak_allocated_mib"]);
  const peakReserved = safeMiB(allocator["peak_reserved_mib"]);
  if (
    allocated === null || reserved === null || peakAllocated === null || peakReserved === null
    || allocated > reserved || peakAllocated < allocated || peakReserved < reserved
  ) return "Not available";
  return `${allocated} / ${reserved} MiB allocated / reserved · Peak ${peakAllocated} / ${peakReserved} MiB`;
}

function deviceMemoryDisplay(gpu: Record<string, unknown>, validStatus: boolean): string {
  if (!validStatus) return "Not checked";
  const allocator = jsonRecord(gpu["torch_allocator"]);
  const deviceMemory = jsonRecord(allocator["device_memory"]);
  if (allocator["state"] === "not_initialized") return "CUDA context not initialized";
  if (deviceMemory["state"] !== "measured") return "Not available";
  const free = safeMiB(deviceMemory["free_mib"]);
  const total = safeMiB(deviceMemory["total_mib"]);
  if (free === null || total === null || total === 0 || free > total) {
    return "Not available";
  }
  return `${free} / ${total} MiB free / total (snapshot)`;
}

export function parseWorkerCapabilities(
  workerStatus: HelperBridgeWorkerResponse | null,
): WorkerCapabilitySnapshot {
  if (!workerStatus?.worker_response_json) {
    return {
      responseAvailable: false,
      asrReady: false,
      translationIdEnReady: false,
      translationEnIdReady: false,
      ttsReady: false,
      blocker: "",
      note: "",
      asrDisplay: "Not checked",
      translationDisplay: "Not checked",
      voiceDisplay: "Not loaded",
      executionDisplay: "Not verified",
      torchMemoryDisplay: "Not checked",
      deviceMemoryDisplay: "Not checked",
    };
  }

  try {
    const payload = jsonRecord(JSON.parse(workerStatus.worker_response_json) as unknown);
    const readiness = jsonRecord(payload["readiness"]);
    const loaded = jsonRecord(payload["loaded"]);
    const gpu = jsonRecord(payload["gpu"]);
    const validStatus = payload["stage"] === "local_realtime_worker_preflight";
    const asrSelected = String(payload["selected_device"] ?? gpu["selected_device"] ?? "not verified");
    const translationSelected = String(
      payload["selected_translation_device"] ?? gpu["selected_translation_device"] ?? "not verified",
    );
    const rawDirections = loaded["translation_directions"];
    const directions = Array.isArray(rawDirections)
      ? rawDirections.map((value: unknown) => String(value)).filter(Boolean)
      : [];

    return {
      responseAvailable: payload["stage"] === "local_realtime_worker_preflight",
      asrReady: readiness["asr"] === true,
      translationIdEnReady: readiness["translation_id_en"] === true,
      translationEnIdReady: readiness["translation_en_id"] === true,
      ttsReady: readiness["voice_actor_tts"] === true,
      blocker: compact(payload["blocker"], ""),
      note: compact(payload["note"], ""),
      asrDisplay: loaded["asr"] === true
        ? `${String(loaded["asr_model_id"] ?? "ASR")} · ${String(loaded["asr_device"] ?? "unknown")} / ${String(loaded["asr_compute_type"] ?? "unknown")}`
        : `Not loaded · selected ${asrSelected}`,
      translationDisplay: directions.length > 0
        ? `${directions.join(", ")} · ${translationSelected}`
        : `Not loaded · selected ${translationSelected}`,
      voiceDisplay: loaded["voice_actor"] === true
        ? `Loaded · ${String(loaded["voice_actor_device"] ?? "unknown")}`
        : "Not loaded",
      executionDisplay: `ASR ${asrSelected} · Translation ${translationSelected}`,
      torchMemoryDisplay: torchMemoryDisplay(gpu, validStatus),
      deviceMemoryDisplay: deviceMemoryDisplay(gpu, validStatus),
    };
  } catch {
    return {
      responseAvailable: false,
      asrReady: false,
      translationIdEnReady: false,
      translationEnIdReady: false,
      ttsReady: false,
      blocker: "helper_bridge:invalid_worker_status_response",
      note: "Worker capability response could not be parsed.",
      asrDisplay: "Not checked",
      translationDisplay: "Not checked",
      voiceDisplay: "Not loaded",
      executionDisplay: "Worker status could not be parsed",
      torchMemoryDisplay: "Not available",
      deviceMemoryDisplay: "Not available",
    };
  }
}

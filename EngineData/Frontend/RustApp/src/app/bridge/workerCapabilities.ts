import { compact } from "../shared/state";
import type { HelperBridgeWorkerResponse } from "../shared/types";
import type { WorkerCapabilitySnapshot } from "./runtimeProductTypes";

function jsonRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
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
    };
  }

  try {
    const payload = jsonRecord(JSON.parse(workerStatus.worker_response_json) as unknown);
    const readiness = jsonRecord(payload["readiness"]);
    const loaded = jsonRecord(payload["loaded"]);
    const gpu = jsonRecord(payload["gpu"]);
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
    };
  }
}

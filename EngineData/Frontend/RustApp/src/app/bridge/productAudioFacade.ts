import { runtimeApi, type AudioDeviceProbeReport } from "./runtimeApi";
import { compact } from "../shared/state";
import type { AudioDeviceListReport, RuntimeSettings } from "../shared/types";

export type ProductAudioDeviceKind = "microphone" | "meeting-sound";

export type ProductAudioDeviceProbe = {
  ok: boolean;
  kind: ProductAudioDeviceKind;
  deviceId: string | null;
  deviceName: string;
  message: string;
};

export type ProductAudioDeviceSelectionResult = ProductAudioDeviceProbe & {
  settings: RuntimeSettings;
};

export async function loadProductAudioDevices(): Promise<AudioDeviceListReport> {
  return runtimeApi.listAudioDevices();
}

export async function probeProductAudioDevice(
  kind: ProductAudioDeviceKind,
  deviceId: string | null,
): Promise<ProductAudioDeviceProbe> {
  const normalizedDeviceId = String(deviceId ?? "").trim() || null;
  if (kind === "microphone") {
    const status = await runtimeApi.probeInputDeviceCandidate(normalizedDeviceId);
    const ok = status.functional_verified === true;
    return {
      ok,
      kind,
      deviceId: normalizedDeviceId,
      deviceName: compact(
        status.selected_device_name ?? normalizedDeviceId,
        normalizedDeviceId ? "Selected microphone" : "Windows Default",
      ),
      message: ok
        ? "Microphone is available."
        : "This microphone can't be used right now. Choose another microphone or Windows Default.",
    };
  }

  const status: AudioDeviceProbeReport = await runtimeApi.probeOutputDeviceCandidate(normalizedDeviceId);
  return {
    ok: Boolean(status.ok),
    kind,
    deviceId: normalizedDeviceId,
    deviceName: compact(
      status.resolved_device_name ?? normalizedDeviceId,
      normalizedDeviceId ? "Selected meeting sound" : "Windows Default",
    ),
    message: status.ok
      ? "Meeting sound is available."
      : "That sound device can't be used right now. Choose another device or Windows Default.",
  };
}

export async function selectProductAudioDevice(
  kind: ProductAudioDeviceKind,
  deviceId: string | null,
  currentSettings: RuntimeSettings,
): Promise<ProductAudioDeviceSelectionResult> {
  const normalizedDeviceId = String(deviceId ?? "").trim() || null;
  const result = await runtimeApi.selectAudioDevice(kind, normalizedDeviceId);
  if (!result) {
    return {
      ok: false,
      kind,
      deviceId: normalizedDeviceId,
      deviceName: normalizedDeviceId ?? "Windows Default",
      message: "Audio settings are unavailable right now. The previous device preference was kept.",
      settings: currentSettings,
    };
  }

  return {
    ok: result.ok,
    kind,
    deviceId: result.device_id,
    deviceName: compact(result.device_name, result.device_id ?? "Windows Default"),
    message: compact(result.message, result.ok ? "Audio device saved." : "Audio device was not changed."),
    settings: result.settings,
  };
}

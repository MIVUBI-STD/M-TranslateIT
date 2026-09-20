import type { RuntimeSettings } from "./types";

const MAX_ERROR_MESSAGE_CHARS = 240;
const UNSAFE_DISPLAY_CHARS = /[\u0000-\u001f\u007f\u202a-\u202e\u2066-\u2069]/g;

export function defaultSettings(): RuntimeSettings {
  return {
    schema_version: 7,
    source_language: "id",
    target_language: "en",
    meeting_setup_state: "new",
    meeting_setup_checkpoint: 1,
    terminology: [],
    audio: {
      input_device_id: null,
      output_device_id: null,
    },
  };
}

export function compact(value: unknown, fallback = "Unknown", maxChars = 180): string {
  const text = String(value ?? "").replace(/\s+/g, " ").trim();
  if (!text) return fallback;
  return text.length > maxChars ? `${text.slice(0, maxChars - 1).trimEnd()}…` : text;
}

export function cloneSettings(value: RuntimeSettings): RuntimeSettings {
  return {
    ...value,
    terminology: value.terminology.map((entry) => ({ ...entry })),
    audio: { ...value.audio },
  };
}

export function deviceId(device: { id?: string; name: string }): string {
  return String(device.id ?? device.name).trim();
}

function cleanDisplayText(value: string): string {
  return value.replace(UNSAFE_DISPLAY_CHARS, "").replace(/\s+/g, " ").trim();
}

export function errorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const clean = cleanDisplayText(raw) || "Unknown error";
  return clean.length > MAX_ERROR_MESSAGE_CHARS ? `${clean.slice(0, MAX_ERROR_MESSAGE_CHARS - 1)}…` : clean;
}

export function languageName(code: string): string {
  const normalized = cleanDisplayText(code).toLowerCase();
  if (normalized.startsWith("id")) return "Indonesian";
  if (normalized.startsWith("en")) return "English";
  return normalized ? normalized.toUpperCase() : "Unknown";
}

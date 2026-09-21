import type { RuntimeSettings } from "../shared/types";

export type MeetingPreset = {
  id: string;
  name: string;
  sourceLanguage: string;
  targetLanguage: string;
  listenSourceLanguage: string;
  listenTargetLanguage: string;
  translationStyle: string;
  microphoneId: string | null;
  meetingSoundId: string | null;
  noiseSuppression: string;
};

const PRESETS_KEY = "translateit.meeting-presets.v1";
const PROVIDER_KEY = "translateit.meeting-preset-provider.v1";
const MAX_PRESETS = 6;

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) as T : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): boolean {
  try {
    localStorage.setItem(key, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}

function cleanName(value: string): string {
  return value.replace(/[\u0000-\u001f\u007f]/g, "").trim().slice(0, 40);
}

export function meetingPresetFromSettings(id: string, name: string, settings: RuntimeSettings): MeetingPreset {
  return {
    id,
    name: cleanName(name) || "Meeting preset",
    sourceLanguage: settings.source_language,
    targetLanguage: settings.target_language,
    listenSourceLanguage: settings.meeting_listen_source_language,
    listenTargetLanguage: settings.meeting_listen_target_language,
    translationStyle: settings.translation_style,
    microphoneId: settings.audio.input_device_id,
    meetingSoundId: settings.audio.output_device_id,
    noiseSuppression: settings.audio.noise_suppression,
  };
}

function optionalText(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim().slice(0, 160) : null;
}

function validPreset(value: unknown): MeetingPreset | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  const id = item["id"];
  const name = item["name"];
  const sourceLanguage = item["sourceLanguage"];
  const targetLanguage = item["targetLanguage"];
  const listenSourceLanguage = item["listenSourceLanguage"];
  const listenTargetLanguage = item["listenTargetLanguage"];
  const translationStyle = item["translationStyle"];
  const noiseSuppression = item["noiseSuppression"];
  if (
    typeof id !== "string"
    || typeof name !== "string"
    || typeof sourceLanguage !== "string"
    || typeof targetLanguage !== "string"
    || typeof listenSourceLanguage !== "string"
    || typeof listenTargetLanguage !== "string"
    || typeof translationStyle !== "string"
    || typeof noiseSuppression !== "string"
  ) return null;
  return {
    id: id.slice(0, 80),
    name: cleanName(name) || "Meeting preset",
    sourceLanguage: sourceLanguage.slice(0, 16),
    targetLanguage: targetLanguage.slice(0, 16),
    listenSourceLanguage: listenSourceLanguage.slice(0, 16),
    listenTargetLanguage: listenTargetLanguage.slice(0, 16),
    translationStyle: translationStyle.slice(0, 24),
    microphoneId: optionalText(item["microphoneId"]),
    meetingSoundId: optionalText(item["meetingSoundId"]),
    noiseSuppression: noiseSuppression.slice(0, 24),
  };
}

export function readMeetingPresets(): MeetingPreset[] {
  const values = readJson<unknown[]>(PRESETS_KEY, []);
  if (!Array.isArray(values)) return [];
  return values.map(validPreset).filter((value): value is MeetingPreset => value !== null).slice(0, MAX_PRESETS);
}

export function saveMeetingPreset(preset: MeetingPreset): { ok: boolean; presets: MeetingPreset[] } {
  const existing = readMeetingPresets().filter((entry) => entry.id !== preset.id);
  const next = [preset, ...existing].slice(0, MAX_PRESETS);
  return { ok: writeJson(PRESETS_KEY, next), presets: next };
}

function readProviderPresetMap(): Record<string, string> {
  const raw = readJson<unknown>(PROVIDER_KEY, {});
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const clean: Record<string, string> = {};
  for (const [provider, presetId] of Object.entries(raw)) {
    if (typeof presetId === "string" && provider.trim() && presetId.trim()) {
      clean[provider.trim().toLowerCase().slice(0, 80)] = presetId.trim().slice(0, 80);
    }
  }
  return clean;
}

export function deleteMeetingPreset(id: string): { ok: boolean; presets: MeetingPreset[] } {
  const next = readMeetingPresets().filter((entry) => entry.id !== id);
  const presetsSaved = writeJson(PRESETS_KEY, next);
  const map = readProviderPresetMap();
  for (const [provider, presetId] of Object.entries(map)) if (presetId === id) delete map[provider];
  const providerMapSaved = writeJson(PROVIDER_KEY, map);
  return { ok: presetsSaved && providerMapSaved, presets: presetsSaved ? next : readMeetingPresets() };
}

export function rememberProviderPreset(provider: string, presetId: string): boolean {
  const key = provider.trim().toLowerCase().slice(0, 80);
  const value = presetId.trim().slice(0, 80);
  if (!key || !value) return false;
  const map = readProviderPresetMap();
  map[key] = value;
  return writeJson(PROVIDER_KEY, map);
}

export function suggestedPresetForProvider(provider: string | null): MeetingPreset | null {
  const key = String(provider ?? "").trim().toLowerCase();
  if (!key) return null;
  const map = readProviderPresetMap();
  const id = map[key];
  return id ? readMeetingPresets().find((preset) => preset.id === id) ?? null : null;
}

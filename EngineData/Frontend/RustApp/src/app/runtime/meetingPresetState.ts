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

function writeJson(key: string, value: unknown): void {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch {}
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

export function readMeetingPresets(): MeetingPreset[] {
  const values = readJson<MeetingPreset[]>(PRESETS_KEY, []);
  if (!Array.isArray(values)) return [];
  return values
    .filter((value) => value && typeof value.id === "string" && typeof value.name === "string")
    .slice(0, MAX_PRESETS);
}

export function saveMeetingPreset(preset: MeetingPreset): MeetingPreset[] {
  const existing = readMeetingPresets().filter((entry) => entry.id !== preset.id);
  const next = [preset, ...existing].slice(0, MAX_PRESETS);
  writeJson(PRESETS_KEY, next);
  return next;
}

export function deleteMeetingPreset(id: string): MeetingPreset[] {
  const next = readMeetingPresets().filter((entry) => entry.id !== id);
  writeJson(PRESETS_KEY, next);
  const map = readJson<Record<string, string>>(PROVIDER_KEY, {});
  for (const [provider, presetId] of Object.entries(map)) if (presetId === id) delete map[provider];
  writeJson(PROVIDER_KEY, map);
  return next;
}

export function rememberProviderPreset(provider: string, presetId: string): void {
  const key = provider.trim().toLowerCase();
  if (!key) return;
  const map = readJson<Record<string, string>>(PROVIDER_KEY, {});
  map[key] = presetId;
  writeJson(PROVIDER_KEY, map);
}

export function suggestedPresetForProvider(provider: string | null): MeetingPreset | null {
  const key = String(provider ?? "").trim().toLowerCase();
  if (!key) return null;
  const map = readJson<Record<string, string>>(PROVIDER_KEY, {});
  const id = map[key];
  return id ? readMeetingPresets().find((preset) => preset.id === id) ?? null : null;
}

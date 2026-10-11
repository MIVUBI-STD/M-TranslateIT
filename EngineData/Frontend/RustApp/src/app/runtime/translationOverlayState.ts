import type {
  OverlayContrast,
  OverlayTextSize,
  OverlayVisibility,
  OverlayWidth,
  TranslationOverlayPreferences,
} from "./translationOverlayPolicy";
const DEFAULT_OVERLAY_PREFERENCES: TranslationOverlayPreferences = {
  meetingEnabled: true,
  visibility: "expanded",
  textSize: "medium",
  width: "standard",
  contrast: "standard",
  clickThrough: false,
};

const LATEST_KEY = "translateit.translationOverlay.latest.v2";
const PREFERENCES_KEY = "translateit.translationOverlay.preferences.v2";
const POSITION_KEY = "translateit.translationOverlay.position.v2";
const ERROR_KEY = "translateit.translationOverlay.error.v1";

export type StoredOverlayPosition = { x: number; y: number };
export type OverlayDiagnostic = { message: string; occurredAt: string } | null;

function storage(): Storage | null {
  try { return typeof localStorage === "undefined" ? null : localStorage; } catch { return null; }
}

export function readOverlayPreferences(): TranslationOverlayPreferences {
  try {
    const raw = storage()?.getItem(PREFERENCES_KEY);
    if (!raw) return { ...DEFAULT_OVERLAY_PREFERENCES };
    const parsed = JSON.parse(raw) as Partial<TranslationOverlayPreferences>;
    const visibility: OverlayVisibility = ["expanded", "collapsed", "hidden"].includes(String(parsed.visibility))
      ? parsed.visibility as OverlayVisibility : DEFAULT_OVERLAY_PREFERENCES.visibility;
    const textSize: OverlayTextSize = ["small", "medium", "large", "extra-large"].includes(String(parsed.textSize))
      ? parsed.textSize as OverlayTextSize : DEFAULT_OVERLAY_PREFERENCES.textSize;
    const width: OverlayWidth = ["compact", "standard", "wide"].includes(String(parsed.width))
      ? parsed.width as OverlayWidth : DEFAULT_OVERLAY_PREFERENCES.width;
    const contrast: OverlayContrast = ["standard", "high"].includes(String(parsed.contrast))
      ? parsed.contrast as OverlayContrast : DEFAULT_OVERLAY_PREFERENCES.contrast;
    return { meetingEnabled: parsed.meetingEnabled !== false, visibility, textSize, width, contrast, clickThrough: parsed.clickThrough === true };
  } catch { return { ...DEFAULT_OVERLAY_PREFERENCES }; }
}

export function writeOverlayPreferences(next: TranslationOverlayPreferences): void {
  storage()?.setItem(PREFERENCES_KEY, JSON.stringify(next));
}

export function updateOverlayPreferences(patch: Partial<TranslationOverlayPreferences>): TranslationOverlayPreferences {
  const next = { ...readOverlayPreferences(), ...patch };
  writeOverlayPreferences(next);
  return next;
}

// Delete the old plaintext caption cache without deserializing the text.
export function discardLegacyOverlayCaption(): void {
  try { storage()?.removeItem(LATEST_KEY); } catch { /* Storage can be disabled. */ }
}

export function readOverlayPosition(): StoredOverlayPosition | null {
  try {
    const raw = storage()?.getItem(POSITION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as StoredOverlayPosition;
    return Number.isFinite(parsed?.x) && Number.isFinite(parsed?.y) ? parsed : null;
  } catch { return null; }
}

export function writeOverlayPosition(position: StoredOverlayPosition): void {
  storage()?.setItem(POSITION_KEY, JSON.stringify(position));
}

export function recordOverlayError(message: string): void {
  storage()?.setItem(ERROR_KEY, JSON.stringify({ message: message.slice(0, 220), occurredAt: new Date().toISOString() }));
}
export function clearOverlayError(): void { storage()?.removeItem(ERROR_KEY); }
export function readOverlayDiagnostic(): OverlayDiagnostic {
  try { const raw = storage()?.getItem(ERROR_KEY); return raw ? JSON.parse(raw) as OverlayDiagnostic : null; } catch { return null; }
}

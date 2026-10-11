import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { MeetingCommittedTurnsSnapshot } from "../bridge/runtimeApi";
import {
  TRANSLATION_OVERLAY_EVENT,
  TRANSLATION_OVERLAY_CLEAR_EVENT,
  TRANSLATION_OVERLAY_READY_EVENT,
  TRANSLATION_OVERLAY_PREFERENCES_EVENT,
  latestMeetingCaption,
  normalizeOverlayText,
  type TranslationOverlayPayload,
  type TranslationOverlayPreferences,
} from "./translationOverlayPolicy";
import {
  clearOverlayError,
  readOverlayPreferences,
  recordOverlayError,
  updateOverlayPreferences,
} from "./translationOverlayState";

export const OVERLAY_WINDOW_LABEL = "translation-overlay";
export type OverlayPublishResult = "shown" | "suppressed" | "unavailable";
// Private in-memory caption, never written to browser storage.
let latestPresented: TranslationOverlayPayload | null = null;

async function overlayWindow(): Promise<WebviewWindow | null> {
  return WebviewWindow.getByLabel(OVERLAY_WINDOW_LABEL);
}
async function emitPreferences(preferences: TranslationOverlayPreferences): Promise<void> {
  await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_PREFERENCES_EVENT, preferences);
}

export async function publishTranslationOverlay(payload: TranslationOverlayPayload, explicit = false): Promise<OverlayPublishResult> {
  const text = normalizeOverlayText(payload.text);
  if (!text) return "suppressed";
  const normalized = { ...payload, text };
  let preferences = readOverlayPreferences();
  const explicitlyRestored = explicit && preferences.visibility === "hidden";
  if (explicitlyRestored) preferences = updateOverlayPreferences({ visibility: "expanded" });
  if (!explicit && (preferences.visibility === "hidden" || (payload.source === "meeting" && !preferences.meetingEnabled))) return "suppressed";
  latestPresented = normalized;

  try {
    const window = await overlayWindow();
    if (!window) throw new Error("Floating caption window is unavailable.");
    if (explicitlyRestored) await emitPreferences(preferences);
    // Deliver current content before exposing the window: never flash a previous caption.
    await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, normalized);
    await window.show();
    clearOverlayError();
    return "shown";
  } catch (error) {
    recordOverlayError(error instanceof Error ? error.message : "Floating caption update failed.");
    return "unavailable";
  }
}

export async function publishLatestMeetingOverlay(turns: MeetingCommittedTurnsSnapshot | null, previousRevision: string): Promise<string> {
  const caption = latestMeetingCaption(turns);
  if (!caption || caption.revision === previousRevision) return previousRevision;
  const result = await publishTranslationOverlay(caption);
  return result === "unavailable" ? previousRevision : caption.revision;
}

// Replay after the separate webview registers its caption listener.
export async function subscribeOverlayReady(): Promise<UnlistenFn> {
  return listen(TRANSLATION_OVERLAY_READY_EVENT, async () => {
    const prefs = readOverlayPreferences();
    if (prefs.visibility === "hidden") return;
    try {
      if (latestPresented && (latestPresented.source !== "meeting" || prefs.meetingEnabled)) await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, latestPresented);
      else await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {});
    } catch { /* Future events still have a live delivery path. */ }
  });
}

export async function notifyOverlayPreferencesChanged(preferences: TranslationOverlayPreferences): Promise<void> {
  try { await emitPreferences(preferences); } catch { }
}

export async function showTranslationOverlay(): Promise<boolean> {
  const preferences = updateOverlayPreferences({ visibility: "expanded" });
  try {
    const window = await overlayWindow();
    if (!window) throw new Error("Floating caption window is unavailable.");
    await emitPreferences(preferences);
    if (latestPresented) await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, latestPresented);
    else await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {});
    await window.show();
    clearOverlayError();
    return true;
  } catch (error) {
    recordOverlayError(error instanceof Error ? error.message : "Floating caption could not be shown.");
    return false;
  }
}

export async function hideTranslationOverlay(): Promise<void> {
  const prefs = updateOverlayPreferences({ visibility: "hidden" });
  latestPresented = null;
  try { await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {}); } catch { /* Unavailable overlay. */ }
  try { await emitPreferences(prefs); } catch { /* Hiding must not depend on event delivery. */ }
  try { await (await overlayWindow())?.hide(); } catch { }
}

export async function destroyTranslationOverlay(): Promise<void> {
  latestPresented = null;
  await (await overlayWindow())?.destroy();
}

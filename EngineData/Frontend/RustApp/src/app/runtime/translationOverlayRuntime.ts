import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Window } from "@tauri-apps/api/window";
import type { MeetingCommittedTurnsSnapshot } from "../bridge/runtimeApi";
import {
  TRANSLATION_OVERLAY_EVENT,
  TRANSLATION_OVERLAY_CLEAR_EVENT,
  TRANSLATION_OVERLAY_HIDDEN_EVENT,
  TRANSLATION_OVERLAY_READY_EVENT,
  TRANSLATION_OVERLAY_PREFERENCES_EVENT,
  latestMeetingCaption,
  normalizeOverlayText,
  mayPresentOverlayCaption,
  type TranslationOverlayPayload,
  type TranslationOverlayClearPayload,
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

// Coalesced native visibility reconciliation; one running operation, one latest intent.
let pendingNativeVisibility: boolean | null = null;
let nativeVisibilityTask: Promise<void> | null = null;

function requestNativeOverlayVisible(visible: boolean): Promise<void> {
  pendingNativeVisibility = visible;
  if (nativeVisibilityTask) return nativeVisibilityTask;
  nativeVisibilityTask = (async () => {
    try {
      while (pendingNativeVisibility !== null) {
        const requested = pendingNativeVisibility;
        pendingNativeVisibility = null;
        const window = await overlayWindow();
        if (!window) throw new Error("Floating caption window is unavailable.");
        // Recheck the cross-webview policy after asynchronous window lookup.
        if (requested && readOverlayPreferences().visibility !== "hidden") await window.show();
        else await window.hide();
      }
    } finally {
      nativeVisibilityTask = null;
    }
  })();
  return nativeVisibilityTask;
}

// Meeting termination invalidates its caption without erasing an explicit Text overlay.
export async function clearLatestMeetingOverlay(): Promise<void> {
  const previous = latestPresented;
  if (previous?.source !== "meeting") return;
  latestPresented = null;
  const payload: TranslationOverlayClearPayload = { onlyRevision: previous.revision };
  try { await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, payload); }
  catch { /* No sensitive payload is logged or persisted. */ }
}

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
  if (!explicit && !mayPresentOverlayCaption(preferences, normalized)) return "suppressed";
  latestPresented = normalized;

  try {
    const window = await overlayWindow();
    if (!window) throw new Error("Floating caption window is unavailable.");
    if (explicitlyRestored) await emitPreferences(preferences);
    // Deliver current content before exposing the window: never flash a previous caption.
    await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, normalized);
    await requestNativeOverlayVisible(true);
    if (readOverlayPreferences().visibility === "hidden") return "suppressed";
    clearOverlayError();
    return "shown";
  } catch (error) {
    recordOverlayError(error instanceof Error ? error.message : "Floating caption update failed.");
    return "unavailable";
  }
}

export async function publishLatestMeetingOverlay(turns: MeetingCommittedTurnsSnapshot | null, previousRevision: string): Promise<string> {
  const caption = latestMeetingCaption(turns);
  if (!caption) {
    await clearLatestMeetingOverlay();
    return "";
  }
  if (caption.revision === previousRevision) return previousRevision;
  const result = await publishTranslationOverlay(caption);
  return result === "unavailable" ? previousRevision : caption.revision;
}

// Replay after the separate webview registers its caption listener.
export async function subscribeOverlayReady(): Promise<UnlistenFn> {
  const stopReady = await listen(TRANSLATION_OVERLAY_READY_EVENT, async () => {
    const prefs = readOverlayPreferences();
    if (prefs.visibility === "hidden") return;
    try {
      if (latestPresented && mayPresentOverlayCaption(prefs, latestPresented)) {
        await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, latestPresented);
      } else {
        await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {});
      }
    } catch { /* Future caption events may still deliver. */ }
  });
  try {
    // The overlay and main window have independent JavaScript module memory.
    const stopHidden = await listen(TRANSLATION_OVERLAY_HIDDEN_EVENT, () => {
      // A delayed Hide event cannot clear a newer explicit Show/translation intent.
      if (readOverlayPreferences().visibility === "hidden") latestPresented = null;
    });
    return () => { stopReady(); stopHidden(); };
  } catch (error) {
    stopReady();
    throw error;
  }
}

export async function notifyOverlayPreferencesChanged(preferences: TranslationOverlayPreferences): Promise<void> {
  try { await emitPreferences(preferences); } catch { }
}

export async function showTranslationOverlay(): Promise<boolean> {
  // An ordinary Show never restores conversation text after the previous Hide.
  if (readOverlayPreferences().visibility === "hidden") latestPresented = null;
  const preferences = updateOverlayPreferences({ visibility: "expanded" });
  try {
    const window = await overlayWindow();
    if (!window) throw new Error("Floating caption window is unavailable.");
    await emitPreferences(preferences);
    if (latestPresented) await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, latestPresented);
    else await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {});
    await requestNativeOverlayVisible(true);
    if (readOverlayPreferences().visibility === "hidden") return false;
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
  try { await requestNativeOverlayVisible(readOverlayPreferences().visibility !== "hidden"); } catch { /* Hide preference remains in force. */ }
  try { await emit(TRANSLATION_OVERLAY_HIDDEN_EVENT); } catch { /* Persisted Hide remains authoritative. */ }
  try { await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_CLEAR_EVENT, {}); } catch { /* Unavailable overlay. */ }
  try { await emitPreferences(prefs); } catch { /* Hiding must not depend on event delivery. */ }
}

export async function destroyTranslationOverlay(): Promise<void> {
  latestPresented = null;
  pendingNativeVisibility = false;
  try { await nativeVisibilityTask; } catch { /* Shutdown still owns destruction. */ }
  await (await overlayWindow())?.destroy();
}


/**
 * Quick Access asks the existing main caption owner to show or hide its window.
 * The popup must not call showTranslationOverlay itself: its private in-memory
 * caption is empty and would replace the user's active captions.
 */
export const QUICK_ACCESS_CAPTION_REQUEST = "translateit://quick-access-caption-request";
export const QUICK_ACCESS_CAPTION_RESPONSE = "translateit://quick-access-caption-response";
export type QuickAccessCaptionRequest = { id: string; visible: boolean };
export type QuickAccessCaptionResponse = { id: string; ok: boolean; visible: boolean; message: string };

export async function subscribeQuickAccessCaptionRequests(): Promise<UnlistenFn> {
  return listen<QuickAccessCaptionRequest>(QUICK_ACCESS_CAPTION_REQUEST, async event => {
    const request = event.payload;
    if (typeof request?.id !== "string" || !/^qa_[a-z0-9]+$/.test(request.id)
        || typeof request.visible !== "boolean") return;
    let ok = false;
    let visible = false;
    let message = "Floating Captions could not be updated.";
    try {
      if (request.visible) {
        updateOverlayPreferences({ meetingEnabled: true });
        ok = await showTranslationOverlay();
      } else {
        await hideTranslationOverlay();
        ok = true;
      }
      const native = await Window.getByLabel(OVERLAY_WINDOW_LABEL);
      visible = native ? await native.isVisible() : false;
      ok = ok && visible === request.visible;
      message = ok ? visible ? "Floating Captions shown." : "Floating Captions hidden."
        : "Floating Captions state was not confirmed.";
    } catch { ok = false; }
    try {
      await emitTo("quick-access", QUICK_ACCESS_CAPTION_RESPONSE, {
        id: request.id, ok, visible, message,
      } satisfies QuickAccessCaptionResponse);
    } catch { /* The caller owns its timeout and must not infer success. */ }
  });
}

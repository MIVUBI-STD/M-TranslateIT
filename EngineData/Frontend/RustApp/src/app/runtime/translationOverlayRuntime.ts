import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  TRANSLATION_OVERLAY_EVENT,
  normalizeOverlayText,
  type TranslationOverlayPayload,
} from "./translationOverlayPolicy";

const OVERLAY_WINDOW_LABEL = "translation-overlay";

export async function publishTranslationOverlay(payload: TranslationOverlayPayload): Promise<boolean> {
  const text = normalizeOverlayText(payload.text);
  if (!text) return false;

  try {
    const window = await WebviewWindow.getByLabel(OVERLAY_WINDOW_LABEL);
    if (!window) return false;
    await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, { ...payload, text });
    await window.show();
    return true;
  } catch {
    // Browser-only previews do not expose the native Tauri window/event bridge.
    return false;
  }
}

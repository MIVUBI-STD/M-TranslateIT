import { Window } from "@tauri-apps/api/window";
import { emitTo, listen } from "@tauri-apps/api/event";
import {
  QUICK_ACCESS_CAPTION_REQUEST, QUICK_ACCESS_CAPTION_RESPONSE,
  type QuickAccessCaptionResponse,
} from "./translationOverlayRuntime";
import { runCommand } from "../shared/tauriBridge";

export const QUICK_ACCESS_WINDOW_LABEL = "quick-access";

// Header, Ctrl+K, and native tray use the same Rust-owned popup instance.
export async function toggleQuickAccess(): Promise<boolean> {
  const shown = await runCommand<boolean>("toggle_quick_access");
  if (shown === null) throw new Error("Quick Access is unavailable.");
  return shown;
}

export async function openTranslateIt(): Promise<void> {
  const opened = await runCommand<boolean>("open_translateit_main");
  if (opened !== true) throw new Error("TranslateIT main window is unavailable.");
}

export async function hideQuickAccess(): Promise<void> {
  await (await Window.getByLabel(QUICK_ACCESS_WINDOW_LABEL))?.hide();
}

export async function destroyQuickAccessWindow(): Promise<void> {
  await (await Window.getByLabel(QUICK_ACCESS_WINDOW_LABEL))?.destroy();
}


let captionRequestCounter = 0;
const CAPTION_REQUEST_TIMEOUT_MS = 5_000;

export async function captionWindowIsVisible(): Promise<boolean | null> {
  try {
    const window = await Window.getByLabel("translation-overlay");
    return window ? await window.isVisible() : null;
  } catch { return null; }
}

/** Correlate one Quick Access request with one main-window caption result. */
export async function changeCaptionVisibility(visible: boolean): Promise<QuickAccessCaptionResponse> {
  const id = "qa_" + (++captionRequestCounter).toString(36);
  let unlisten: (() => void) | null = null;
  let timeout: ReturnType<typeof setTimeout> | null = null;
  return new Promise<QuickAccessCaptionResponse>(async (resolve, reject) => {
    const finish = () => {
      if (timeout !== null) clearTimeout(timeout);
      unlisten?.();
      unlisten = null;
    };
    try {
      unlisten = await listen<QuickAccessCaptionResponse>(QUICK_ACCESS_CAPTION_RESPONSE, event => {
        if (event.payload?.id !== id) return;
        finish();
        resolve(event.payload);
      });
      timeout = setTimeout(() => {
        finish();
        reject(new Error("Quick Access caption response expired."));
      }, CAPTION_REQUEST_TIMEOUT_MS);
      await emitTo("main", QUICK_ACCESS_CAPTION_REQUEST, { id, visible });
    } catch (error) {
      finish();
      reject(error);
    }
  });
}

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { runCommand } from "../shared/tauriBridge";

export const QUICK_ACCESS_WINDOW_LABEL = "quick-access";

// Header, Ctrl+K, and native tray use the same Rust-owned popup instance.
export async function toggleQuickAccess(): Promise<boolean> {
  const shown = await runCommand<boolean>("toggle_quick_access");
  if (shown === null) throw new Error("Quick Access is unavailable.");
  return shown;
}

export async function openTranslateIt(): Promise<void> {
  const result = await runCommand<void>("open_translateit_main");
  // This command returns void on success; runCommand's null sentinel cannot
  // distinguish a successful void command from an unavailable bridge.
  void result;
}

export async function hideQuickAccess(): Promise<void> {
  await (await WebviewWindow.getByLabel(QUICK_ACCESS_WINDOW_LABEL))?.hide();
}

export async function destroyQuickAccessWindow(): Promise<void> {
  await (await WebviewWindow.getByLabel(QUICK_ACCESS_WINDOW_LABEL))?.destroy();
}

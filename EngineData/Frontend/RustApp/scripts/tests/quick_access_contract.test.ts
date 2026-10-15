import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const source = (path: string) => readFileSync(new URL(path, root), "utf8");
const native = source("src-tauri/src/commands/quick_access.rs");
const config = JSON.parse(source("src-tauri/tauri.conf.json"));
const main = source("src/main.ts");
const popup = source("src/pages/QuickAccess.svelte");
const app = source("src/App.svelte");
const header = source("src/components/layout/AppHeader.svelte");

test("Quick Access is one on-demand popup reached by header, keyboard and native tray", () => {
  assert.equal(config.app.windows.filter((window: { label: string }) => window.label === "quick-access").length, 1);
  const quick = config.app.windows.find((window: { label: string }) => window.label === "quick-access");
  assert.equal(quick?.visible, false);
  assert.equal(quick?.alwaysOnTop, true);
  assert.equal(quick?.skipTaskbar, true);
  assert.match(main, /view === "quick-access"/);
  assert.match(header, /toggleQuickAccess/);
  assert.match(app, /onQuickAccessShortcut/);
  assert.match(native, /TrayIconBuilder::with_id\("quick-access-tray"\)/);
  assert.match(native, /toggle_window\(tray\.app_handle\(\)\)/);
  assert.match(native, /get_webview_window\(QUICK_ACCESS_LABEL\)/);
});

test("Quick Access is meeting-first and old command palette is gone", () => {
  for (const file of [
    "src/components/runtime/ProductivityActions.svelte",
    "src/components/runtime/CommandPalette.svelte",
    "src/app/runtime/productCommandRegistry.ts",
  ]) {
    assert.equal(existsSync(new URL(file, root)), false, file);
  }
  assert.match(popup, /Original Voice/);
  assert.match(popup, /TranslateIT Voice/);
  assert.match(popup, /Floating Captions/);
  assert.match(popup, /Confirm Stop/);
  assert.doesNotMatch(popup, /Search tools and settings|Favorites|My Voice|Text Translation/);
  assert.match(popup, /runtimeApi\.selectMeetingAudioSource/);
  assert.match(popup, /applicationRuntimeApi\.dispatchIntent/);
});

test("Dismissing Quick Access does not end Meeting; app exit destroys its window", () => {
  assert.match(popup, /onBlur/);
  assert.match(popup, /event\.key === "Escape"/);
  assert.match(popup, /closeAfterSuccess/);
  assert.match(native, /monitor_from_point/);
  assert.match(native, /work_area\(\)/);
  const close = source("src/app/runtime/nativeCloseRuntime.ts");
  assert.match(close, /await destroyQuickAccessWindow\(\)/);
  assert.doesNotMatch(source("src/app/runtime/quickAccessRuntime.ts"), /stop_meeting|set_meeting_audio_source/);
});

test("Quick Access controls captions through the existing overlay only", () => {
  assert.match(popup, /showTranslationOverlay\(\)/);
  assert.match(popup, /hideTranslationOverlay\(\)/);
  assert.match(popup, /notifyOverlayPreferencesChanged/);
  assert.match(popup, /native\.meeting\.audio_source !== requested/);
  assert.match(popup, /result\.snapshot\.meeting\.lifecycle !== "live"/);
});

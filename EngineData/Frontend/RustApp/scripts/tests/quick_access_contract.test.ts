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


test("Late status observations cannot overwrite a newer confirmed source", () => {
  assert.match(popup, /next\.revision < snapshot\.revision/);
  assert.match(popup, /const actual = snapshot\?\.meeting \?\? native\.meeting/);
  assert.match(popup, /actual\.audio_source !== requested/);
});

test("Native popup doesn't misreport a Windows foreground-focus denial as Show failure", () => {
  assert.match(native, /let _ = window\.set_focus\(\)/);
  assert.match(native, /TOGGLE_LOCK/);
  assert.match(native, /quick_access:window_show_failed/);
});


test("Tray callback and popup owner share one generic Tauri runtime identity", () => {
  assert.match(native, /fn popup_window<R: tauri::Runtime>/);
  assert.match(native, /fn position_popup<R: tauri::Runtime>/);
  assert.match(native, /pub fn toggle_window<R: tauri::Runtime>/);
  assert.match(native, /pub fn install_tray<R: tauri::Runtime>/);
});


test("Quick Access never pins itself over an unfocused call during a slow switch", () => {
  assert.match(popup, /const onBlur = \(\) => \{/);
  assert.match(popup, /Hiding does not cancel or alter the authoritative Meeting action/);
  assert.match(popup, /const actual = snapshot\?\.meeting \?\? result\.snapshot\.meeting/);
  assert.match(popup, /Preserve an unacknowledged error across close\/reopen/);
  assert.match(popup, /overflow-y-auto px-4 py-2/);
});


test("Popup success from a hidden operation cannot dismiss a later invocation", () => {
  assert.match(popup, /if \(!document\.hasFocus\(\)\) return/);
  assert.match(popup, /const token = \+\+actionRevision/);
  assert.match(popup, /if \(!busy\) resultMessage = ""/);
});


test("Quick Access includes a single native mute button independent of source selection", () => {
  const registry = source("src-tauri/src/commands/registry.rs");
  const nativeSource = source("src-tauri/src/commands/meeting_session/audio_source.rs");
  const api = source("src/app/bridge/runtimeApi.ts");
  assert.match(popup, /Microphone Mute/);
  assert.match(popup, /toggleMute\(\)/);
  assert.match(popup, /meeting\?\.muted/);
  assert.match(popup, /setMeetingOutputMuted\(desired\)/);
  assert.match(registry, /set_meeting_output_muted/);
  assert.match(nativeSource, /Mute is an output gate, never a third microphone source/);
  assert.match(api, /muted: boolean/);
});
test("Quick Access native hide uses Window API, not Webview.hide", () => {
  const runtime = source("src/app/runtime/quickAccessRuntime.ts");
  assert.match(runtime, /import \{ Window \} from "@tauri-apps\/api\/window"/);
  assert.match(runtime, /Window\.getByLabel\(QUICK_ACCESS_WINDOW_LABEL\)/);
  assert.doesNotMatch(runtime, /WebviewWindow\.getByLabel/);
});


test("Pre-Start preferred voice source has one Rust owner across Meeting and Quick Access", () => {
  const native = source("src-tauri/src/commands/meeting_session/audio_source.rs");
  const status = source("src-tauri/src/commands/meeting_session/preflight.rs");
  const meetingPage = source("src/pages/Meeting.svelte");
  const registry = source("src-tauri/src/commands/registry.rs");
  assert.match(native, /static PREFER_ORIGINAL: AtomicBool/);
  assert.match(native, /fn set_preferred_source\(source: &str\)/);
  assert.match(status, /super::audio_source::preferred_source\(\)/);
  assert.match(meetingPage, /selectMeetingPreferredAudioSource\(source\)/);
  assert.match(popup, /selectMeetingPreferredAudioSource\(requested\)/);
  assert.match(registry, /set_meeting_preferred_audio_source/);
  assert.match(popup, /next\.meeting\.lifecycle === "idle"/);
});

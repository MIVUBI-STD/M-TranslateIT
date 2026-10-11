import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
const tauri = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const mainCapability = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/default.json", import.meta.url), "utf8"));
const overlayCapability = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/translation-overlay.json", import.meta.url), "utf8"));
const closeRuntime = readFileSync(new URL("../../src/app/runtime/nativeCloseRuntime.ts", import.meta.url), "utf8");
const overlayRuntime = readFileSync(new URL("../../src/app/runtime/translationOverlayRuntime.ts", import.meta.url), "utf8");
const overlayPage = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");

test("floating caption native window keeps readability contract", () => {
  const overlay = tauri.app.windows.find((window) => window.label === "translation-overlay"); assert.ok(overlay);
  assert.equal(overlay.alwaysOnTop, true); assert.equal(overlay.skipTaskbar, true); assert.equal(overlay.visible, false); assert.equal(overlay.decorations, false); assert.equal(overlay.resizable, false);
  assert.ok(overlay.maxWidth >= 760, "native window must support wide preset");
});
test("shutdown destroys overlay before main window", () => {
  assert.match(closeRuntime, /destroyTranslationOverlay\(\)/); assert.match(closeRuntime, /getCurrentWindow\(\)\.destroy\(\)/);
});
test("caption delivery is in-memory plus live event and ready replay", () => {
  assert.match(overlayRuntime, /let latestPresented:/);
  assert.match(overlayRuntime, /subscribeOverlayReady/);
  assert.match(overlayPage, /listen<TranslationOverlayPayload>/);
  assert.match(overlayPage, /TRANSLATION_OVERLAY_READY_EVENT/);
  assert.doesNotMatch(overlayRuntime, /writeLatestOverlayCaption|localStorage/);
});
test("overlay and main window permissions remain split by least privilege", () => {
  assert.deepEqual(mainCapability.windows, ["main"]);
  assert.deepEqual(overlayCapability.windows, ["translation-overlay"]);
  for (const permission of ["core:window:allow-set-size", "core:window:allow-set-position", "core:window:allow-start-dragging", "core:window:allow-set-ignore-cursor-events"]) {
    assert.ok(overlayCapability.permissions.includes(permission), permission);
    assert.equal(mainCapability.permissions.includes(permission), false, permission);
  }
  assert.ok(mainCapability.permissions.includes("core:window:allow-hide"));
  assert.ok(overlayCapability.permissions.includes("core:window:allow-hide"));
  for (const permission of ["core:window:allow-show", "core:window:allow-destroy"]) {
    assert.ok(mainCapability.permissions.includes(permission), permission);
    assert.equal(overlayCapability.permissions.includes(permission), false, permission);
  }
});
test("overlay includes high contrast, work area placement and explicit hide", () => {
  assert.match(overlayPage, /forced-colors: active/); assert.match(overlayPage, /Hide floating caption/); assert.match(overlayPage, /monitor\.workArea/);
});

test("Hide/Show cannot retain old private caption content", () => {
  assert.match(overlayRuntime, /TRANSLATION_OVERLAY_CLEAR_EVENT/);
  assert.match(overlayPage, /function clearCaption\(\)/);
  assert.match(overlayPage, /unlistenClear/);
});

test("overlay adapts text bounds and reconciles monitor DPI without global shortcuts", () => {
  assert.match(overlayPage, /onScaleChanged/);
  assert.match(overlayPage, /fitOverlayWidth/);
  assert.match(overlayPage, /onOverlayKeydown/);
  assert.match(overlayPage, /captionText\?\.scrollHeight/);
});

test("caption payload or clear is sent before exposing the native window", () => {
  const publish = overlayRuntime.split("export async function publishTranslationOverlay")[1]?.split("export async function publishLatestMeetingOverlay")[0] ?? "";
  const show = overlayRuntime.split("export async function showTranslationOverlay")[1]?.split("export async function hideTranslationOverlay")[0] ?? "";
  assert.ok(publish.indexOf("await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, normalized)") < publish.indexOf("await window.show()"));
  assert.ok(show.indexOf("TRANSLATION_OVERLAY_CLEAR_EVENT") < show.indexOf("await window.show()"));
});

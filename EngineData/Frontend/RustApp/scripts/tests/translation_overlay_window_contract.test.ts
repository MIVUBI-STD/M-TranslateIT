import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
const tauri = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const mainCapability = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/default.json", import.meta.url), "utf8"));
const overlayCapability = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/translation-overlay.json", import.meta.url), "utf8"));
const closeRuntime = readFileSync(new URL("../../src/app/runtime/nativeCloseRuntime.ts", import.meta.url), "utf8");
const overlayRuntime = readFileSync(new URL("../../src/app/runtime/translationOverlayRuntime.ts", import.meta.url), "utf8");
const overlayPage = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");
const meetingLiveController = readFileSync(new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url), "utf8");

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

test("latest caption is delivered before requesting native Show", () => {
  const publish = overlayRuntime.split("export async function publishTranslationOverlay")[1]?.split("export async function publishLatestMeetingOverlay")[0] ?? "";
  const show = overlayRuntime.split("export async function showTranslationOverlay")[1]?.split("export async function hideTranslationOverlay")[0] ?? "";
  assert.ok(publish.indexOf("await emitTo(OVERLAY_WINDOW_LABEL, TRANSLATION_OVERLAY_EVENT, normalized)") < publish.indexOf("await requestNativeOverlayVisible(true)"));
  assert.ok(show.indexOf("TRANSLATION_OVERLAY_CLEAR_EVENT") < show.indexOf("await requestNativeOverlayVisible(true)"));
});

test("native positioning is serialized, DPI-constrained, and never persists an automatic move", () => {
  assert.match(overlayPage, /layoutQueue = layoutQueue\.then\(async \(\) =>/);
  assert.match(overlayPage, /if \(disposed \|\| layoutPending\) return layoutQueue/);
  assert.match(overlayPage, /fitOverlayHeight\(currentHeight/);
  assert.match(overlayPage, /isProgrammaticOverlayMove\(payload, lastAutomaticPosition\)/);
  assert.match(overlayPage, /new PhysicalPosition\(next\.x, next\.y\)/);
});

test("caption resize and native subscriptions have bounded lifecycle ownership", () => {
  assert.match(overlayPage, /new ResizeObserver/);
  assert.match(overlayPage, /observer\.disconnect\(\)/);
  assert.match(overlayPage, /const track = async/);
  assert.match(overlayPage, /if \(disposed\) \{ unlisten\(\); return false; \}/);
  assert.match(overlayPage, /for \(const unlisten of unlisteners\) unlisten\(\)/);
  assert.match(overlayPage, /recordOverlayError\("Floating caption setup is unavailable\."\)/);
});

test("screen-reader caption and bounded transcript regions are keyboard reachable", () => {
  assert.match(overlayPage, /role="region" tabindex="0" aria-label="Current translation"/);
  assert.match(overlayPage, /role="region" tabindex="0" aria-label="Recent committed translations"/);
  assert.match(overlayPage, /aria-live=\{captionsPaused \? "off" : "polite"\}/);
  assert.match(overlayPage, /historyBody\?\.focus\(\)/);
  assert.match(overlayPage, /historyButton\?\.focus\(\)/);
});

test("stale Meeting review responses cannot restore private text after Hide or session rollover", () => {
  assert.match(overlayPage, /requestedSession = overlayMeetingSession\(caption\)/);
  assert.match(overlayPage, /requestId !== historyRequestId/);
  assert.match(overlayPage, /snapshot\.session_id === requestedSession/);
  assert.match(overlayPage, /historyRequestId \+= 1/);
  assert.match(overlayPage, /clearCaption\(\);\s*await hideTranslationOverlay\(\)/);
});

test("native preference delivery errors remain bounded and user recoverable", () => {
  assert.match(overlayPage, /Floating caption preferences could not be applied/);
  assert.match(overlayPage, /Floating caption size could not be changed/);
  assert.match(overlayPage, /recordOverlayError/);
});

test("Hide from the overlay revokes the main window transient caption", () => {
  assert.match(overlayRuntime, /await emit\(TRANSLATION_OVERLAY_HIDDEN_EVENT\)/);
  assert.match(overlayRuntime, /listen\(TRANSLATION_OVERLAY_HIDDEN_EVENT/);
  assert.match(overlayRuntime, /if \(readOverlayPreferences\(\)\.visibility === "hidden"\) latestPresented = null/);
  assert.match(overlayRuntime, /if \(readOverlayPreferences\(\)\.visibility === "hidden"\) latestPresented = null/);
  assert.match(overlayPage, /mayPresentOverlayCaption\(readOverlayPreferences\(\), next\)/);
});
test("hidden preferences clear stale private overlay content", () => {
  assert.match(overlayPage, /if \(next\.visibility === "hidden"\) \{\s*clearCaption\(\)/);
  assert.match(overlayRuntime, /return \(\) => \{ stopReady\(\); stopHidden\(\); \}/);
});
test("native visibility is serialized and coalesces to the latest user intent", () => {
  assert.match(overlayRuntime, /let pendingNativeVisibility: boolean \| null = null/);
  assert.match(overlayRuntime, /if \(nativeVisibilityTask\) return nativeVisibilityTask/);
  assert.match(overlayRuntime, /while \(pendingNativeVisibility !== null\)/);
  assert.match(overlayRuntime, /readOverlayPreferences\(\)\.visibility !== "hidden"/);
  assert.match(overlayRuntime, /await requestNativeOverlayVisible\(readOverlayPreferences\(\)\.visibility !== "hidden"\)/);
});

test("Meeting end and reset revoke the previous caption, never a newer Text overlay", () => {
  assert.match(meetingLiveController, /void clearLatestMeetingOverlay\(\)/);
  assert.match(meetingLiveController, /await clearLatestMeetingOverlay\(\)/);
  assert.match(overlayRuntime, /if \(previous\?\.source !== "meeting"\) return/);
  assert.match(overlayRuntime, /onlyRevision: previous\.revision/);
  assert.match(overlayPage, /caption\?\.revision !== onlyRevision && pendingCaption\?\.revision !== onlyRevision/);
  assert.match(overlayRuntime, /if \(!caption\) \{\s*await clearLatestMeetingOverlay\(\)/);
});

test("out-of-order preferences events reconcile from the current persisted state", () => {
  assert.match(overlayPage, /TRANSLATION_OVERLAY_PREFERENCES_EVENT, \(\) =>/);
  assert.match(overlayPage, /applyPreferences\(readOverlayPreferences\(\)\)/);
  assert.doesNotMatch(overlayPage, /applyPreferences\(event\.payload\)/);
});

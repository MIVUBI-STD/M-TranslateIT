import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
const tauri = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const capability = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/default.json", import.meta.url), "utf8"));
const closeRuntime = readFileSync(new URL("../../src/app/runtime/nativeCloseRuntime.ts", import.meta.url), "utf8");
const overlayRuntime = readFileSync(new URL("../../src/app/runtime/translationOverlayRuntime.ts", import.meta.url), "utf8");
const overlayPage = readFileSync(new URL("../../src/pages/TranslationOverlay.svelte", import.meta.url), "utf8");

test("floating caption native window keeps readability contract", () => {
  const overlay = tauri.app.windows.find((window) => window.label === "translation-overlay"); assert.ok(overlay);
  assert.equal(overlay.alwaysOnTop, true); assert.equal(overlay.skipTaskbar, true); assert.equal(overlay.visible, false); assert.equal(overlay.decorations, false); assert.equal(overlay.resizable, false);
});
test("shutdown destroys overlay before main window", () => {
  assert.match(closeRuntime, /destroyTranslationOverlay\(\)/); assert.match(closeRuntime, /getCurrentWindow\(\)\.destroy\(\)/);
});
test("caption delivery is durable state plus realtime event", () => {
  assert.match(overlayRuntime, /writeLatestOverlayCaption\(normalized\)/); assert.match(overlayPage, /listen<TranslationOverlayPayload>/); assert.match(overlayPage, /readLatestOverlayCaption\(\)/);
});
test("overlay permissions cover lifecycle and placement", () => {
  for (const permission of ["core:window:allow-show","core:window:allow-hide","core:window:allow-set-size","core:window:allow-set-position","core:window:allow-destroy"]) assert.ok(capability.permissions.includes(permission), permission);
});
test("overlay includes high contrast, work area placement and explicit hide", () => {
  assert.match(overlayPage, /forced-colors: active/); assert.match(overlayPage, /Hide floating caption/); assert.match(overlayPage, /monitor\.workArea/);
});

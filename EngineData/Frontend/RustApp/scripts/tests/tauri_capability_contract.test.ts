import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function capability(name: string) {
  return JSON.parse(
    readFileSync(
      new URL(`../../src-tauri/capabilities/${name}`, import.meta.url),
      "utf8",
    ),
  ) as {
    identifier: string;
    windows: string[];
    permissions: string[];
  };
}

test("main window capability stays least-privilege", () => {
  const main = capability("default.json");
  assert.deepEqual(main.windows, ["main"]);
  assert.ok(main.permissions.includes("core:default"));
  assert.ok(main.permissions.includes("core:window:allow-destroy"));
  assert.ok(main.permissions.includes("core:window:allow-show"));
  assert.ok(main.permissions.includes("core:window:allow-hide"));
  assert.equal(main.permissions.includes("core:window:allow-set-size"), false);
  assert.equal(main.permissions.includes("core:window:allow-set-position"), false);
  assert.equal(main.permissions.includes("core:window:allow-start-dragging"), false);
});

test("translation overlay owns only overlay window mutation permissions", () => {
  const overlay = capability("translation-overlay.json");
  assert.deepEqual(overlay.windows, ["translation-overlay"]);
  assert.ok(overlay.permissions.includes("core:default"));
  assert.ok(overlay.permissions.includes("core:window:allow-set-size"));
  assert.ok(overlay.permissions.includes("core:window:allow-set-position"));
  assert.ok(overlay.permissions.includes("core:window:allow-start-dragging"));
  assert.ok(overlay.permissions.includes("core:window:allow-hide"));
  assert.equal(overlay.permissions.includes("core:window:allow-destroy"), false);
  assert.equal(overlay.permissions.includes("core:window:allow-show"), false);
});

test("capabilities do not overlap window ownership", () => {
  const main = capability("default.json");
  const overlay = capability("translation-overlay.json");
  assert.equal(main.windows.includes("translation-overlay"), false);
  assert.equal(overlay.windows.includes("main"), false);
});

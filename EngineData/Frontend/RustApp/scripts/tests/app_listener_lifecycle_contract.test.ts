import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("async listener installation cleans up immediately if App disposes during await", () => {
  assert.match(app, /const stop = await installNativeCloseGuard/);
  assert.match(app, /const stop = await applicationRuntimeApi\.subscribe/);
  assert.match(app, /startVoiceBuildRuntimeSync/);
  assert.ok((app.match(/if \(disposed\) \{\s*stop\(\);\s*return;\s*\}/g) ?? []).length >= 2);
});

test("mounted listeners still retain explicit teardown handles", () => {
  assert.match(app, /unlistenClose = stop/);
  assert.match(app, /unlistenApplicationRuntime = stop/);
  assert.match(app, /stopVoiceBuildRuntimeSync/);
  assert.match(app, /unlistenClose\?\.\(\)/);
  assert.match(app, /unlistenApplicationRuntime\?\.\(\)/);
  assert.match(app, /stopVoiceBuildRuntimeSync\(\)/);
});

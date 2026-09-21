import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const command = readFileSync(new URL("../../src-tauri/src/commands/text_translation.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
const facade = readFileSync(new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url), "utf8");
const appTree = [
  readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8"),
  readFileSync(new URL("../../src/main.ts", import.meta.url), "utf8"),
].join("\n");

test("quick translate is exposed as an API without installing an activation mechanism", () => {
  assert.match(registry, /quick_translate_text/);
  assert.match(api, /quickTranslateText/);
  assert.match(facade, /runProductQuickTranslation/);
  assert.doesNotMatch(appTree, /quickTranslateText|runProductQuickTranslation/);
});

test("quick translate reuses the canonical text translation core", () => {
  assert.match(command, /translate_with_persistent_helper\(&source, "quick_text"\)/);
  assert.match(command, /translate_with_persistent_helper\(&source, "standalone_text"\)\.0/);
  assert.equal((command.match(/send_helper_worker_task\("translate"/g) ?? []).length, 2);
});

test("quick translate does not add clipboard, keyboard hook, or global shortcut ownership", () => {
  const combined = [command, api, facade].join("\n");
  assert.doesNotMatch(combined, /clipboard|globalShortcut|register_all|keyboard hook|SetWindowsHookEx/i);
});

test("quick translate returns the exact configured language direction without a second settings read", () => {
  assert.equal((command.match(/let settings = load_settings\(\);/g) ?? []).length, 2);
  assert.match(command, /source_language: String/);
  assert.match(command, /target_language: String/);
  assert.match(api, /source_language: string/);
  assert.match(api, /target_language: string/);
});

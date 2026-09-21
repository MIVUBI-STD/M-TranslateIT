import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const command = readFileSync(new URL("../../src-tauri/src/commands/text_translation.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
const productivity = readFileSync(new URL("../../src/components/runtime/ProductivityActions.svelte", import.meta.url), "utf8");
const facade = readFileSync(new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url), "utf8");

test("quick translate is explicit user-facing UI over the canonical runtime API", () => {
  assert.match(registry, /quick_translate_text/);
  assert.match(api, /quickTranslateText/);
  assert.match(productivity, /Quick Translate Clipboard/);
  assert.match(productivity, /navigator\.clipboard\?\.readText/);
  assert.match(productivity, /runtimeApi\.quickTranslateText/);
  assert.doesNotMatch(facade, /runProductQuickTranslation|ProductQuickTranslationResult/);
});

test("quick translate reuses the canonical text translation core", () => {
  assert.match(command, /translate_with_persistent_helper\(&source, "quick_text"\)/);
  assert.match(command, /translate_with_persistent_helper\(&source, "standalone_text"\)\.0/);
  assert.equal((command.match(/send_helper_worker_task\("translate"/g) ?? []).length, 2);
});

test("quick translate has no clipboard watcher or default global OS shortcut", () => {
  assert.doesNotMatch(productivity, /setInterval|clipboardchange|globalShortcut|register_all|SetWindowsHookEx/i);
  assert.match(productivity, /Ctrl\+K/);
  assert.doesNotMatch([command, api].join("\n"), /globalShortcut|SetWindowsHookEx/i);
});

test("quick translate returns the exact configured language direction", () => {
  assert.match(command, /source_language: String/);
  assert.match(command, /target_language: String/);
  assert.match(api, /source_language: string/);
  assert.match(api, /target_language: string/);
});

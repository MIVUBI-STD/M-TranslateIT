import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const registry = source("src-tauri/src/commands/registry.rs");
const commands = source("src-tauri/src/commands/text_translation.rs");
const api = source("src/app/bridge/runtimeApi.ts");
const product = source("src/app/bridge/runtimeProductFacade.ts");
const productivity = source("src/components/runtime/ProductivityActions.svelte");
const shortcuts = source("src/app/runtime/productCommandRegistry.ts");

test("quick clipboard and alternative wording have no frontend or native command entrypoints", () => {
  assert.doesNotMatch(shortcuts, /quick-translate-clipboard|quickTranslateClipboard/);
  assert.doesNotMatch(productivity, /quickTranslateClipboard|publishTranslationOverlay|navigator\.clipboard/);
  assert.doesNotMatch(api, /quickTranslateText|translateTextAlternative/);
  assert.doesNotMatch(product, /runProductTranslationAlternative/);
  assert.doesNotMatch(registry, /quick_translate_text|translate_text_alternative/);
  assert.doesNotMatch(commands, /pub fn quick_translate_text|pub fn translate_text_alternative/);
});
test("the remaining Text function uses exactly one canonical worker request", () => {
  assert.match(commands, /translate_with_persistent_helper\(&source, "standalone_text"\)\.0/);
  assert.match(api, /async translateText\(source: string\)/);
});

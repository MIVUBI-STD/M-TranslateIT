import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const dialog = readFileSync(
  new URL("../../src/components/runtime/NativeCloseDialog.svelte", import.meta.url),
  "utf8",
);

test("native close dialog is controlled by close controller state", () => {
  assert.match(dialog, /<Dialog\.Root \{open\} onOpenChange=/);
  assert.match(dialog, /onOpenChange\?: \(open: boolean\) => void/);
  assert.equal(dialog.includes("$bindable"), false);
  assert.equal(dialog.includes("bind:open"), false);
});

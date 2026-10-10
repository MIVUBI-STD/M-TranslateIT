import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(
  new URL("../../../../../.github/workflows/code-health.yml", import.meta.url),
  "utf8",
);

test("manual Code Health executes all three source domains on the selected SHA", () => {
  assert.match(workflow, /^on:\n  workflow_dispatch:/m);
  assert.match(workflow, /name: Select complete manual source proof/);
  for (const domain of ["frontend", "python", "rust"]) {
    assert.ok(workflow.includes('echo "' + domain + '=true"'), domain);
    assert.ok(workflow.includes("needs.changes.outputs." + domain + " == 'true'"), domain);
  }
  assert.match(workflow, /proof-summary:/);
  assert.match(workflow, /Exact SHA proof summary/);
  assert.doesNotMatch(workflow, /PUSH_BASE|PR_BASE|git diff --name-only|github\.event\.before/);
});

test("full manual source proof requires every relevant Linux and Windows result", () => {
  assert.match(workflow, /Enforce matching changed-domain proof/);
  for (const marker of [
    'require_success "$FRONTEND_CHANGED" "$FRONTEND_RESULT"',
    'require_success "$PYTHON_CHANGED" "$PYTHON_RESULT"',
    'require_success "$PYTHON_CHANGED" "$PYTHON_WINDOWS_RESULT"',
    'require_success "$RUST_CHANGED" "$RUST_RESULT"',
    'require_success "$RUST_CHANGED" "$RUST_WINDOWS_RESULT"',
  ]) {
    assert.ok(workflow.includes(marker), marker);
  }
  assert.match(workflow, /exit 1/);
});

test("worker protocol keeps both Python and frontend contract proof", () => {
  assert.match(workflow, /run: npm run test:frontend-runtime/);
  assert.match(workflow, /run: npm run validate:bridge-contract/);
  assert.match(workflow, /run: npm run validate:tauri-command-args/);
  assert.match(workflow, /name: Python source and unit health/);
  assert.match(workflow, /name: Python Windows source and unit health/);
});

test("skipped source domains are not evidence", () => {
  assert.match(workflow, /A skipped domain is not evidence for that domain/);
  assert.match(workflow, /Only completed matching jobs on this exact SHA/);
});

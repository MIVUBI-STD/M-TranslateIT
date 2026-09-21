import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(
  new URL("../../../../../.github/workflows/code-health.yml", import.meta.url),
  "utf8",
);

test("Code Health supports explicit exact-SHA proof runs", () => {
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /proof-summary:/);
  assert.match(workflow, /Exact SHA proof summary/);
});

test("Code Health aggregate gate fails when a changed domain lacks success", () => {
  assert.match(workflow, /Enforce matching changed-domain proof/);
  assert.match(workflow, /require_success "\$FRONTEND_CHANGED"/);
  assert.match(workflow, /require_success "\$PYTHON_CHANGED"/);
  assert.match(workflow, /require_success "\$RUST_CHANGED"/);
  assert.match(workflow, /exit 1/);
});

test("Code Health summary states skipped jobs are not proof", () => {
  assert.match(
    workflow,
    /A skipped domain is not evidence for that domain/,
  );
});

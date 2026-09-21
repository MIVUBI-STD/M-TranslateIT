import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function workflow(name: string): string {
  return readFileSync(
    new URL(`../../../../../.github/workflows/${name}`, import.meta.url),
    "utf8",
  );
}

const singleJobProofWorkflows = [
  "repository-verify.yml",
  "milmmt-repo-contract.yml",
  "asr-quality-contract.yml",
  "tts-quality-contract.yml",
  "quality-readiness-contract.yml",
  "workerruntime-lock.yml",
];

test("important source-contract workflows support manual exact-SHA proof", () => {
  for (const name of singleJobProofWorkflows) {
    const source = workflow(name);
    assert.match(source, /workflow_dispatch:/, name);
    assert.match(source, /Write exact-SHA proof summary/, name);
    assert.match(source, /GITHUB_SHA/, name);
    assert.match(source, /not TARGET_WINDOWS native acceptance/, name);
  }
});

test("manual R3 release proof includes controlled Windows payload", () => {
  const source = workflow("release-payload-verify.yml");
  assert.match(source, /workflow_dispatch:/);
  assert.match(source, /EVENT_NAME" == "workflow_dispatch"/);
  assert.match(source, /controlled=true/);
  assert.match(
    source,
    /github\.event_name == 'push' \|\| github\.event_name == 'workflow_dispatch'/,
  );
  assert.match(source, /Exact SHA release proof summary/);
  assert.match(source, /Enforce release proof completeness/);
  assert.match(
    source,
    /Controlled Windows payload proof required but result was/,
  );
});

test("Code Health retains aggregate exact-SHA proof semantics", () => {
  const source = workflow("code-health.yml");
  assert.match(source, /workflow_dispatch:/);
  assert.match(source, /Exact SHA proof summary/);
  assert.match(source, /Enforce matching changed-domain proof/);
});

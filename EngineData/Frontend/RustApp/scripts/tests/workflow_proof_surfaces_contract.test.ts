import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function workflow(name: string): string {
  return readFileSync(new URL("../../../../../.github/workflows/" + name, import.meta.url), "utf8");
}

const sourceWorkflows = [
  "repository-verify.yml", "milmmt-repo-contract.yml", "asr-quality-contract.yml",
  "tts-quality-contract.yml", "quality-readiness-contract.yml", "workerruntime-lock.yml",
];

test("repository and quality contracts provide manual exact-SHA evidence", () => {
  for (const name of sourceWorkflows) {
    const source = workflow(name);
    assert.match(source, /workflow_dispatch:/, name);
    assert.match(source, /Write exact-SHA proof summary/, name);
    assert.match(source, /GITHUB_SHA/, name);
    assert.match(source, /not TARGET_WINDOWS native acceptance/, name);
  }
});

test("manual R3 release always requires controlled Windows payload proof", () => {
  const source = workflow("release-payload-verify.yml");
  for (const marker of [
    "workflow_dispatch:", "Require controlled manual release proof",
    'echo "controlled=true"', "if: needs.payload-scope.outputs.controlled == 'true'",
    "Exact SHA release proof summary", "Enforce release proof completeness",
    "Controlled Windows payload proof required but result was",
  ]) {
    assert.ok(source.includes(marker), marker);
  }
  assert.doesNotMatch(source, /PUSH_BASE|git diff --name-only|github\.event_name == 'push'/);
});

test("Code Health retains complete manual exact-SHA gate", () => {
  const source = workflow("code-health.yml");
  assert.match(source, /Select complete manual source proof/);
  assert.match(source, /Exact SHA proof summary/);
  assert.match(source, /Enforce matching changed-domain proof/);
});

test("all eight workflows forbid automatic push, PR, schedule and release triggers", () => {
  for (const name of [...sourceWorkflows, "code-health.yml", "release-payload-verify.yml"]) {
    const source = workflow(name);
    const trigger = source.match(/^on:\n([\s\S]*?)(?=^(?:permissions|concurrency):)/m)?.[1];
    assert.ok(trigger, name + ": explicit workflow event block missing");
    assert.match(trigger, /^  workflow_dispatch:/m, name);
    assert.doesNotMatch(trigger, /^  (?:push|pull_request|pull_request_target|schedule|workflow_run|release|create|delete|merge_group|issue_comment):/m, name);
  }
});

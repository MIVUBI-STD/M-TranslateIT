import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(appRoot, "..", "..", "..");
const workflow = readFileSync(resolve(repoRoot, ".github", "workflows", "code-health.yml"), "utf8");

// Literal cross-language test references are checked separately by
// validate_test_reference_reachability.mjs. Manual Code Health selects all
// frontend, Rust and Python source suites; no second path list is maintained.
for (const marker of [
  'echo "frontend=true"', 'echo "python=true"', 'echo "rust=true"',
  "needs.changes.outputs.frontend == 'true'",
  "needs.changes.outputs.python == 'true'",
  "needs.changes.outputs.rust == 'true'",
  "npm run test:frontend-runtime", "name: Python source and unit health",
]) {
  if (!workflow.includes(marker)) {
    console.error("Manual cross-language Code Health routing missing: " + marker);
    process.exit(1);
  }
}
if (/PUSH_BASE|PR_BASE|git diff --name-only/.test(workflow)) {
  console.error("Manual Code Health must not retain unreachable push/PR path routing.");
  process.exit(1);
}
console.log("[test-ci-routing] manual Code Health covers frontend, Rust and Python source domains.");

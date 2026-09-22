import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(appRoot, "..", "..", "..");
const testsRoot = resolve(appRoot, "scripts", "tests");
const workflow = readFileSync(resolve(repoRoot, ".github", "workflows", "code-health.yml"), "utf8");

function collectTests(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectTests(path));
    else if (entry.isFile() && entry.name.endsWith(".test.ts")) files.push(path);
  }
  return files;
}

const referencedPython = new Set();

for (const testPath of collectTests(testsRoot)) {
  const source = readFileSync(testPath, "utf8");
  for (const match of source.matchAll(/new URL\(\s*["'`]([^"'`]+\.py)["'`]/g)) {
    if (match[1].includes("${")) continue;
    const absolute = resolve(dirname(testPath), match[1]);
    if (!existsSync(absolute)) continue;
    const relative = absolute
      .slice(repoRoot.length + 1)
      .replaceAll("\\", "/");
    if (relative.startsWith("EngineData/Backend/LocalWorker/WorkerRuntime/")) {
      referencedPython.add(relative);
    }
  }
}

const missing = [...referencedPython]
  .filter((path) => !workflow.includes(path))
  .sort();

if (missing.length > 0) {
  console.error("Cross-language frontend-test CI routing failed:");
  for (const path of missing) {
    console.error(`- frontend tests reference ${path}, but Code Health does not explicitly cross-route it to frontend proof`);
  }
  process.exit(1);
}

console.log(
  `[test-ci-routing] ${referencedPython.size} Python sources referenced by frontend tests are cross-routed to frontend Code Health.`,
);

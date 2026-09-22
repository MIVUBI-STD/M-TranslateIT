import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const testsRoot = resolve(appRoot, "scripts", "tests");

function collect(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path));
    else if (entry.isFile() && entry.name.endsWith(".test.ts")) files.push(path);
  }
  return files;
}

function candidates(testPath, relativeRef) {
  const base = resolve(dirname(testPath), relativeRef);
  if (extname(base)) return [base];
  return [
    base,
    `${base}.ts`,
    `${base}.svelte`,
    `${base}.rs`,
    `${base}.json`,
    `${base}.yml`,
    resolve(base, "index.ts"),
    resolve(base, "index.svelte"),
  ];
}

const failures = [];
for (const testPath of collect(testsRoot)) {
  const source = readFileSync(testPath, "utf8");
  const refs = new Set();

  for (const match of source.matchAll(/\bfrom\s+["'](\.\.?\/[^"']+)["']/g)) {
    refs.add(match[1]);
  }
  for (const match of source.matchAll(/new URL\(\s*["'`](\.\.?\/[^"'`]+)["'`]/g)) {
    if (!match[1].includes("${")) refs.add(match[1]);
  }

  for (const relativeRef of refs) {
    if (!candidates(testPath, relativeRef).some((path) => existsSync(path))) {
      failures.push(`${testPath}: missing referenced source ${relativeRef}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Test reference reachability failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[test-reference-reachability] all static test source references resolve.");

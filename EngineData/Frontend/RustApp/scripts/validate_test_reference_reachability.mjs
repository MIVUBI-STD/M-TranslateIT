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

function candidates(baseDirectory, relativeRef) {
  const base = resolve(baseDirectory, relativeRef);
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
  const testDirectory = dirname(testPath);
  const refs = new Map();

  // Resolve only a literal directory alias whose URL base is import.meta.url.
  // Other dynamic URL roots cannot be safely interpreted as test-file paths.
  const directoryAliases = new Map();
  for (const match of source.matchAll(/\bconst\s+([A-Za-z_$][\w$]*)\s*=\s*new URL\(\s*["'`](\.\.?\/[^"'`]+)["'`]\s*,\s*import\.meta\.url\s*\)/g)) {
    if (match[2].endsWith("/")) {
      directoryAliases.set(match[1], resolve(testDirectory, match[2]));
    }
  }

  function addReference(baseDirectory, relativeRef) {
    refs.set(baseDirectory + "\0" + relativeRef, { baseDirectory, relativeRef });
  }

  for (const match of source.matchAll(/\bfrom\s+["'](\.\.?\/[^"']+)["']/g)) {
    addReference(testDirectory, match[1]);
  }

  // For URL literals, the second argument owns the location. A URL relative
  // to a named root must not be falsely resolved against the test file.
  for (const match of source.matchAll(/new URL\(\s*["'`](\.\.?\/[^"'`]+)["'`]\s*,\s*(import\.meta\.url|[A-Za-z_$][\w$]*)\s*\)/g)) {
    if (match[1].includes("$" + "{")) continue;
    const baseDirectory = match[2] === "import.meta.url"
      ? testDirectory
      : directoryAliases.get(match[2]);
    if (baseDirectory) addReference(baseDirectory, match[1]);
  }

  for (const { baseDirectory, relativeRef } of refs.values()) {
    if (!candidates(baseDirectory, relativeRef).some((path) => existsSync(path))) {
      failures.push(testPath + ": missing referenced source " + relativeRef);
    }
  }
}

if (failures.length > 0) {
  console.error("Test reference reachability failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[test-reference-reachability] all static test source references resolve.");

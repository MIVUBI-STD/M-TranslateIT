import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = resolve(appRoot, "src");
const packageJson = JSON.parse(readFileSync(resolve(appRoot, "package.json"), "utf8"));
const declared = new Set([
  ...Object.keys(packageJson.dependencies ?? {}),
  ...Object.keys(packageJson.devDependencies ?? {}),
]);
const runtimeDependencies = new Set(Object.keys(packageJson.dependencies ?? {}));
const nodeBuiltins = new Set([
  "assert", "buffer", "child_process", "crypto", "events", "fs", "http", "https",
  "os", "path", "stream", "url", "util", "worker_threads",
]);

function collect(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path));
    else if (entry.isFile() && [".ts", ".svelte"].includes(extname(entry.name))) files.push(path);
  }
  return files;
}

function packageName(specifier) {
  if (specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("node:")) return null;
  const [first, second] = specifier.split("/");
  return first?.startsWith("@") ? [first, second].filter(Boolean).join("/") : first;
}

const imported = new Set();
for (const path of collect(sourceRoot)) {
  const body = readFileSync(path, "utf8");
  for (const pattern of [
    /\bfrom\s+["']([^"']+)["']/g,
    /\bimport\s+["']([^"']+)["']/g,
    /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g,
  ]) {
    for (const match of body.matchAll(pattern)) {
      const name = packageName(match[1]);
      if (name) imported.add(name);
    }
  }
}

const undeclared = [...imported]
  .filter((name) => !declared.has(name) && !nodeBuiltins.has(name))
  .sort();
const unusedRuntime = [...runtimeDependencies]
  .filter((name) => !imported.has(name))
  .sort();

if (undeclared.length || unusedRuntime.length) {
  console.error("Frontend dependency usage validation failed:");
  for (const name of undeclared) console.error(`- imported but undeclared: ${name}`);
  for (const name of unusedRuntime) console.error(`- runtime dependency never imported by src: ${name}`);
  process.exit(1);
}

console.log(`[dependency-usage] ${imported.size} external source packages accounted for; ${runtimeDependencies.size} runtime dependencies are used.`);

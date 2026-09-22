import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = resolve(appRoot, "src");
const runtimeApiPath = resolve(sourceRoot, "app", "bridge", "runtimeApi.ts");

function collect(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path));
    else if (entry.isFile() && [".ts", ".svelte"].includes(extname(entry.name))) files.push(path);
  }
  return files;
}

const runtimeApiSource = readFileSync(runtimeApiPath, "utf8");
const methodNames = new Set(
  [...runtimeApiSource.matchAll(/^\s{2}(?:async\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(/gm)]
    .map((match) => match[1])
    .filter((name) => name !== "constructor"),
);

const consumers = collect(sourceRoot).filter((path) => path !== runtimeApiPath);
const usage = new Map([...methodNames].map((name) => [name, []]));

for (const path of consumers) {
  const body = readFileSync(path, "utf8");
  for (const name of methodNames) {
    const pattern = new RegExp(`\\bruntimeApi\\.${name}\\b`);
    if (pattern.test(body)) usage.get(name).push(relative(appRoot, path).replaceAll("\\", "/"));
  }
}

const unused = [...usage.entries()]
  .filter(([, paths]) => paths.length === 0)
  .map(([name]) => name)
  .sort();

if (unused.length > 0) {
  console.error("Runtime API method reachability failed; bridge wrappers have no frontend consumer:");
  for (const name of unused) console.error(`- runtimeApi.${name}`);
  process.exit(1);
}

console.log(`[runtime-api-usage] ${methodNames.size} runtimeApi methods have frontend consumers.`);

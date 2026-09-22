import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = resolve(appRoot, "src");
const trackedExtensions = new Set([".ts", ".svelte"]);

function collectModules(directory) {
  const modules = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) modules.push(...collectModules(path));
    else if (
      entry.isFile()
      && trackedExtensions.has(extname(entry.name))
      && !entry.name.endsWith(".d.ts")
    ) {
      modules.push(path);
    }
  }
  return modules;
}

const modules = collectModules(sourceRoot);
const moduleSet = new Set(modules);

function resolveRelativeImport(importer, specifier) {
  if (!specifier.startsWith(".")) return null;
  const base = resolve(dirname(importer), specifier);
  const candidates = [
    base,
    `${base}.ts`,
    `${base}.svelte`,
    resolve(base, "index.ts"),
    resolve(base, "index.svelte"),
  ];
  return candidates.find((candidate) => moduleSet.has(candidate)) ?? null;
}

function importedModules(path) {
  const body = readFileSync(path, "utf8");
  const specifiers = new Set();
  for (const pattern of [
    /\bfrom\s+["'](\.[^"']+)["']/g,
    /\bimport\s+["'](\.[^"']+)["']/g,
    /\bimport\s*\(\s*["'](\.[^"']+)["']\s*\)/g,
  ]) {
    for (const match of body.matchAll(pattern)) specifiers.add(match[1]);
  }

  const resolved = [];
  const unresolved = [];
  for (const specifier of specifiers) {
    const target = resolveRelativeImport(path, specifier);
    if (target) {
      resolved.push(target);
      continue;
    }

    const extension = extname(specifier);
    if (!extension || extension === ".ts" || extension === ".svelte") {
      unresolved.push(specifier);
    }
  }
  return { resolved, unresolved };
}

const entrypoint = resolve(sourceRoot, "main.ts");
if (!moduleSet.has(entrypoint)) throw new Error("frontend-reachability: src/main.ts is missing");

const reachable = new Set();
const unresolvedImports = [];
const queue = [entrypoint];
while (queue.length > 0) {
  const current = queue.shift();
  if (reachable.has(current)) continue;
  reachable.add(current);

  const imported = importedModules(current);
  for (const specifier of imported.unresolved) {
    unresolvedImports.push({
      importer: relative(appRoot, current).replaceAll("\\", "/"),
      specifier,
    });
  }
  for (const dependency of imported.resolved) {
    if (!reachable.has(dependency)) queue.push(dependency);
  }
}

if (unresolvedImports.length > 0) {
  console.error("Frontend reachability failed; relative source imports could not be resolved:");
  for (const { importer, specifier } of unresolvedImports) {
    console.error(`- ${importer}: ${specifier}`);
  }
  process.exit(1);
}

const orphans = modules
  .filter((path) => !reachable.has(path))
  .map((path) => relative(appRoot, path).replaceAll("\\", "/"))
  .sort();

if (orphans.length > 0) {
  console.error("Frontend reachability failed; source modules are not reachable from src/main.ts:");
  for (const orphan of orphans) console.error(`- ${orphan}`);
  process.exit(1);
}

console.log(`[frontend-reachability] ${reachable.size} source modules are reachable from src/main.ts; no orphan modules found.`);

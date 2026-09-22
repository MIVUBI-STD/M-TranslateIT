import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = resolve(appRoot, "src");

const apiObjects = [
  ["runtimeApi", "app/bridge/runtimeApi.ts"],
  ["applicationRuntimeApi", "app/bridge/applicationRuntimeApi.ts"],
  ["myVoiceApi", "app/bridge/myVoiceApi.ts"],
  ["myVoiceBuildApi", "app/bridge/myVoiceBuildApi.ts"],
  ["appUpdateApi", "app/update/appUpdateApi.ts"],
];

function collect(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path));
    else if (entry.isFile() && [".ts", ".svelte"].includes(extname(entry.name))) files.push(path);
  }
  return files;
}

function objectBlock(source, objectName) {
  const marker = `export const ${objectName} = {`;
  const start = source.indexOf(marker);
  if (start < 0) throw new Error(`bridge-api-usage: missing export object ${objectName}`);
  const bodyStart = start + marker.length;
  const end = source.indexOf("\n};", bodyStart);
  if (end < 0) throw new Error(`bridge-api-usage: unterminated export object ${objectName}`);
  return source.slice(bodyStart, end);
}

function objectMethods(source, objectName) {
  const block = objectBlock(source, objectName);
  const names = new Set();

  for (const match of block.matchAll(/^\s{2}(?:async\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(/gm)) {
    names.add(match[1]);
  }
  for (const match of block.matchAll(/^\s{2}([A-Za-z_][A-Za-z0-9_]*)\s*:/gm)) {
    names.add(match[1]);
  }
  for (const match of block.matchAll(/^\s{2}([A-Za-z_][A-Za-z0-9_]*)\s*,\s*$/gm)) {
    names.add(match[1]);
  }

  return names;
}

const allSource = collect(sourceRoot);
const failures = [];
let totalMethods = 0;

for (const [objectName, relativePath] of apiObjects) {
  const apiPath = resolve(sourceRoot, relativePath);
  const source = readFileSync(apiPath, "utf8");
  const methodNames = objectMethods(source, objectName);
  totalMethods += methodNames.size;

  const consumers = allSource.filter((path) => path !== apiPath);
  for (const name of methodNames) {
    const pattern = new RegExp(`\\b${objectName}\\.${name}\\b`);
    const usedBy = consumers
      .filter((path) => pattern.test(readFileSync(path, "utf8")))
      .map((path) => relative(appRoot, path).replaceAll("\\", "/"));

    if (usedBy.length === 0) failures.push(`${objectName}.${name}`);
  }
}

if (failures.length > 0) {
  console.error("Bridge object API method reachability failed; wrappers have no frontend consumer:");
  for (const name of failures.sort()) console.error(`- ${name}`);
  process.exit(1);
}

console.log(`[bridge-api-usage] ${totalMethods} bridge object methods have frontend consumers.`);

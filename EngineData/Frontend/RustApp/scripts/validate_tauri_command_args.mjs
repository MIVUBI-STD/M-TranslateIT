import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rustRoot = resolve(appRoot, "src-tauri", "src");
const frontendRoot = resolve(appRoot, "src");

function collect(directory, extensions) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path, extensions));
    else if (entry.isFile() && extensions.has(extname(entry.name))) files.push(path);
  }
  return files;
}

function snakeToCamel(value) {
  return value.replace(/_([a-z0-9])/g, (_, character) => character.toUpperCase());
}

function splitParams(value) {
  const result = [];
  let current = "";
  let depth = 0;
  for (const character of value) {
    if (character === "<" || character === "(" || character === "[" || character === "{") depth += 1;
    if (character === ">" || character === ")" || character === "]" || character === "}") depth = Math.max(0, depth - 1);
    if (character === "," && depth === 0) {
      if (current.trim()) result.push(current.trim());
      current = "";
    } else {
      current += character;
    }
  }
  if (current.trim()) result.push(current.trim());
  return result;
}

function frontendArgKeys(objectBody) {
  if (!objectBody) return [];
  return [...objectBody.matchAll(/(?:^|,)\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?::|,|$)/g)]
    .map((match) => match[1]);
}

const rustCommands = new Map();
for (const path of collect(rustRoot, new Set([".rs"]))) {
  const body = readFileSync(path, "utf8");
  const pattern = /#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+([a-z_][a-z_0-9]*)\s*\(([\s\S]*?)\)\s*(?:->|\{)/g;
  for (const match of body.matchAll(pattern)) {
    const parameters = splitParams(match[2]);
    const frontendParameters = [];
    for (const parameter of parameters) {
      const parts = parameter.split(/:(.+)/);
      if (parts.length < 3) continue;
      const name = parts[0].trim().replace(/^mut\s+/, "");
      const type = parts[1].trim();
      if (
        /(?:^|::)(?:AppHandle|Window|WebviewWindow)(?:<|$)/.test(type)
        || /(?:^|::)State\s*</.test(type)
      ) {
        continue;
      }
      frontendParameters.push(snakeToCamel(name));
    }
    rustCommands.set(match[1], {
      path: relative(appRoot, path).replaceAll("\\", "/"),
      args: frontendParameters.sort(),
    });
  }
}

const frontendCalls = new Map();
for (const path of collect(frontendRoot, new Set([".ts", ".svelte"]))) {
  const body = readFileSync(path, "utf8");
  const pattern = /(?:runCommand|invoke[A-Za-z0-9_]*)(?:<[\s\S]{0,200}?>)?\(\s*"([a-z_][a-z_0-9]*)"\s*(?:,\s*\{([\s\S]{0,600}?)\})?\s*\)/g;
  for (const match of body.matchAll(pattern)) {
    const keys = frontendArgKeys(match[2] ?? "").sort();
    const existing = frontendCalls.get(match[1]);
    if (existing && JSON.stringify(existing.args) !== JSON.stringify(keys)) {
      throw new Error(`tauri-command-args: inconsistent frontend argument shapes for ${match[1]}`);
    }
    frontendCalls.set(match[1], {
      path: relative(appRoot, path).replaceAll("\\", "/"),
      args: keys,
    });
  }
}

const registry = readFileSync(resolve(rustRoot, "commands", "registry.rs"), "utf8");
const registered = new Set(
  [...registry.matchAll(/crate::(?:commands|engine)::[a-z_0-9:]+::([a-z_][a-z_0-9]*)/g)]
    .map((match) => match[1]),
);

const failures = [];
for (const command of registered) {
  const rust = rustCommands.get(command);
  const frontend = frontendCalls.get(command);
  if (!rust) {
    failures.push(`${command}: registered command definition was not found`);
    continue;
  }
  if (!frontend) {
    failures.push(`${command}: frontend invocation was not found`);
    continue;
  }
  if (JSON.stringify(rust.args) !== JSON.stringify(frontend.args)) {
    failures.push(
      `${command}: Rust args [${rust.args.join(", ")}] != frontend args [${frontend.args.join(", ")}]`,
    );
  }
}

if (failures.length > 0) {
  console.error("Tauri command argument contract validation failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`[tauri-command-args] ${registered.size} registered commands have matching frontend argument keys.`);

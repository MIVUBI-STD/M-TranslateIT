import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriRoot = resolve(appRoot, "src-tauri");
const rustRoot = resolve(tauriRoot, "src");
const cargo = readFileSync(resolve(tauriRoot, "Cargo.toml"), "utf8");

function collect(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collect(path));
    else if (entry.isFile() && extname(entry.name) === ".rs") files.push(path);
  }
  return files;
}

function section(name) {
  const marker = `[${name}]`;
  const start = cargo.indexOf(marker);
  if (start < 0) return "";
  const bodyStart = start + marker.length;
  const rest = cargo.slice(bodyStart);
  const next = rest.search(/\n\[[^\]]+\]/);
  return next < 0 ? rest : rest.slice(0, next);
}

function dependencyNames(body) {
  const names = [];
  for (const line of body.split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*/);
    if (match?.[1]) names.push(match[1]);
  }
  return names;
}

const source = collect(rustRoot).map((path) => readFileSync(path, "utf8")).join("\n");
const buildSource = readFileSync(resolve(tauriRoot, "build.rs"), "utf8");
const normalize = (name) => name.replaceAll("-", "_");
const referenced = (body, name) => {
  const crate = normalize(name);
  return body.includes(`${crate}::`) || body.includes(`${crate}!`) || new RegExp(`\\b${crate}\\b`).test(body);
};

const runtimeDeps = dependencyNames(section("dependencies"));
const buildDeps = dependencyNames(section("build-dependencies"));
const unusedRuntime = runtimeDeps.filter((name) => !referenced(source, name));
const unusedBuild = buildDeps.filter((name) => !referenced(buildSource, name));

if (unusedRuntime.length || unusedBuild.length) {
  console.error("Rust direct dependency usage validation failed:");
  for (const name of unusedRuntime) console.error(`- runtime dependency never referenced by src-tauri/src: ${name}`);
  for (const name of unusedBuild) console.error(`- build dependency never referenced by build.rs: ${name}`);
  process.exit(1);
}

console.log(`[rust-dependency-usage] ${runtimeDeps.length} runtime and ${buildDeps.length} build dependencies are referenced.`);

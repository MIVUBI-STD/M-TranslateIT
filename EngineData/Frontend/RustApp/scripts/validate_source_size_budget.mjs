import { readdir, stat } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

const roots = [resolve(appRoot, "src"), resolve(appRoot, "src-tauri", "src")];
const trackedExtensions = new Set([".rs", ".svelte", ".ts"]);

const defaultBudgets = {
  ".rs": 30_000,
  ".svelte": 20_000,
  ".ts": 20_000,
};

// Path-specific budgets are allowed only to tighten ownership below the default.
// They are not exemptions and must never be used to permit oversized source files.
const pathBudgets = new Map([
  ["src/App.svelte", 16_000],
  ["src/app/bridge/runtimeProductFacade.ts", 9_000],
  ["src-tauri/src/commands/voice_lab_recording.rs", 24_000],
]);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(path)));
    else if (entry.isFile() && trackedExtensions.has(extname(entry.name))) files.push(path);
  }
  return files;
}

const violations = [];
for (const root of roots) {
  for (const path of await collectFiles(root)) {
    const repoPath = relative(appRoot, path).replaceAll("\\", "/");
    const extension = extname(path);
    const budget = pathBudgets.get(repoPath) ?? defaultBudgets[extension];
    const { size } = await stat(path);
    if (size > budget) violations.push({ repoPath, size, budget });
  }
}

if (violations.length > 0) {
  console.error("Source size budget exceeded:");
  for (const { repoPath, size, budget } of violations) {
    console.error(`- ${repoPath}: ${size} bytes > ${budget} byte budget`);
  }
  console.error("Split ownership before adding more responsibility; do not raise a budget without an explicit architecture decision.");
  process.exit(1);
}

console.log("Source size budget passed.");

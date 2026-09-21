import { spawnSync } from "node:child_process";

const executable = process.platform === "win32" ? "npx.cmd" : "npx";
const result = spawnSync(
  executable,
  ["svelte-check", "--tsconfig", "./tsconfig.json"],
  { cwd: process.cwd(), encoding: "utf8", shell: false },
);

if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);
if (result.error) {
  console.error(`svelte-check could not start: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) process.exit(result.status ?? 1);

const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
const summary = output.match(/svelte-check found\s+(\d+)\s+errors?\s+and\s+(\d+)\s+warnings?/i);
if (!summary) {
  console.error("Strict Svelte check could not verify the diagnostic summary; failing closed.");
  process.exit(1);
}
const errors = Number(summary[1]);
const warnings = Number(summary[2]);
if (errors !== 0 || warnings !== 0) {
  console.error(`Strict Svelte check requires 0 errors and 0 warnings; found ${errors} errors and ${warnings} warnings.`);
  process.exit(1);
}
console.log("[svelte-check-strict] 0 errors and 0 warnings.");

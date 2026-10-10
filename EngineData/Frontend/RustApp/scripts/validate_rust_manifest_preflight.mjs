import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const repoRoot = resolve(appRoot, "..", "..", "..");
const manifestPath = join(appRoot, "src-tauri", "Cargo.toml");
const lockPath = join(appRoot, "src-tauri", "Cargo.lock");
const policyPath = join(repoRoot, "toolchain.json");

// These are minimum-version *review facts* for selected exact locked crates,
// not a substitute for Cargo's dependency resolution, compilation or MSRV proof.
// Any change to a reviewed locked version must receive a new source review.
const reviewedLockFacts = [
  ["time", "0.3.49", "1.88.0"],
  ["icu_provider", "2.2.0", "1.86.0"],
  ["tauri-plugin-updater", "2.11.0", "1.77.2"],
];

function versionParts(value) {
  if (typeof value !== "string" || !/^\d+\.\d+(?:\.\d+)?$/.test(value)) return null;
  const parts = value.split(".").map(Number);
  return [parts[0], parts[1], parts[2] ?? 0];
}

function isLower(a, b) {
  const left = versionParts(a);
  const right = versionParts(b);
  if (!left || !right) return true;
  for (let i = 0; i < 3; i += 1) {
    if (left[i] !== right[i]) return left[i] < right[i];
  }
  return false;
}

function lockedVersion(lock, name) {
  const expression = new RegExp(
    '\\[\\[package\\]\\]\\s*\\nname = "' + name + '"\\s*\\nversion = "([^"]+)"',
    "m",
  );
  return lock.match(expression)?.[1] ?? null;
}

export function evaluateRustMsrv(manifest, policy, lock) {
  const failures = [];
  for (const marker of ["[package]", "[dependencies]"]) {
    if (!manifest.includes(marker)) failures.push("missing manifest section " + marker);
  }
  if (!manifest.includes("tauri")) failures.push("Tauri dependency marker absent");
  const declared = manifest.match(/^rust-version\s*=\s*"([^"]+)"/m)?.[1] ?? null;
  const policyVersion = policy?.rust?.minimum;
  if (!versionParts(declared)) failures.push("invalid or missing Cargo.toml rust-version");
  if (!versionParts(policyVersion)) failures.push("invalid or missing toolchain.json rust.minimum");
  if (declared !== policyVersion) failures.push("Cargo.toml and toolchain.json Rust minimum diverge");

  for (const [name, reviewedVersion, minimum] of reviewedLockFacts) {
    const current = lockedVersion(lock, name);
    if (current !== reviewedVersion) {
      failures.push(name + " changed from reviewed Cargo.lock version " + reviewedVersion +
        "; re-evaluate its MSRV before accepting a new lockfile");
    } else if (isLower(declared, minimum)) {
      failures.push(name + " " + current + " requires at least Rust " + minimum);
    }
  }
  const tauri = lockedVersion(lock, "tauri");
  if (!tauri) failures.push("Cargo.lock is missing Tauri");
  else if (!isLower(tauri, "2.12.0") && isLower(declared, "1.90.0")) {
    failures.push("Tauri 2.12+ requires a reviewed Rust 1.90+ toolchain");
  }
  return failures;
}

function main() {
  for (const path of [manifestPath, lockPath, policyPath]) {
    if (!existsSync(path)) {
      console.error("[rust-manifest-preflight] missing required source: " + path);
      process.exit(1);
    }
  }
  const manifest = readFileSync(manifestPath, "utf8");
  const lock = readFileSync(lockPath, "utf8");
  const policy = JSON.parse(readFileSync(policyPath, "utf8"));
  const failures = evaluateRustMsrv(manifest, policy, lock);
  if (failures.length) {
    for (const issue of failures) console.error("[rust-manifest-preflight] " + issue);
    process.exit(1);
  }
  console.log("[rust-manifest-preflight] reviewed locked Rust MSRV source contract passed; full Cargo check NOT EXECUTED.");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();

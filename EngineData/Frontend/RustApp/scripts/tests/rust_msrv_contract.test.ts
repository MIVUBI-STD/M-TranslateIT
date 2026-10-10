import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { evaluateRustMsrv } from "../validate_rust_manifest_preflight.mjs";

const root = new URL("../../", import.meta.url);
const manifest = readFileSync(new URL("src-tauri/Cargo.toml", root), "utf8");
const lock = readFileSync(new URL("src-tauri/Cargo.lock", root), "utf8");
const policy = JSON.parse(readFileSync(new URL("../../../toolchain.json", root), "utf8"));

test("reviewed locked dependency MSRV is aligned with Cargo and toolchain", () => {
  assert.deepEqual(evaluateRustMsrv(manifest, policy, lock), []);
});

test("Rust 1.77 underclaim is rejected for the existing lockfile", () => {
  const old = manifest.replace('rust-version = "1.88.0"', 'rust-version = "1.77"');
  const oldPolicy = { ...policy, rust: { ...policy.rust, minimum: "1.77" } };
  assert.match(evaluateRustMsrv(old, oldPolicy, lock).join("\n"), /time 0\.3\.49 requires at least Rust 1\.88/);
});

test("toolchain policy cannot drift from Cargo rust-version", () => {
  const drift = { ...policy, rust: { ...policy.rust, minimum: "1.77" } };
  assert.match(evaluateRustMsrv(manifest, drift, lock).join("\n"), /minimum diverge/);
});

test("unreviewed lock changes require new minimum compiler review", () => {
  const changed = lock.replace(
    /(\[\[package\]\]\s*\nname = "time"\s*\nversion = ")0\.3\.49"/,
    (_, prefix) => prefix + '0.3.50"',
  );
  assert.notEqual(changed, lock);
  assert.match(evaluateRustMsrv(manifest, policy, changed).join("\n"), /time changed from reviewed/);
});

test("missing package is not silently treated as proof", () => {
  const edited = lock.replace(
    /(\[\[package\]\]\s*\nname = ")icu_provider"/,
    (_, prefix) => prefix + 'renamed_icu_provider"',
  );
  assert.notEqual(edited, lock);
  assert.match(evaluateRustMsrv(manifest, policy, edited).join("\n"), /icu_provider changed from reviewed/);
});

test("future Tauri minor cannot silently retain the old MSRV", () => {
  const future = lock.replace(
    /(\[\[package\]\]\s*\nname = "tauri"\s*\nversion = ")2\.11\.2"/,
    (_, prefix) => prefix + '2.12.0"',
  );
  assert.notEqual(future, lock);
  assert.match(evaluateRustMsrv(manifest, policy, future).join("\n"), /Tauri 2\.12\+/);
});

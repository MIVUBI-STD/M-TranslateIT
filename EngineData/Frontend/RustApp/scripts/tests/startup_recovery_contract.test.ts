import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const recovery = readFileSync(new URL("../../src-tauri/src/commands/startup_recovery.rs", import.meta.url), "utf8");
const bootstrap = readFileSync(new URL("../../src-tauri/src/app_bootstrap.rs", import.meta.url), "utf8");
const main = readFileSync(new URL("../../src-tauri/src/main.rs", import.meta.url), "utf8");
const shutdown = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/shutdown.rs", import.meta.url), "utf8");

test("startup recovery never resumes Meeting and cleans only bounded cache namespaces", () => {
  assert.match(bootstrap, /begin_startup_recovery/);
  assert.match(recovery, /audio_segments/);
  assert.match(recovery, /meeting_tts/);
  assert.match(recovery, /helper_functional_readiness/);
  assert.doesNotMatch(recovery, /start_meeting_translation|begin_application_meeting_session/);
  assert.doesNotMatch(recovery, /remove_dir_all\([^)]*user_cache_dir/);
});

test("active previous process prevents destructive stale cleanup without trusting reused PIDs", () => {
  assert.match(recovery, /marker_process_is_alive/);
  assert.match(recovery, /process\.start_time\(\)/);
  assert.match(recovery, /marker_matches_process_start/);
  assert.match(recovery, /let Some\(marker\) = read_marker[\s\S]*stale_found = true/);
  assert.match(recovery, /another_instance_detected = live_other_found/);
  assert.match(recovery, /Shared Meeting cache was left untouched|did not modify the other process's Meeting cache/);
});

test("clean app exit remains fail-closed and clears the marker after helper shutdown", () => {
  assert.match(main, /application_runtime::prepare_for_app_exit\(\)/);
  assert.match(shutdown, /if !super::super::helper_bridge::shutdown_helper_bridge_for_app_exit\(\) \{\s*return false;/);
  assert.match(shutdown, /mark_clean_shutdown\(\)/);
  assert.ok(shutdown.indexOf("mark_clean_shutdown") > shutdown.indexOf("shutdown_helper_bridge_for_app_exit"));
});

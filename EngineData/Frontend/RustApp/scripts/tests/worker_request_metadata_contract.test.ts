import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const policy = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge/request_policy.rs", import.meta.url),
  "utf8",
);
const runtime = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
  "utf8",
);
const transport = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge/transport.rs", import.meta.url),
  "utf8",
);
const worker = readFileSync(
  new URL(
    "../../../../Backend/LocalWorker/WorkerRuntime/realtime_local_worker_base.py",
    import.meta.url,
  ),
  "utf8",
);
const common = readFileSync(
  new URL(
    "../../../../Backend/LocalWorker/WorkerRuntime/worker_runtime_common.py",
    import.meta.url,
  ),
  "utf8",
);

test("Rust injects the worker command and bounded request deadline consumed by Python", () => {
  assert.match(policy, /object\.insert\("command"\.to_string\(\), json!\(task\)\)/);
  assert.match(runtime, /"deadline_unix_ms"\.to_string\(\)/);
  assert.match(worker, /request\.get\("command", "status"\)/);
  assert.match(common, /payload\.get\("deadline_unix_ms", 0\)/);
  assert.match(worker, /request_deadline_expired\(request\)/);
});

test("request correlation metadata remains Rust transport-owned", () => {
  assert.match(policy, /object\.insert\("request_id"\.to_string\(\), json!\(request_id\)\)/);
  assert.match(policy, /object\.insert\("scheduler_priority"\.to_string\(\), json!\(priority\.label\(\)\)\)/);
  assert.match(transport, /object\.insert\("request_id"\.to_string\(\), json!\(request_id\)\)/);
  assert.match(transport, /object\.insert\("scheduler_priority"\.to_string\(\), json!\(priority\.label\(\)\)\)/);
  assert.doesNotMatch(worker, /meeting_generation|meeting_session_id|meeting_lane|meeting_start_prepare/);
});

test("Meeting scheduling metadata remains host-authoritative", () => {
  for (const field of [
    "meeting_generation",
    "meeting_session_id",
    "meeting_lane",
    "meeting_start_prepare",
  ]) {
    assert.match(policy, new RegExp(field));
  }
});

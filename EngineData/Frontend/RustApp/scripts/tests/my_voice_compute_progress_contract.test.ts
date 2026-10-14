import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = new URL("../../", import.meta.url);
const read = (name: string) => readFileSync(new URL(name, app), "utf8");

test("My Voice explicit hardware check is bounded, truthful, and not build-status polling", () => {
  const owner = read("src-tauri/src/commands/voice_lab_build.rs");
  const ui = read("src/components/my-voice/MyVoiceBuild.svelte");
  const bridge = read("src/app/bridge/myVoiceBuildApi.ts");
  const probe = read("../../Backend/LocalWorker/WorkerRuntime/my_voice_compute_probe.py");
  assert.match(owner, /pub async fn get_my_voice_compute_status/);
  assert.match(owner, /spawn_blocking\(probe_my_voice_compute_status\)/);
  assert.match(owner, /Duration::from_secs\(15\)/);
  assert.match(owner, /my_voice_compute_probe\.py/);
  assert.match(probe, /torch_module\.cuda\.is_available\(\)/);
  assert.match(probe, /get_device_properties\(0\)/);
  assert.match(probe, /vram_mib/);
  assert.match(probe, /cuda\.mem_get_info\(index\)/);
  assert.match(probe, /cuda_device_uuid\(cuda, index\)/);
  assert.match(probe, /sensor_reader\(uuid\)/);
  assert.match(bridge, /getComputeStatus\(\)/);
  assert.match(ui, /computeReadVersion \+= 1/);
  assert.match(ui, /computeStatus\.admission_state === "admitted"/);
  assert.match(owner, /let compute = probe_my_voice_compute_status\(\);/);
  assert.match(owner, /gpu_admission_blocked/);
  assert.match(owner, /"CUDA_VISIBLE_DEVICES", &gpu_uuid/);
  assert.match(owner, /"TRANSLATEIT_MYVOICE_GPU_UUID", &gpu_uuid/);
  assert.match(owner, /\.arg\("--gpu-budget-mib"\)/);
  assert.match(ui, /!computeCanStart/);
  assert.match(ui, /onTrainingComputeEligibility\?\.\(computeCanStart\)/);
  const parent = read("src/pages/MyVoice.svelte");
  assert.match(parent, /!trainingComputeAllowed/);
  assert.match(parent, /onTrainingComputeEligibility=\{\(allowed\)/);
  assert.doesNotMatch(bridge.match(/async getStatus\(\)[\s\S]*?\n  \},/)?.[0] ?? "", /getComputeStatus/);
});

test("progress labels follow actual SoVITS and GPT stage transitions, not elapsed time", () => {
  const worker = read("../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits_build.py");
  const ui = read("src/components/my-voice/MyVoiceBuild.svelte");
  const sovits = worker.indexOf('status_writer("training", "Learning voice characteristics');
  const gpt = worker.indexOf('status_writer("training", "Learning speech generation');
  assert.ok(sovits > 0 && gpt > sovits);
  assert.ok(worker.indexOf('run_stage(source_root, assets["sovits_train"]', sovits) < gpt);
  assert.ok(worker.indexOf('run_stage(source_root, assets["gpt_train"]', gpt) > gpt);
  assert.match(ui, /build\.message\.includes\("SoVITS training stage"\)/);
  assert.match(ui, /build\.message\.includes\("GPT training stage"\)/);
  assert.match(ui, /build\.message\.replace\(/);
});

test("worker completion and Review are bound to one generation identity", () => {
  const rust = read("src-tauri/src/commands/voice_lab_build.rs");
  const python = read("../../Backend/LocalWorker/WorkerRuntime/voice_lab_build.py");
  const receipts = read("src-tauri/src/commands/voice_lab_build/receipts.rs");
  assert.match(rust, /\.arg\("--run-token"\)/);
  assert.match(rust, /\.arg\("--run-generation"\)/);
  assert.match(rust, /value\.run_token == identity\.token/);
  assert.match(rust, /commit_completion\(&paths, &run_identity, &evaluation\.review_id\)/);
  assert.match(rust, /completion_matches\(&paths, &evaluation\.review_id\)/);
  assert.match(python, /"run_token": run_token/);
  assert.match(python, /"sequence": sequence/);
  assert.match(receipts, /create_new\(true\)/);
});

test("SAFE_8GB resource and process controls fail closed at native boundaries", () => {
  const owner = read("src-tauri/src/commands/voice_lab_build.rs");
  const nativeGuard = read("src-tauri/src/commands/my_voice_resources.rs");
  const treeGuard = read("src-tauri/src/commands/voice_lab_build/process_owner.rs");
  const supervisor = read("../../Backend/LocalWorker/WorkerRuntime/my_voice_stage_supervisor.py");
  const systemGuard = read("../../Backend/LocalWorker/WorkerRuntime/my_voice_resource_guard.py");
  const recovery = read("src-tauri/src/commands/voice_lab/storage/training_recovery.rs");
  const ui = read("src/components/my-voice/MyVoiceBuild.svelte");
  assert.match(owner, /my_voice_resources::preflight/);
  assert.match(owner, /creation_flags\(0x0000_0004\)/);
  assert.match(owner, /owned_job\.resume_suspended\(pid\)/);
  assert.match(nativeGuard, /GlobalMemoryStatusEx/);
  assert.match(nativeGuard, /GetDiskFreeSpaceExW/);
  assert.match(nativeGuard, /checkpoint_archive_space_insufficient/);
  assert.match(treeGuard, /ResumeThread/);
  assert.match(supervisor, /_resume_owned_suspended_process\(proc\.pid\)/);
  assert.match(supervisor, /observer\.observe\(SafetySample/);
  assert.match(supervisor, /cooperative_safe_stop_checkpoint_candidate/);
  assert.match(supervisor, /_create_cooperative_request/);
  assert.match(supervisor, /cooperative_stop_not_acknowledged/);
  assert.match(supervisor, /on_safety_change\(result.state, result.reason\)/);
  assert.match(supervisor, /proc\.returncode != 0/);
  assert.match(owner, /stopped_at_checkpoint_boundary/);
  assert.match(systemGuard, /START_CACHE_MIB = 12 \* 1024/);
  assert.match(recovery, /ensure_archive_capacity\(&root, total\)/);
  assert.match(ui, /case "resource_admission_blocked":/);
  assert.match(ui, /case "build_process_guard_failed":/);
});

test("explicit compute checker remains in the release small-resource closure", () => {
  const config = JSON.parse(read("src-tauri/tauri.release.conf.json"));
  const key = "../../../Backend/LocalWorker/WorkerRuntime/my_voice_compute_probe.py";
  assert.equal(config.bundle.resources[key], "EngineData/Backend/LocalWorker/WorkerRuntime/my_voice_compute_probe.py");
  assert.match(read("scripts/validate_release_package_contract.mjs"), /my_voice_compute_probe\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_compute_probe\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_gpu_policy\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_gpu_identity\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_single_gpu_adapter\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_resource_guard\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_adaptive_safety\.py/);
  assert.match(read("scripts/validate_release_payload.mjs"), /my_voice_cooperative\.py/);
  const adapter = read("../../Backend/LocalWorker/WorkerRuntime/my_voice_single_gpu_adapter.py");
  const runner = read("../../Backend/LocalWorker/WorkerRuntime/voice_lab_upstream_stage.py");
  assert.match(adapter, /single_gpu_source_revision_mismatch/);
  assert.match(adapter, /run\(0, 1, hps\)/);
  assert.match(adapter, /sovits_preserve_physical_gpu_uuid/);
  assert.match(adapter, /strategy="auto"/);
  assert.match(runner, /single_gpu_source\(script, relative_script\.as_posix\(\)\)/);
  assert.match(read("../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits_build.py"), /evaluation_child_guard/);
  assert.match(read("../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits_build.py"), /evaluation_sample_identity_invalid/);
  assert.match(read("src-tauri/src/commands/registry.rs"), /get_my_voice_compute_status/);
});

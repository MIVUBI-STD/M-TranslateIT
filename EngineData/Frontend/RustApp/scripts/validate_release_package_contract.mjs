import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const tauriRoot = join(appRoot, "src-tauri");
const backendRoot = resolve(appRoot, "../../Backend");
const workerRoot = join(backendRoot, "LocalWorker", "WorkerRuntime");
const errors = [];
const fail = (message) => errors.push(message);
const readText = (path) => readFileSync(path, "utf8");
const readJson = (path) => JSON.parse(readText(path));
const requireFile = (path) => {
  if (!existsSync(path) || !statSync(path).isFile()) fail(`Missing required file: ${path}`);
};
const requireMarkers = (body, label, markers) => {
  for (const marker of markers) if (!body.includes(marker)) fail(`${label} marker missing: ${marker}`);
};
const forbidMarkers = (body, label, markers) => {
  for (const marker of markers) if (body.includes(marker)) fail(`${label} forbidden marker present: ${marker}`);
};
const compareExact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} drifted: actual=${JSON.stringify(actual)} expected=${JSON.stringify(expected)}`);
  }
};

const paths = {
  package: join(appRoot, "package.json"),
  config: join(tauriRoot, "tauri.release.conf.json"),
  manifest: join(workerRoot, "model_manifest.json"),
  entrypoint: join(workerRoot, "realtime_local_worker.py"),
  base: join(workerRoot, "realtime_local_worker_base.py"),
  provider: join(workerRoot, "milmmt_translation_provider.py"),
  build: join(scriptDir, "build_release.ps1"),
  stage: join(scriptDir, "stage_release_inputs_impl.ps1"),
  builder: join(scriptDir, "build_r3_external_payload.py"),
  hook: join(tauriRoot, "windows", "r3_payload_hooks.template.nsh"),
  helper: join(tauriRoot, "windows", "r3_payload_installer.ps1"),
};
for (const path of Object.values(paths)) requireFile(path);
if (errors.length) {
  for (const error of errors) console.error(`[release-package] ${error}`);
  process.exit(1);
}

const packageJson = readJson(paths.package);
const config = readJson(paths.config);
const manifest = readJson(paths.manifest);
const entrypoint = readText(paths.entrypoint);
const base = readText(paths.base);
const provider = readText(paths.provider);
const build = readText(paths.build);
const stage = readText(paths.stage);
const builder = readText(paths.builder);
const hook = readText(paths.hook);
const helper = readText(paths.helper);

const canonicalPayloadRoots = [
  "EngineData/Backend/LocalWorker/PythonRuntime",
  "EngineData/Backend/RuntimeAssets/ASR/ModelData",
  "EngineData/Backend/RuntimeAssets/Translation/ModelData",
  "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices",
  "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS",
  "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package",
];
const builderRootsMatch = builder.match(/PAYLOAD_ROOTS\s*=\s*\(([\s\S]*?)\)\s*\nREQUIRED_FILES/);
if (!builderRootsMatch) fail("payload builder PAYLOAD_ROOTS block is missing or unparsable.");
else {
  const roots = [...builderRootsMatch[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  compareExact(roots, canonicalPayloadRoots, "payload builder root closure");
}
const helperRootsMatch = helper.match(/\$PayloadRoots=@\(([\s\S]*?)\)\s*\n/);
if (!helperRootsMatch) fail("installer helper PayloadRoots block is missing or unparsable.");
else {
  const roots = [...helperRootsMatch[1].matchAll(/'([^']+)'/g)].map((match) => match[1].replaceAll("\\", "/"));
  compareExact(roots, canonicalPayloadRoots, "installer helper root closure");
}

const expectedResources = {
  "../../../Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py": "EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py",
  "../../../Backend/LocalWorker/WorkerRuntime/realtime_local_worker_base.py": "EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker_base.py",
  "../../../Backend/LocalWorker/WorkerRuntime/milmmt_translation_provider.py": "EngineData/Backend/LocalWorker/WorkerRuntime/milmmt_translation_provider.py",
  "../../../Backend/LocalWorker/WorkerRuntime/worker_runtime_common.py": "EngineData/Backend/LocalWorker/WorkerRuntime/worker_runtime_common.py",
  "../../../Backend/LocalWorker/WorkerRuntime/worker_io_runtime.py": "EngineData/Backend/LocalWorker/WorkerRuntime/worker_io_runtime.py",
  "../../../Backend/LocalWorker/WorkerRuntime/translation_envelope.py": "EngineData/Backend/LocalWorker/WorkerRuntime/translation_envelope.py",
  "../../../Backend/LocalWorker/WorkerRuntime/voice_lab_build.py": "EngineData/Backend/LocalWorker/WorkerRuntime/voice_lab_build.py",
  "../../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits.py": "EngineData/Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits.py",
  "../../../Backend/LocalWorker/WorkerRuntime/voice_lab_upstream_stage.py": "EngineData/Backend/LocalWorker/WorkerRuntime/voice_lab_upstream_stage.py",
  "../../../Backend/LocalWorker/WorkerRuntime/model_manifest.json": "EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json",
  "../../../Backend/RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt": "EngineData/Backend/RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt",
  "../../../Backend/RuntimeAssets/AudioProvider/VBCABLE/NOTICE.txt": "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/NOTICE.txt",
};
const resources = config.bundle?.resources;
if (!resources || Array.isArray(resources)) fail("R3 release must use the canonical source-to-target resource map.");
else {
  const actual = Object.entries(resources).sort(([a], [b]) => a.localeCompare(b));
  const expected = Object.entries(expectedResources).sort(([a], [b]) => a.localeCompare(b));
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail("R3 Setup resource map drifted from the small canonical closure.");
  for (const source of Object.keys(resources)) {
    if (/UserData|DevelopingData|\.venv|tests/i.test(source)) fail(`Development/user baggage in Setup resources: ${source}`);
  }
}
for (const largeSource of [
  "../../../Backend/LocalWorker/PythonRuntime/",
  "../../../Backend/RuntimeAssets/ASR/ModelData/",
  "../../../Backend/RuntimeAssets/Translation/ModelData/",
  "../../../Backend/RuntimeAssets/Voice/GPTSoVITS/",
  "../../../Backend/RuntimeAssets/AudioProvider/VBCABLE/Package/",
]) {
  if (resources && Object.prototype.hasOwnProperty.call(resources, largeSource)) fail(`Large payload re-embedded in Tauri resources: ${largeSource}`);
}

const nsis = config.bundle?.windows?.nsis ?? {};
if (nsis.installMode !== "perMachine") fail("R3 NSIS installMode must be perMachine.");
if (nsis.installerHooks !== "./target/translateit-r3-payload-hooks.generated.nsh") fail("R3 NSIS must use the generated payload hook.");
if (existsSync(join(tauriRoot, "target", "translateit-r3-payload-hooks.generated.nsh"))) fail("Generated R3 hook must stay out of source.");

requireMarkers(stage, "release staging", [
  "$ffmpegReleaseTag = 'autobuild-2026-09-07-15-39'",
  "$ffmpegAssetName = 'ffmpeg-n8.1.2-51-g7ba069f4f1-win64-lgpl-8.1.zip'",
  "$ffmpegArchiveSha = '232464b6f9f1d55fa42c1b0e7ae1c9ca5a19272ba61229e8b32a93751055e135'",
  "releases/download/$ffmpegReleaseTag/$ffmpegAssetName",
  "Assert-Hash $ffmpegArchive $ffmpegArchiveSha 'FFmpeg pinned archive'",
  "integrity_source=pinned_release_asset_sha256",
  "--enable-version3",
]);
forbidMarkers(stage, "release staging moving FFmpeg source", [
  "BtbN/FFmpeg-Builds/releases/latest",
  "ffmpeg-n8.1-latest-win64-lgpl-8.1.zip",
  "$ffmpegRelease = Invoke-RestMethod",
]);

requireMarkers(builder, "payload builder", [
  'PAYLOAD_SCHEMA = "translateit.r3.external_payload.v1"',
  'INSTALLED_RUNTIME_SCHEMA = "translateit.installed_runtime.v1"',
  "TRANSLATEIT_PAYLOAD_CONTRACT.json",
  'TRANSFORMERS_VERSION = "4.57.6"',
  'TOKENIZERS_VERSION = "0.22.2"',
  "validate_no_filesystem_indirection",
  "is_filesystem_indirection",
  "is_junction",
  '"-t7z"', '"-m0=LZMA2"', '"-mx=9"', "find_7zip", "find_tar", "expanded_bytes", "@@APP_VERSION@@", "@@PAYLOAD_EXPANDED_BYTES@@",
]);
forbidMarkers(builder.toLowerCase(), "payload builder network/bootstrap", ["urllib", "requests.get", "invoke-webrequest", "start-bitstransfer", "http://", "https://"]);

requireMarkers(hook, "NSIS lifecycle hook", [
  "@@PAYLOAD_SCHEMA@@", "@@INSTALLED_RUNTIME_SCHEMA@@", "@@APP_VERSION@@", "@@PAYLOAD_SHA256@@", "@@PAYLOAD_EXPANDED_BYTES@@",
  "NSIS_HOOK_PREINSTALL", "NSIS_HOOK_POSTINSTALL", "NSIS_HOOK_PREUNINSTALL", "-Mode Verify", "-Mode Install", "-Mode VerifyInstalled", "/UPDATE", "app-update mode", "$0 == 3010", "SetRebootFlag true",
  "RMDir /r \"$INSTDIR\\EngineData\\Backend\\LocalWorker\\PythonRuntime\"",
  "RMDir /r \"$INSTDIR\\EngineData\\Backend\\RuntimeAssets\\Voice\\BuiltInVoices\"",
  "TRANSLATEIT_INSTALLED_RUNTIME.json",
]);
forbidMarkers(hook.toLowerCase(), "NSIS network/bootstrap", ["inetc::", "nsisdl::", "http://", "https://", "execshell"]);

requireMarkers(helper, "installer helper", [
  "[ValidateSet('Verify','Install','VerifyInstalled')]", "Verify-InstalledRuntime", "Assert-InstalledRuntimeManifest", "Get-FileHash -Algorithm SHA256", "Read-PayloadContract", "Ensure-FreeSpace", ".translateit-r3-stage", ".translateit-r3-backup",
  "Rollback-Payload", "Read-PythonMetadata", "pnputil.exe", "VBCABLE_Setup_x64.exe", "@('-i','-h')", "exit 3010",
  "ExpectedInstalledRuntimeSchema", "preserve_system_driver", "preserve_app_local_user_data",
  "BuiltInVoices\\MaleVoice\\reference.wav",
  "BuiltInVoices\\FemaleVoice\\reference.wav",
  "BuiltInVoices\\SOURCES.json",
]);
forbidMarkers(helper.toLowerCase(), "installer helper network/bootstrap", ["invoke-webrequest", "start-bitstransfer", "webclient", "http://", "https://", "pip install", "pnputil /delete-driver"]);

requireMarkers(build, "release build", [
  "build_r3_external_payload.py", "TranslateIT-Payload.7z", "TranslateIT-Setup.exe", "translateit-r3-payload-hooks.generated.nsh",
  "npm run preflight:tauri-package", "--config src-tauri/tauri.release.conf.json", "R3 release directory must contain exactly Setup + Payload",
]);

if (manifest.schema !== "translateit.local_model_inventory.v2" || manifest.inventory_scope !== "full_product_release_assets") fail("model_manifest.json release inventory contract drifted.");
const requiredIds = new Set((manifest.models ?? []).filter((item) => item.required === true).map((item) => item.model_id));
for (const id of ["faster-whisper-large-v3-turbo", "milmmt-46-1b-v1.0", "gpt-sovits-v2proplus-voicelab"]) if (!requiredIds.has(id)) fail(`Required model missing: ${id}`);
for (const legacy of ["marianmt-id-en", "marianmt-en-id", "m2m100-418m"]) if (requiredIds.has(legacy)) fail(`Legacy translator remains required: ${legacy}`);
const milmmt = (manifest.models ?? []).find((item) => item.model_id === "milmmt-46-1b-v1.0");
if (milmmt?.repo_id !== "xiaomi-research/MiLMMT-46-1B-v1.0" || milmmt?.revision !== "4fc480b6c58dec29c159dcdf9fde0f6d5c354995") fail("MiLMMT release identity drifted.");
if (!entrypoint.includes("import realtime_local_worker_base as runtime") || !entrypoint.includes("milmmt_translation_provider.install(vars(runtime))") || !entrypoint.includes("_PROVIDER_SENTINEL")) fail("Canonical worker entrypoint drifted.");
if (entrypoint.includes("exec(") || entrypoint.includes("compile(") || entrypoint.includes('globals()["__name__"]')) fail("Canonical worker entrypoint returned to dynamic exec bootstrap.");
if (base.includes("AutoModelForSeq2SeqLM") || base.toLowerCase().includes("m2m100")) fail("Legacy translator implementation returned to worker base.");
for (const marker of ["AutoModelForCausalLM", "milmmt-46-1b-v1.0", "4fc480b6c58dec29c159dcdf9fde0f6d5c354995", "do_sample=False"]) if (!provider.includes(marker)) fail(`MiLMMT provider marker missing: ${marker}`);

const scripts = packageJson.scripts ?? {};
if (!String(scripts["preflight:tauri-package"] ?? "").includes("validate_release_package_contract.mjs")) fail("Tauri package preflight must include the R3 release contract.");
if (!String(scripts["validate:source-contracts"] ?? "").includes("preflight:tauri-package")) fail("Source validation must include R3 package preflight.");

if (errors.length) {
  for (const error of errors) console.error(`[release-package] ${error}`);
  process.exit(1);
}
console.log("[release-package] R3 source contract PASS: pinned FFmpeg release asset identity, exact builder/installer payload-root closure, version/hash-bound external payload, app-only /UPDATE preserves only a fully verified compatible installed runtime, no filesystem-indirection payload roots, transactional full runtime replacement including built-in Meeting voice references, symmetric uninstall cleanup, explicit non-exec WorkerRuntime composition, Setup-owned VB-CABLE install/restart, user-data and system-driver preservation policy, and small Tauri resource closure are aligned.");

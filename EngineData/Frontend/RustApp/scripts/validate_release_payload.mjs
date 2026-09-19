import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { buildThirdPartyNoticeBundle, thirdPartyNoticeOutputPath } from "./generate_third_party_notices.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const backendRoot = resolve(appRoot, "../../Backend");
const workerRoot = join(backendRoot, "LocalWorker", "WorkerRuntime");
const pythonRoot = join(backendRoot, "LocalWorker", "PythonRuntime");
const runtimeAssetsRoot = join(backendRoot, "RuntimeAssets");
const voiceSourceRoot = join(runtimeAssetsRoot, "Voice", "GPTSoVITS", "Source");
const vbCableRoot = join(runtimeAssetsRoot, "AudioProvider", "VBCABLE");
const vbCablePackageRoot = join(vbCableRoot, "Package");
const expectedRevision = "d523079fc05d9a8028d6085bffe4a2757c32abb6";
const expectedPythonArchiveSha256 = "4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3";
const expectedNltkRevision = "550b6625bcef1f2abff2ff770a5a0d272c9c6b2a";
const expectedNltkPackages = {
  "corpora/cmudict.zip": "d07cca47fd72ad32ea9d8ad1219f85301eeaf4568f8b6b73747506a71fb5afd6",
  "taggers/averaged_perceptron_tagger.zip": "e1f13cf2532daadfd6f3bc481a49859f0b8ea6432ccdcd83e6a49a5f19008de9",
  "taggers/averaged_perceptron_tagger_eng.zip": "6025f530624335c67d6547d44757b357b4e79bae030a0383e9887a92c1718f0b",
};
const exceptionalPythonLicenseMaterials = {
  "ctranslate2-4.8.1.dist-info": {
    source: ["package=ctranslate2==4.8.1", "source_repo=OpenNMT/CTranslate2", "source_ref=v4.8.1", "source_commit=0d8bcd362ac75ef860ef161d6f0efad0ae439ff0"],
    files: { "LICENSE": "54aa79d9fe3c09e67a16dcd95b9e88676405a6ec174efda31036983cf7672ecb" },
  },
  "flatbuffers-25.12.19.dist-info": {
    source: ["package=flatbuffers==25.12.19", "source_repo=google/flatbuffers", "source_ref=v25.12.19", "source_commit=7e163021e59cca4f8e1e35a7c828b5c6b7915953"],
    files: { "LICENSE": "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30" },
  },
  "jieba-0.42.1.dist-info": {
    source: ["package=jieba==0.42.1", "sdist_sha256=055ca12f62674fafed09427f176506079bc135638a14e23e25be909131928db2", "source_repo=fxsjy/jieba", "source_ref=v0.42.1", "source_commit=1e20c89b66f56c9301b0feed211733ffaa1bd72a"],
    files: { "LICENSE": "18ba0984839f85853b29fadaf992f7dba8fd0ca0fbeae34de2b8735222dc7a37" },
  },
  "jieba_fast-0.53.dist-info": {
    source: ["package=jieba-fast==0.53", "sdist_sha256=e92089d52faa91d51b6a7c1e6e4c4c85064a0e36f6a29257af2254b9e558ddd0", "source_repo=deepcs233/jieba_fast", "source_commit=5e6b21dece184e1004a35bfd802c8772059de3ab", "upstream_release_tag=none"],
    files: { "LICENSE": "18ba0984839f85853b29fadaf992f7dba8fd0ca0fbeae34de2b8735222dc7a37" },
  },
  "loguru-0.7.3.dist-info": {
    source: ["package=loguru==0.7.3", "sdist_sha256=19480589e77d47b8d85b2c827ad95d49bf31b0dcde16593892eb51dd18706eb6", "source_repo=Delgan/loguru", "source_ref=0.7.3", "source_commit=ae3bfd1b85b6b4a3db535f69b975687c79498be4"],
    files: { "LICENSE": "b35d026cc7aca9d5859a02eb87ddf7a386a24c986838651bd1f283f94e003327" },
  },
  "onnxruntime-1.28.0.dist-info": {
    source: ["package=onnxruntime==1.28.0", "source_repo=microsoft/onnxruntime", "source_ref=v1.28.0", "source_commit=da9b5e364c465de65c49d91e696cd6485270757f"],
    files: {
      "LICENSE": "2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c",
      "ThirdPartyNotices.txt": "0e07b95f3a8d6230037707c5c4a2b554d12c4cb67369669ac255635528ffcee2",
    },
  },
  "sentencepiece-0.2.2.dist-info": {
    source: ["package=sentencepiece==0.2.2", "sdist_sha256=3d2b5e824b5622038dc7b490897efe05ebbbb9e7350fc142f3ecc8789ef9bdf6"],
    files: {
      "LICENSE": "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
      "ABSEIL_LICENSE": "c79a7fea0e3cac04cd43f20e7b648e5a0ff8fa5344e644b0ee09ca1162b62747",
      "DARTS_CLONE_LICENSE": "155f59997298ee336602c49f9c1110f268ac394ca2197eb02647a3555935ad52",
      "ESAXX_LICENSE": "7c28553d1d3312d65fe309f76a22ebaf33a3d76c8c1e3b98a88ee4654ecb53db",
      "PROTOBUF_LITE_LICENSE": "6e5e117324afd944dcf67f36cf329843bc1a92229a8cd9bb573d7a83130fea7d",
    },
  },
  "tensorboard_data_server-0.7.2.dist-info": {
    source: ["package=tensorboard-data-server==0.7.2", "source_repo=tensorflow/tensorboard", "source_commit=81150b898a306b89cde90e949358c2eefe018eaa", "source_commit_message=tensorboard-data-server 0.7.2"],
    files: { "LICENSE": "d7c9068d896188264b60827b8cd1e25fdb9f5b5cad0b1589e90b96c87729c404" },
  },
  "tokenizers-0.22.2.dist-info": {
    source: ["package=tokenizers==0.22.2", "sdist_sha256=473b83b915e547aa366d1eee11806deaf419e17be16310ac0a14077f1e28f917"],
    files: { "LICENSE": "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4" },
  },
  "wordsegment-1.3.1.dist-info": {
    source: ["package=wordsegment==1.3.1", "sdist_sha256=3dcc7cd1e9bba3f3ffe6a0e54d98377bc502fc34e9e9d8c8199ac5636924f023"],
    files: { "LICENSE": "8fe4d37c518608a57c6b0f24e26915144f8daf460eb4e1572146596ec3673294" },
  },
};

const expectedVbCableFiles = {
  "pin_in.ico": "934865449455103c1c5997d8220acd160c3891f8a870f8e745b743d12681ac42",
  "pin_out.ico": "e8728a811e1f1af7d2ba31f77e47d449d5bba091e3e89a0df325ac7a3e67652c",
  "readme.txt": "f865f3e78e37006d48e56c93f51eff4ca79acda9969854400d79bfa3db38a8d5",
  "vbaudio_cable_2003.cat": "23a10e3bcd6ffe0de6d3d67830be4c447724b3299e13e954dd6bd2257cf55da1",
  "vbaudio_cable_2003.sys": "9650e20c38429d4680a7e603ecbe6601914ec4c94f8473112f21515bf53b84cf",
  "vbaudio_cable_vista.cat": "810b30193a400b1559302c23f81ef8aaadf038b3caf3aff9bf200b59688a2def",
  "vbaudio_cable_vista.sys": "8b02c26313b75ceb8fb9bd16b6b167cf70d7f3bc977dfc1986c0859f8c72b49f",
  "vbaudio_cable_win7.cat": "1c38afacf115818c925bc26faf216e3563e85bbc4d6d793e5f76f2aa670d08e7",
  "vbaudio_cable_win7.sys": "d047e3ee66e3ee023e598232ea22aa28dbb39adeabe818adfb2a72ab738df0b9",
  "vbaudio_cable_xp.cat": "51434ebbda13caf3c7617ef3035126baacca3e523feaf6828b398d52503d70d2",
  "vbaudio_cable_xp.sys": "9f9bc80a96cc94c761887749e51d9b4fb7ee6741ce624b8b2f9f85cd2e3fb02e",
  "vbaudio_cable64_2003.cat": "70f88e34c857c999367eb5b0303e23f5676b15a79ed4054f374749232baa0e3a",
  "vbaudio_cable64_2003.sys": "5a726f3f1616f587c9f118bf337fd359df3f7cb9fc967a14229d59a31f2a9720",
  "vbaudio_cable64_vista.cat": "cc17731e91f2f071e4a108bba34a2b8dcb69ea5975f4b9c39fcbc670e5cf15fb",
  "vbaudio_cable64_vista.sys": "703572fa9e8aa1616e6b2abd36b91a4718de77eadc02ae05ed2c8f304d058afc",
  "vbaudio_cable64_win10.cat": "0a921ebadbe39cf3fa7c14ff37d1ca22565a9651244bd3ac177dc810bc99072e",
  "vbaudio_cable64_win10.sys": "f01344602472f1b527de5ee98f18987c03db48fea444e457d061e937f1d531d5",
  "vbaudio_cable64_win7.cat": "800b541f06bba3925ba058e7cc7ca837cfd4d845e073309eb2a9d36a2626403a",
  "vbaudio_cable64_win7.sys": "c7f3be383c81ab9aa642479f95872e40e19a4cfd72d4c8d7de80abc11b713e21",
  "vbaudio_cable64arm_win10.sys": "2dc35db3dfad0f25771a3e59af38e8b1268878ebe127476ea75fee109f2927dd",
  "VBCABLE_ControlPanel.exe": "f5b44706fe7ba2eed0516dee791f826dc7a9891e997f6ce9704e2899300b14ff",
  "VBCABLE_Setup_x64.exe": "734c35dfa6d98f48782a451633ceb471166ec70d60482fd89a1123d0ee3c4f41",
  "VBCABLE_Setup.exe": "01ffc86b623ff3c75a883aa900c0215a89482988e1c8e55988fc0a9fb513dbed",
  "vbMmeCable_2003.inf": "64b67f80535d92a1a8625b4c9b9f7302ed959cb375947ca993b8cbaf205d3569",
  "vbMmeCable_vista.inf": "50761a7e817b3a5e96a4eb8e3d31fbc249b0601343dcb732dd3cbe0b0a70f232",
  "vbMmeCable_win7.inf": "5664f33116c1021f4280cfde1c571554fbb70b5480bd58a4fd53b281cd4f515c",
  "vbMmeCable_xp.inf": "58d9737fa732c11c8cc52839a3f61ecf2cb2a98a7dfffe423e3e591de7f56d46",
  "vbMmeCable64_2003.inf": "73aa40eef245da221c6fc6ea3299983421c9a9051df8da7414652304f01bb835",
  "vbMmeCable64_vista.inf": "340feb0ce66ffb7922595a763bf23d2fec07bed9e50b6cb6327e559174c515d4",
  "vbMmeCable64_win10.inf": "61c857be74831cc299d9be62f8d49d137f14063454fc54f859d5bc9b4b813daf",
  "vbMmeCable64_win7.inf": "da35387ccfe813f5c553bb7e0caf4e67adbb4429e742c2bd3c2014f80e6ec516",
};

const errors = [];
const fail = (message) => errors.push(message);
const requireFile = (path, label = path) => {
  if (!existsSync(path) || !statSync(path).isFile() || statSync(path).size <= 0) {
    fail(`Missing required release file: ${label}`);
  }
};
const requireDir = (path, label = path) => {
  if (!existsSync(path) || !statSync(path).isDirectory()) {
    fail(`Missing required release directory: ${label}`);
  }
};
const sha256File = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const parseKeyValueRecord = (text) => {
  const entries = String(text)
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const separator = line.indexOf("=");
      return separator > 0 ? [line.slice(0, separator), line.slice(separator + 1)] : null;
    })
    .filter(Boolean);
  return Object.fromEntries(entries);
};
const hasAnyFile = (root) => {
  if (!existsSync(root) || !statSync(root).isDirectory()) return false;
  const stack = [root];
  while (stack.length) {
    const current = stack.pop();
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isFile() && statSync(path).size > 0) return true;
      if (entry.isDirectory()) stack.push(path);
    }
  }
  return false;
};

for (const file of [
  "realtime_local_worker.py",
  "realtime_local_worker_base.py",
  "milmmt_translation_provider.py",
  "worker_runtime_common.py",
  "worker_io_runtime.py",
  "translation_envelope.py",
  "voice_lab_build.py",
  "voice_lab_gpt_sovits.py",
  "voice_lab_upstream_stage.py",
  "model_manifest.json",
]) {
  requireFile(join(workerRoot, file), `WorkerRuntime/${file}`);
}

requireFile(join(pythonRoot, "python.exe"), "LocalWorker/PythonRuntime/python.exe");
requireFile(join(pythonRoot, "LICENSE.txt"), "LocalWorker/PythonRuntime/LICENSE.txt");
requireFile(join(pythonRoot, "PYTHON_SOURCE.txt"), "LocalWorker/PythonRuntime/PYTHON_SOURCE.txt");
requireFile(join(pythonRoot, "python312._pth"), "LocalWorker/PythonRuntime/python312._pth");
if (existsSync(join(pythonRoot, "PYTHON_SOURCE.txt"))) {
  const sourceRecord = readFileSync(join(pythonRoot, "PYTHON_SOURCE.txt"), "utf8");
  for (const marker of [
    "source_kind=cpython-embeddable",
    "release=3.12.10",
    "source_url=https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip",
    "archive_bytes=11133606",
    "archive_md5=fe8ef205f2e9c3ba44d0cf9954e1abd3",
    `archive_sha256=${expectedPythonArchiveSha256}`,
  ]) {
    if (!sourceRecord.includes(marker)) fail(`PYTHON_SOURCE.txt provenance marker is missing: ${marker}`);
  }
}
if (existsSync(join(pythonRoot, "python312._pth"))) {
  const activePaths = readFileSync(join(pythonRoot, "python312._pth"), "utf8")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"));
  if (JSON.stringify(activePaths) !== JSON.stringify(["python312.zip", ".", "..\\WorkerRuntime"])) {
    fail("PythonRuntime/python312._pth must expose only python312.zip, vendored PythonRuntime packages, and canonical sibling WorkerRuntime.");
  }
  if (activePaths.some((line) => line.toLowerCase() === "import site")) {
    fail("PythonRuntime/python312._pth must keep import site disabled; system/user site-packages are not release dependencies.");
  }
}

for (const [distInfoName, requirement] of Object.entries(exceptionalPythonLicenseMaterials)) {
  const distInfoRoot = join(pythonRoot, distInfoName);
  const licensesRoot = join(distInfoRoot, "licenses");
  requireDir(distInfoRoot, `LocalWorker/PythonRuntime/${distInfoName}`);
  requireDir(licensesRoot, `LocalWorker/PythonRuntime/${distInfoName}/licenses`);
  const sourcePath = join(licensesRoot, "TRANSLATEIT_SOURCE.txt");
  requireFile(sourcePath, `${distInfoName}/licenses/TRANSLATEIT_SOURCE.txt`);
  if (existsSync(sourcePath)) {
    const sourceRecord = readFileSync(sourcePath, "utf8");
    for (const marker of requirement.source) {
      if (!sourceRecord.includes(marker)) fail(`${distInfoName} exceptional license source marker is missing: ${marker}`);
    }
  }
  for (const [file, expectedHash] of Object.entries(requirement.files)) {
    const path = join(licensesRoot, file);
    requireFile(path, `${distInfoName}/licenses/${file}`);
    if (existsSync(path) && sha256File(path) !== expectedHash) {
      fail(`${distInfoName} exceptional license material hash mismatch: ${file}`);
    }
  }
}

const noticeBundlePath = thirdPartyNoticeOutputPath(backendRoot);
requireFile(noticeBundlePath, "RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt");
if (existsSync(noticeBundlePath)) {
  try {
    const expectedNotices = buildThirdPartyNoticeBundle({ backendRoot }).content;
    const actualNotices = readFileSync(noticeBundlePath, "utf8").replace(/\r\n/g, "\n").replace(/\r/g, "\n").trimEnd() + "\n";
    if (actualNotices !== expectedNotices) fail("Third-party notice bundle is stale or does not match the staged release payload.");
  } catch (error) {
    fail(`Third-party notice bundle cannot be validated: ${error instanceof Error ? error.message : String(error)}`);
  }
}

const manifestPath = join(workerRoot, "model_manifest.json");
if (existsSync(manifestPath)) {
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    fail("WorkerRuntime/model_manifest.json is not valid JSON.");
  }
  if (manifest) {
    if (manifest.schema !== "translateit.local_model_inventory.v2" || manifest.inventory_scope !== "full_product_release_assets") {
      fail("WorkerRuntime/model_manifest.json is not the canonical full-product release inventory.");
    }
    for (const model of manifest.models ?? []) {
      if (model.required !== true) continue;
      const expectedPath = String(model.expected_path ?? "").trim();
      if (!expectedPath.startsWith("EngineData/Backend/RuntimeAssets/")) {
        fail(`Required model ${model.model_id ?? "<unknown>"} has an invalid release path.`);
        continue;
      }
      const relative = expectedPath.slice("EngineData/Backend/RuntimeAssets/".length);
      const target = join(runtimeAssetsRoot, ...relative.split("/"));
      if (!existsSync(target) || (!statSync(target).isFile() && !hasAnyFile(target))) {
        fail(`Required release asset is missing or empty: ${model.model_id} -> ${expectedPath}`);
      }
    }
  }
}

requireFile(join(voiceSourceRoot, "TRANSLATEIT_GPTSOVITS_REVISION.txt"), "GPT-SoVITS revision marker");

const builtinVoiceRoot = join(runtimeAssetsRoot, "Voice", "BuiltInVoices");
for (const builtin of ["MaleVoice", "FemaleVoice"]) {
  requireDir(join(builtinVoiceRoot, builtin), `BuiltInVoices/${builtin}`);
  requireFile(join(builtinVoiceRoot, builtin, "reference.wav"), `BuiltInVoices/${builtin}/reference.wav`);
  requireFile(join(builtinVoiceRoot, builtin, "REFERENCE_SOURCE.txt"), `BuiltInVoices/${builtin}/REFERENCE_SOURCE.txt`);
}
requireFile(join(builtinVoiceRoot, "SOURCES.json"), "BuiltInVoices/SOURCES.json");
if (existsSync(join(builtinVoiceRoot, "SOURCES.json"))) {
  const sourcesJsonText = readFileSync(join(builtinVoiceRoot, "SOURCES.json"), "utf8").replace(/^\uFEFF/, "");
  const sources = JSON.parse(sourcesJsonText);
  if (sources.schema !== "translateit.builtin_voice_sources.v1") fail("builtin_voice_sources_schema");
  for (const voice of sources.voices ?? []) {
    const path = join(builtinVoiceRoot, voice.voice_id, "reference.wav");
    if (!existsSync(path)) continue;
    const actual = createHash("sha256").update(readFileSync(path)).digest("hex");
    if (actual !== String(voice.wav_sha256).toLowerCase()) fail(`builtin_voice_hash_mismatch:${voice.voice_id}`);
  }
}
if (existsSync(join(voiceSourceRoot, "TRANSLATEIT_GPTSOVITS_REVISION.txt"))) {
  const revision = readFileSync(join(voiceSourceRoot, "TRANSLATEIT_GPTSOVITS_REVISION.txt"), "utf8").trim();
  if (revision !== expectedRevision) fail("GPT-SoVITS release payload revision does not match the approved V2ProPlus pin.");
}
const nltkSourcePath = join(voiceSourceRoot, "NLTK_DATA_SOURCE.txt");
requireFile(nltkSourcePath, "GPT-SoVITS/Source/NLTK_DATA_SOURCE.txt");
if (existsSync(nltkSourcePath)) {
  const sourceRecord = readFileSync(nltkSourcePath, "utf8");
  for (const marker of [
    "source_kind=nltk_data",
    "repository=nltk/nltk_data",
    `revision=${expectedNltkRevision}`,
    ...Object.entries(expectedNltkPackages).map(([name, hash]) => `${name} sha256=${hash}`),
  ]) {
    if (!sourceRecord.includes(marker)) fail(`NLTK_DATA_SOURCE.txt provenance marker is missing: ${marker}`);
  }
}
const ffmpegPath = join(voiceSourceRoot, "ffmpeg.exe");
const ffmpegLicensePath = join(voiceSourceRoot, "FFMPEG_LICENSE.txt");
const ffmpegSourcePath = join(voiceSourceRoot, "FFMPEG_SOURCE.txt");
requireFile(ffmpegPath, "GPT-SoVITS/Source/ffmpeg.exe");
requireFile(ffmpegLicensePath, "GPT-SoVITS/Source/FFMPEG_LICENSE.txt");
requireFile(ffmpegSourcePath, "GPT-SoVITS/Source/FFMPEG_SOURCE.txt");
if (existsSync(ffmpegLicensePath)) {
  const ffmpegLicense = readFileSync(ffmpegLicensePath, "utf8");
  if (!ffmpegLicense.includes("GNU LESSER GENERAL PUBLIC LICENSE") || !ffmpegLicense.includes("Version 3, 29 June 2007")) {
    fail("FFMPEG_LICENSE.txt does not contain the expected LGPL v3 license text.");
  }
}
if (existsSync(ffmpegSourcePath)) {
  const sourceRecord = readFileSync(ffmpegSourcePath, "utf8");
  const source = parseKeyValueRecord(sourceRecord);
  for (const [key, expected] of Object.entries({
    source_kind: "ffmpeg",
    binary_builder: "BtbN/FFmpeg-Builds",
    builder_release_api: "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/autobuild-2026-09-07-15-39",
    builder_release_id: "384189644",
    builder_release_tag: "autobuild-2026-09-07-15-39",
    builder_release_name: "Auto-Build 2026-09-07 15:39",
    builder_release_published_at: "2026-09-07T15:40:02Z",
    asset_id: "549007162",
    archive: "ffmpeg-n8.1.2-51-g7ba069f4f1-win64-lgpl-8.1.zip",
    license_profile: "LGPL-3.0-or-later",
    build_profile: "win64-lgpl-static",
    integrity_source: "pinned_release_asset_sha256",
  })) {
    if (source[key] !== expected) fail(`FFMPEG_SOURCE.txt provenance field mismatch: ${key}`);
  }
  for (const key of ["download_url", "ffmpeg_version_line"]) {
    if (!String(source[key] ?? "").trim()) fail(`FFMPEG_SOURCE.txt provenance field is missing: ${key}`);
  }
  for (const key of ["archive_sha256", "ffmpeg_exe_sha256", "ffmpeg_license_sha256"]) {
    if (!/^[0-9a-f]{64}$/.test(String(source[key] ?? ""))) fail(`FFMPEG_SOURCE.txt SHA-256 field is invalid: ${key}`);
  }
  if (!String(source.download_url ?? "").startsWith("https://github.com/BtbN/FFmpeg-Builds/releases/download/")) {
    fail("FFMPEG_SOURCE.txt download_url is not a BtbN GitHub release asset.");
  }
  if (existsSync(ffmpegPath) && /^[0-9a-f]{64}$/.test(String(source.ffmpeg_exe_sha256 ?? "")) && sha256File(ffmpegPath) !== source.ffmpeg_exe_sha256) {
    fail("GPT-SoVITS/Source/ffmpeg.exe does not match the SHA-256 recorded by controlled staging.");
  }
  if (existsSync(ffmpegLicensePath) && /^[0-9a-f]{64}$/.test(String(source.ffmpeg_license_sha256 ?? "")) && sha256File(ffmpegLicensePath) !== source.ffmpeg_license_sha256) {
    fail("GPT-SoVITS/Source/FFMPEG_LICENSE.txt does not match the SHA-256 recorded by controlled staging.");
  }
  if (existsSync(ffmpegPath)) {
    const version = spawnSync(ffmpegPath, ["-hide_banner", "-version"], { encoding: "utf8", windowsHide: true });
    const versionText = `${version.stdout ?? ""}\n${version.stderr ?? ""}`.trim();
    const versionLine = versionText.split(/\r?\n/)[0]?.trim() ?? "";
    if (version.status !== 0 || !/^ffmpeg version n?8\.1(?:\.|\s|-)/.test(versionLine)) {
      fail(`Staged FFmpeg does not report the approved n8.1 stable line: ${versionLine || version.status}`);
    }
    if (source.ffmpeg_version_line && versionLine !== source.ffmpeg_version_line) {
      fail("FFMPEG_SOURCE.txt ffmpeg_version_line does not match the staged executable.");
    }
    const build = spawnSync(ffmpegPath, ["-hide_banner", "-buildconf"], { encoding: "utf8", windowsHide: true });
    const buildText = `${build.stdout ?? ""}\n${build.stderr ?? ""}`;
    if (build.status !== 0) fail("Staged FFmpeg build configuration could not be inspected.");
    if (/(?:^|\s)--enable-(?:gpl|nonfree)(?:\s|$)/m.test(buildText)) {
      fail("Staged FFmpeg unexpectedly enables GPL or nonfree build options.");
    }
    if (!/(?:^|\s)--enable-version3(?:\s|$)/m.test(buildText)) {
      fail("Staged FFmpeg does not expose the expected LGPL version3 build contract.");
    }
  }
}
for (const dir of [
  ["GPT_SoVITS", "GPT-SoVITS core source"],
  ["nltk_data/corpora/cmudict", "NLTK cmudict"],
  ["nltk_data/taggers/averaged_perceptron_tagger", "NLTK perceptron tagger"],
  ["nltk_data/taggers/averaged_perceptron_tagger_eng", "NLTK English perceptron tagger"],
]) {
  requireDir(join(voiceSourceRoot, ...dir[0].split("/")), dir[1]);
}

const forbiddenVoicePayload = [
  "webui.py",
  "api.py",
  "api_v2.py",
  "GPT_SoVITS/inference_webui.py",
  "tools/asr",
  "tools/uvr5",
  "tools/subfix_webui.py",
  "ffplay.exe",
  "ffprobe.exe",
];
for (const relative of forbiddenVoicePayload) {
  const target = join(voiceSourceRoot, ...relative.split("/"));
  if (existsSync(target)) fail(`Unapproved GPT-SoVITS WebUI/server/auxiliary payload must not be bundled: ${relative}`);
}

requireFile(join(vbCableRoot, "NOTICE.txt"), "AudioProvider/VBCABLE/NOTICE.txt");
if (existsSync(join(vbCableRoot, "NOTICE.txt"))) {
  const notice = readFileSync(join(vbCableRoot, "NOTICE.txt"), "utf8").toLowerCase();
  for (const marker of [
    "vb-audio",
    "donationware",
    "vb-cable",
    "https://vb-cable.com/",
    "https://shop.vb-audio.com/en/win-apps/11-vb-cable.html",
    "managed professional/company/institution",
  ]) {
    if (!notice.includes(marker)) fail(`VB-CABLE distribution notice marker is missing: ${marker}`);
  }
}
requireDir(vbCablePackageRoot, "AudioProvider/VBCABLE/Package");
if (existsSync(vbCablePackageRoot) && statSync(vbCablePackageRoot).isDirectory()) {
  const entries = readdirSync(vbCablePackageRoot, { withFileTypes: true });
  const actualNames = entries.map((entry) => entry.name).sort((a, b) => a.localeCompare(b));
  const expectedNames = Object.keys(expectedVbCableFiles).sort((a, b) => a.localeCompare(b));
  if (entries.some((entry) => !entry.isFile()) || JSON.stringify(actualNames) !== JSON.stringify(expectedNames)) {
    fail("AudioProvider/VBCABLE/Package must be the exact flat 31-file extraction of reviewed VBCABLE_Driver_Pack45.zip.");
  }
  for (const [name, expectedHash] of Object.entries(expectedVbCableFiles)) {
    const path = join(vbCablePackageRoot, name);
    requireFile(path, `AudioProvider/VBCABLE/Package/${name}`);
    if (existsSync(path) && sha256File(path) !== expectedHash) {
      fail(`VB-CABLE Pack45 file does not match reviewed official bytes: ${name}`);
    }
  }
}

if (errors.length) {
  for (const error of errors) console.error(`[release-payload] ${error}`);
  console.error("[release-payload] Release payload is incomplete or contains unapproved baggage. Prepare the controlled runtime assets before building the installer.");
  process.exit(1);
}

console.log("[release-payload] Required private Python runtime, canonical modular WorkerRuntime, release model inventory, deterministic third-party notice bundle, pruned GPT-SoVITS VoiceLab payload, integrity-bound FFmpeg LGPL executable/license/source record, and standard VB-CABLE provider package are present for Tauri/NSIS staging. This is controlled payload-input proof only, not whole-release legal, driver-install, installed-runtime, or clean-machine proof.");

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
# The release wrapper reserves a unique private stage. This implementation
# never prepares or deletes the canonical runtime directories until promotion.
$Temp = $env:TRANSLATEIT_RELEASE_STAGE_ROOT
if (-not $Temp -or -not $Temp.StartsWith((Join-Path $RepoRoot '.workspace\staging') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Release input staging must start from stage_release_inputs.ps1.'
}
$PayloadRoot = Join-Path $Temp 'payload-tree'
$SourceBackend = Join-Path $RepoRoot 'EngineData\Backend'
$Worker = Join-Path $SourceBackend 'LocalWorker\WorkerRuntime'
$Backend = Join-Path $PayloadRoot 'EngineData\Backend'
$PythonRoot = Join-Path $Backend 'LocalWorker\PythonRuntime'
$Assets = Join-Path $Backend 'RuntimeAssets'
$AsrRoot = Join-Path $Assets 'ASR\ModelData'
$TranslationRoot = Join-Path $Assets 'Translation\ModelData'
$VoiceSource = Join-Path $Assets 'Voice\GPTSoVITS\Source'
$VbPackage = Join-Path $Assets 'AudioProvider\VBCABLE\Package'
# Runtime asset and release staging files belong to the same chosen repository root.
# A hosted runner may use its disposable runner temp, never the user's data root.
$env:UV_CACHE_DIR = Join-Path $RepoRoot '.workspace\cache\uv'
New-Item -ItemType Directory -Force -Path $env:UV_CACHE_DIR | Out-Null

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Assert-Hash([string]$Path, [string]$Expected, [string]$Label) {
    $actual = Get-Sha256 $Path
    if ($actual -ne $Expected.ToLowerInvariant()) { throw "$Label SHA-256 mismatch: $actual" }
    Write-Host "[release-stage][hash] $Label=$actual"
}

function Assert-TextEquals([string]$Path, [string]$Expected, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "$Label missing: $Path" }
    $actual = (Get-Content -LiteralPath $Path -Raw).Trim()
    if ($actual -ne $Expected) { throw "$Label mismatch: $actual" }
    Write-Host "[release-stage][marker] $Label=$actual"
}

Import-Module (Join-Path $PSScriptRoot 'ReleaseDownloads.psm1') -Force
$ReleaseDownloadCache = Join-Path $RepoRoot '.workspace\downloads\verified'
function Invoke-Download([string]$Url, [string]$Path, [string]$ExpectedSha256 = '') {
    Write-Host "[release-stage][download] $Url"
    if ($ExpectedSha256) {
        $null = Get-VerifiedReleaseArchive -Url $Url -Destination $Path -ExpectedSha256 $ExpectedSha256 -CacheRoot $ReleaseDownloadCache
    } else {
        # GPT-SoVITS source archive currently has no independent archive hash
        # contract; never cache an unchecked download as a verified input.
        Invoke-WebRequest -Uri $Url -OutFile $Path
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Download missing: $Url" }
}

function Copy-Tree([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    & robocopy $Source $Destination /E /NFL /NDL /NJH /NJS /NP | Out-Null
    $code = $LASTEXITCODE
    if ($code -gt 7) { throw "robocopy failed ($code): $Source -> $Destination" }
}

New-Item -ItemType Directory -Path $Temp -ErrorAction Stop | Out-Null
foreach ($path in @($PythonRoot, $AsrRoot, $TranslationRoot, $VoiceSource, $VbPackage)) {
    New-Item -ItemType Directory -Force -Path $path | Out-Null
}

Write-Host '[release-stage] Stage CPython 3.12.10 embeddable runtime'
$pythonZip = Join-Path $Temp 'python-3.12.10-embed-amd64.zip'
Invoke-Download 'https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip' $pythonZip '4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3'
$pythonBytes = (Get-Item -LiteralPath $pythonZip).Length
if ($pythonBytes -ne 11133606) { throw "CPython archive byte count mismatch: $pythonBytes" }
$pythonMd5 = (Get-FileHash -Algorithm MD5 -LiteralPath $pythonZip).Hash.ToLowerInvariant()
if ($pythonMd5 -ne 'fe8ef205f2e9c3ba44d0cf9954e1abd3') { throw "CPython official MD5 mismatch: $pythonMd5" }
$pythonSha = Get-Sha256 $pythonZip
if ($pythonSha -ne '4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3') { throw "CPython SHA-256 mismatch: $pythonSha" }
Expand-Archive -LiteralPath $pythonZip -DestinationPath $PythonRoot -Force
@('python312.zip', '.', '..\WorkerRuntime') | Set-Content -LiteralPath (Join-Path $PythonRoot 'python312._pth') -Encoding ascii
# The private Python _pth deliberately imports its sibling WorkerRuntime.
# Mirror only immutable source/config files for staged import verification;
# never copy developer .venv, caches, tests, models or personal voice data.
$StagedWorker = Join-Path $Backend 'LocalWorker\WorkerRuntime'
New-Item -ItemType Directory -Force -Path $StagedWorker | Out-Null
Get-ChildItem -LiteralPath $Worker -File | Where-Object { $_.Extension -in @('.py', '.json') } |
    Copy-Item -Destination $StagedWorker -Force
if (-not (Test-Path -LiteralPath (Join-Path $StagedWorker 'voice_lab_gpt_sovits.py') -PathType Leaf)) {
    throw 'Staged private Python WorkerRuntime source closure missing.'
}
& (Join-Path $PythonRoot 'python.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Embedded CPython did not start.' }
@"
source_kind=cpython-embeddable
release=3.12.10
source_url=https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip
archive_bytes=$pythonBytes
archive_md5=$pythonMd5
archive_sha256=$pythonSha
"@ | Set-Content -LiteralPath (Join-Path $PythonRoot 'PYTHON_SOURCE.txt') -Encoding ascii

Write-Host '[release-stage] Build isolated frozen release Python dependency environment'
$hostPython = (Get-Command python -ErrorAction Stop).Source
$null = Get-Command uv -ErrorAction Stop
$stagingVenv = Join-Path $Temp 'worker-release-venv'
$previousProjectEnvironment = [Environment]::GetEnvironmentVariable('UV_PROJECT_ENVIRONMENT', 'Process')
try {
    # Never mutate WorkerRuntime/.venv or the system Python/pip environment.
    $env:UV_PROJECT_ENVIRONMENT = $stagingVenv
    Push-Location $Worker
    try {
        uv sync --frozen --no-dev --python $hostPython
        if ($LASTEXITCODE -ne 0) { throw 'Isolated release uv sync failed.' }
    } finally { Pop-Location }
} finally {
    [Environment]::SetEnvironmentVariable('UV_PROJECT_ENVIRONMENT', $previousProjectEnvironment, 'Process')
}
$stagePython = Join-Path $stagingVenv 'Scripts\python.exe'
if (-not (Test-Path -LiteralPath $stagePython -PathType Leaf)) { throw 'Isolated release Python missing.' }
& $stagePython -c "import huggingface_hub, torch; print('[release-stage][venv] isolated release imports PASS', torch.__version__)"
if ($LASTEXITCODE -ne 0) { throw 'Isolated release Python imports failed.' }
$sitePackages = Join-Path $stagingVenv 'Lib\site-packages'
if (-not (Test-Path -LiteralPath $sitePackages -PathType Container)) { throw 'Frozen WorkerRuntime site-packages missing.' }
Copy-Tree $sitePackages $PythonRoot
$privatePython = Join-Path $PythonRoot 'python.exe'
& $privatePython -s -c "import torch, torchaudio, transformers, ctranslate2, faster_whisper, sentencepiece, soundfile, numpy, voice_lab_gpt_sovits; print('[release-stage][python] private imports PASS', torch.__version__, transformers.__version__)"
if ($LASTEXITCODE -ne 0) { throw 'Private Python runtime import smoke failed.' }

Write-Host '[release-stage] Acquire required Hugging Face models from canonical manifest'
$env:HF_HUB_DISABLE_TELEMETRY = '1'
$env:HF_HOME = Join-Path $RepoRoot '.workspace\cache\huggingface'
$env:HF_HUB_CACHE = Join-Path $env:HF_HOME 'hub'
$env:HF_XET_CACHE = Join-Path $env:HF_HOME 'xet'
New-Item -ItemType Directory -Force -Path $env:HF_HOME | Out-Null
$modelAcquirer = Join-Path $Worker 'prepare_model_assets.py'
& $stagePython $modelAcquirer --model-id 'faster-whisper-large-v3-turbo' --model-id 'milmmt-46-1b-v1.0' --destination-root $PayloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Canonical required model acquisition failed.' }

$asrRevisionMarker = Join-Path $AsrRoot 'faster-whisper-large-v3-turbo\.translateit_model_revision'
$milmmtRoot = Join-Path $TranslationRoot 'xiaomi-research--MiLMMT-46-1B-v1.0'
$milmmtRevisionMarker = Join-Path $milmmtRoot '.translateit_model_revision'
Assert-TextEquals $asrRevisionMarker '0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf' 'ASR revision'
Assert-TextEquals $milmmtRevisionMarker '4fc480b6c58dec29c159dcdf9fde0f6d5c354995' 'MiLMMT revision'
foreach ($requiredModelFile in @(
    'config.json',
    'generation_config.json',
    'model.safetensors',
    'tokenizer.json',
    'tokenizer.model',
    'tokenizer_config.json'
)) {
    $requiredPath = Join-Path $milmmtRoot $requiredModelFile
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "MiLMMT required file missing: $requiredModelFile"
    }
}

Write-Host '[release-stage] Verify pinned model artifact SHA-256 from model_manifest.json'
$manifest = Get-Content -LiteralPath (Join-Path $Worker 'model_manifest.json') -Raw | ConvertFrom-Json
foreach ($model in $manifest.models) {
    if ($model.required -ne $true) { continue }
    if ($model.source_type -ne 'huggingface') { continue }
    if (-not $model.artifact_hashes) { throw "artifact_hashes_missing:$($model.model_id)" }
    $modelRoot = Join-Path $PayloadRoot ($model.expected_path -replace '/', '\')
    foreach ($property in $model.artifact_hashes.PSObject.Properties) {
        $artifactPath = Join-Path $modelRoot ($property.Name -replace '/', '\')
        if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
            throw "artifact file missing: $($model.model_id)/$($property.Name)"
        }
        $actual = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $property.Value.ToLower()) {
            throw "artifact sha256 mismatch: $($model.model_id)/$($property.Name)"
        }
    }
}
Write-Host '[release-stage][sha256] ASR + MiLMMT artifact hashes PASS'
Write-Host '[release-stage] Verify repository-owned built-in voice references'
$builtInRoot = Join-Path $RepoRoot 'EngineData\Backend\RuntimeAssets\Voice\BuiltInVoices'
$builtInSources = Join-Path $builtInRoot 'SOURCES.json'
if (-not (Test-Path -LiteralPath $builtInSources -PathType Leaf)) { throw 'BuiltInVoices/SOURCES.json missing' }
$builtInRecord = Get-Content -LiteralPath $builtInSources -Raw | ConvertFrom-Json
if ($builtInRecord.schema -ne 'translateit.builtin_voice_sources.v1') { throw 'BuiltInVoices source schema mismatch' }
$expectedVoiceIds = @('MaleVoice', 'FemaleVoice')
$actualVoiceIds = @($builtInRecord.voices | ForEach-Object { $_.voice_id })
if (@($actualVoiceIds | Sort-Object -Unique).Count -ne 2 -or
    @($actualVoiceIds | Sort-Object).Count -ne 2 -or
    @(Compare-Object ($expectedVoiceIds | Sort-Object) ($actualVoiceIds | Sort-Object)).Count -ne 0) {
    throw 'BuiltInVoices manifest must contain exactly MaleVoice and FemaleVoice'
}
foreach ($voice in $builtInRecord.voices) {
    if ([string]$voice.wav_sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw "Invalid BuiltInVoices SHA-256: $($voice.voice_id)" }
    $reference = Join-Path $builtInRoot "$($voice.voice_id)\reference.wav"
    $attribution = Join-Path $builtInRoot "$($voice.voice_id)\REFERENCE_SOURCE.txt"
    if (-not (Test-Path -LiteralPath $reference -PathType Leaf)) { throw "BuiltInVoices reference missing: $($voice.voice_id)" }
    if (-not (Test-Path -LiteralPath $attribution -PathType Leaf)) { throw "BuiltInVoices attribution missing: $($voice.voice_id)" }
    Assert-Hash $reference ([string]$voice.wav_sha256) "BuiltInVoices/$($voice.voice_id)/reference.wav"
}
Write-Host '[release-stage] BuiltInVoices tracked source and hashes PASS'

Write-Host '[release-stage] Stage pinned GPT-SoVITS pretrained Hugging Face assets'
$env:GPT_ASSET_OUT = Join-Path $Temp 'gpt-assets'
$downloadGptAssets = Join-Path $Temp 'download-gpt-assets.py'
@'
import os
import shutil
from pathlib import Path
from huggingface_hub import snapshot_download

out = Path(os.environ["GPT_ASSET_OUT"])
snapshot_download(
    repo_id="lj1995/GPT-SoVITS",
    revision="336b2ec4e8d4ac74740798dd40af44e74659ecaf",
    local_dir=out,
    allow_patterns=[
        "s1v3.ckpt",
        "sv/pretrained_eres2netv2w24s4ep4.ckpt",
        "v2Pro/s2Dv2ProPlus.pth",
        "v2Pro/s2Gv2ProPlus.pth",
        "chinese-hubert-base/**",
        "chinese-roberta-wwm-ext-large/**",
    ],
)
cache = out / ".cache"
if cache.exists():
    shutil.rmtree(cache)
print("[release-stage][hf] GPT-SoVITS pretrained snapshot PASS")
'@ | Set-Content -LiteralPath $downloadGptAssets -Encoding utf8NoBOM
& $stagePython $downloadGptAssets
if ($LASTEXITCODE -ne 0) { throw 'GPT-SoVITS Hugging Face snapshot staging failed.' }

Write-Host '[release-stage] Stage pinned GPT-SoVITS source revision'
$gptSourceZip = Join-Path $Temp 'gpt-sovits-source.zip'
Invoke-Download 'https://github.com/RVC-Boss/GPT-SoVITS/archive/d523079fc05d9a8028d6085bffe4a2757c32abb6.zip' $gptSourceZip
$gptExtract = Join-Path $Temp 'gpt-source-extract'
Expand-Archive -LiteralPath $gptSourceZip -DestinationPath $gptExtract -Force
$gptRoot = Get-ChildItem -LiteralPath $gptExtract -Directory | Select-Object -First 1
if (-not $gptRoot) { throw 'GPT-SoVITS source archive root missing.' }
Copy-Tree $gptRoot.FullName $VoiceSource
'd523079fc05d9a8028d6085bffe4a2757c32abb6' | Set-Content -LiteralPath (Join-Path $VoiceSource 'TRANSLATEIT_GPTSOVITS_REVISION.txt') -Encoding ascii
foreach ($relative in @('webui.py', 'api.py', 'api_v2.py', 'GPT_SoVITS\inference_webui.py', 'tools\asr', 'tools\uvr5', 'tools\subfix_webui.py')) {
    Remove-Item -LiteralPath (Join-Path $VoiceSource $relative) -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host '[release-stage] Stage pinned GPT-SoVITS pretrained assets'
$assetRoot = $env:GPT_ASSET_OUT
$pretrained = Join-Path $VoiceSource 'GPT_SoVITS\pretrained_models'
New-Item -ItemType Directory -Force -Path $pretrained, (Join-Path $pretrained 'sv'), (Join-Path $pretrained 'v2Pro') | Out-Null
Copy-Item -LiteralPath (Join-Path $assetRoot 's1v3.ckpt') -Destination (Join-Path $pretrained 's1v3.ckpt') -Force
Copy-Item -LiteralPath (Join-Path $assetRoot 'sv\pretrained_eres2netv2w24s4ep4.ckpt') -Destination (Join-Path $pretrained 'sv\pretrained_eres2netv2w24s4ep4.ckpt') -Force
Copy-Item -LiteralPath (Join-Path $assetRoot 'v2Pro\s2Dv2ProPlus.pth') -Destination (Join-Path $pretrained 'v2Pro\s2Dv2ProPlus.pth') -Force
Copy-Item -LiteralPath (Join-Path $assetRoot 'v2Pro\s2Gv2ProPlus.pth') -Destination (Join-Path $pretrained 'v2Pro\s2Gv2ProPlus.pth') -Force
Copy-Tree (Join-Path $assetRoot 'chinese-hubert-base') (Join-Path $pretrained 'chinese-hubert-base')
Copy-Tree (Join-Path $assetRoot 'chinese-roberta-wwm-ext-large') (Join-Path $pretrained 'chinese-roberta-wwm-ext-large')
Assert-Hash (Join-Path $pretrained 's1v3.ckpt') '87133414860ea14ff6620c483a3db5ed07b44be42e2c3fcdad65523a729a745a' 's1v3.ckpt'
Assert-Hash (Join-Path $pretrained 'sv\pretrained_eres2netv2w24s4ep4.ckpt') '4f5a0bf73c61eb41b174e1bb54e7ee3c83233892be8e0af1f187024e8e581a35' 'speaker model'
Assert-Hash (Join-Path $pretrained 'v2Pro\s2Dv2ProPlus.pth') '635cd84bf6f7f9b8d41c88c7106f81d782c794c61f931845214ea037b0c5bef2' 's2Dv2ProPlus.pth'
Assert-Hash (Join-Path $pretrained 'v2Pro\s2Gv2ProPlus.pth') 'd42a22bbbf65fb2bbdd45ad6a66841156977db45c7aabe0a6992ff378d9c7d3b' 's2Gv2ProPlus.pth'

Write-Host '[release-stage] Stage exact NLTK data snapshot'
$nltkRevision = '550b6625bcef1f2abff2ff770a5a0d272c9c6b2a'
$nltkRoot = Join-Path $VoiceSource 'nltk_data'
New-Item -ItemType Directory -Force -Path (Join-Path $nltkRoot 'corpora'), (Join-Path $nltkRoot 'taggers') | Out-Null
$nltkPackages = @(
    @('corpora', 'cmudict', 'd07cca47fd72ad32ea9d8ad1219f85301eeaf4568f8b6b73747506a71fb5afd6'),
    @('taggers', 'averaged_perceptron_tagger', 'e1f13cf2532daadfd6f3bc481a49859f0b8ea6432ccdcd83e6a49a5f19008de9'),
    @('taggers', 'averaged_perceptron_tagger_eng', '6025f530624335c67d6547d44757b357b4e79bae030a0383e9887a92c1718f0b')
)
$nltkRecords = @()
foreach ($item in $nltkPackages) {
    $category = $item[0]; $name = $item[1]; $expected = $item[2]
    $zip = Join-Path $Temp "$name.zip"
    $url = "https://raw.githubusercontent.com/nltk/nltk_data/$nltkRevision/packages/$category/$name.zip"
    Invoke-Download $url $zip $expected
    Assert-Hash $zip $expected "NLTK $category/$name.zip"
    $nltkRecords += "$category/$name.zip sha256=$expected"
    Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $nltkRoot $category) -Force
}
@('source_kind=nltk_data', 'repository=nltk/nltk_data', "revision=$nltkRevision") + $nltkRecords | Set-Content -LiteralPath (Join-Path $VoiceSource 'NLTK_DATA_SOURCE.txt') -Encoding ascii

Write-Host '[release-stage] Stage pinned FFmpeg n8.1 LGPL executable from BtbN'
# Month-end BtbN release pin; the GitHub release/asset API supplies this
# identity and archive SHA. The staged archive/executable/license still must
# pass local integrity and LGPL build-option checks before packaging.
$ffmpegReleaseTag = 'autobuild-2026-09-30-13-08'
$ffmpegReleaseId = '400057568'
$ffmpegReleaseName = 'Auto-Build 2026-09-30 13:08'
$ffmpegReleasePublishedAt = '2026-09-30T13:09:24Z'
$ffmpegAssetId = '600950502'
$ffmpegAssetName = 'ffmpeg-n8.1.3-9-g29e619e767-win64-lgpl-8.1.zip'
$ffmpegArchiveSha = '4a7642b2264c03e8a0ce8a3825b933ee5580656f45695a086fe7e294045ffc0a'
$ffmpegDownloadUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/$ffmpegReleaseTag/$ffmpegAssetName"
$ffmpegArchive = Join-Path $Temp 'ffmpeg.zip'
Invoke-Download $ffmpegDownloadUrl $ffmpegArchive $ffmpegArchiveSha
Assert-Hash $ffmpegArchive $ffmpegArchiveSha 'FFmpeg pinned archive'
$ffmpegExtract = Join-Path $Temp 'ffmpeg-extract'
Expand-Archive -LiteralPath $ffmpegArchive -DestinationPath $ffmpegExtract -Force
$ffmpegExe = Get-ChildItem -LiteralPath $ffmpegExtract -Recurse -File -Filter 'ffmpeg.exe' | Select-Object -First 1
$ffmpegLicense = Get-ChildItem -LiteralPath $ffmpegExtract -Recurse -File -Filter 'LICENSE.txt' | Select-Object -First 1
if (-not $ffmpegExe -or -not $ffmpegLicense) { throw 'BtbN FFmpeg archive layout incomplete.' }
$ffmpegVersionOutput = @(& $ffmpegExe.FullName -hide_banner -version 2>&1)
if ($LASTEXITCODE -ne 0 -or $ffmpegVersionOutput.Count -eq 0) { throw 'Staged FFmpeg executable did not report its version.' }
$ffmpegVersionLine = ([string]$ffmpegVersionOutput[0]).Trim()
if ($ffmpegVersionLine -notmatch '^ffmpeg version n?8\.1(?:\.|\s|-)') { throw "Unexpected FFmpeg stable line: $ffmpegVersionLine" }
$ffmpegBuildOutput = @(& $ffmpegExe.FullName -hide_banner -buildconf 2>&1)
if ($LASTEXITCODE -ne 0 -or $ffmpegBuildOutput.Count -eq 0) { throw 'Staged FFmpeg executable did not report its build configuration.' }
$ffmpegBuildText = [string]::Join("`n", $ffmpegBuildOutput)
if ($ffmpegBuildText -match '(?m)--enable-(?:gpl|nonfree)(?:\s|$)') { throw 'FFmpeg asset unexpectedly enables GPL or nonfree build options.' }
if ($ffmpegBuildText -notmatch '(?m)--enable-version3(?:\s|$)') { throw 'FFmpeg LGPL asset does not expose the expected version3 build contract.' }
$ffmpegLicenseText = Get-Content -LiteralPath $ffmpegLicense.FullName -Raw
if ($ffmpegLicenseText -notmatch 'GNU LESSER GENERAL PUBLIC LICENSE' -or $ffmpegLicenseText -notmatch 'Version 3, 29 June 2007') { throw 'FFmpeg archive license material does not contain LGPL v3 terms.' }
$ffmpegExeSha = Get-Sha256 $ffmpegExe.FullName
$ffmpegLicenseSha = Get-Sha256 $ffmpegLicense.FullName
Write-Host "[release-stage][hash] ffmpeg.exe=$ffmpegExeSha"
Write-Host "[release-stage][hash] FFmpeg LICENSE.txt=$ffmpegLicenseSha"
Copy-Item -LiteralPath $ffmpegExe.FullName -Destination (Join-Path $VoiceSource 'ffmpeg.exe') -Force
Copy-Item -LiteralPath $ffmpegLicense.FullName -Destination (Join-Path $VoiceSource 'FFMPEG_LICENSE.txt') -Force
@"
source_kind=ffmpeg
binary_builder=BtbN/FFmpeg-Builds
builder_release_api=https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/$ffmpegReleaseTag
builder_release_id=$ffmpegReleaseId
builder_release_tag=$ffmpegReleaseTag
builder_release_name=$ffmpegReleaseName
builder_release_published_at=$ffmpegReleasePublishedAt
asset_id=$ffmpegAssetId
archive=$ffmpegAssetName
archive_sha256=$ffmpegArchiveSha
download_url=$ffmpegDownloadUrl
ffmpeg_version_line=$ffmpegVersionLine
ffmpeg_exe_sha256=$ffmpegExeSha
ffmpeg_license_sha256=$ffmpegLicenseSha
license_profile=LGPL-3.0-or-later
build_profile=win64-lgpl-static
integrity_source=pinned_release_asset_sha256
"@ | Set-Content -LiteralPath (Join-Path $VoiceSource 'FFMPEG_SOURCE.txt') -Encoding ascii

Write-Host '[release-stage] Stage exact official VB-CABLE Pack45'
$vbArchive = Join-Path $Temp 'VBCABLE_Driver_Pack45.zip'
Invoke-Download 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip' $vbArchive 'b950e39f01af1d04ea623c8f6d8eb9b6ea5c477c637295fabf20631c85116bfb'
Assert-Hash $vbArchive 'b950e39f01af1d04ea623c8f6d8eb9b6ea5c477c637295fabf20631c85116bfb' 'VB-CABLE Pack45 archive'
Remove-Item -LiteralPath $VbPackage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $VbPackage | Out-Null
Expand-Archive -LiteralPath $vbArchive -DestinationPath $VbPackage -Force
if ((Get-ChildItem -LiteralPath $VbPackage -File).Count -ne 31) { throw 'VB-CABLE Pack45 extraction count mismatch.' }
foreach ($exeName in @('VBCABLE_Setup.exe', 'VBCABLE_Setup_x64.exe')) {
    $sig = Get-AuthenticodeSignature -LiteralPath (Join-Path $VbPackage $exeName)
    if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'BUREL VINCENT') { throw "VB-CABLE Authenticode invalid: $exeName" }
}

Write-Host '[release-stage] Remove transient download metadata from staged runtime'
Get-ChildItem -LiteralPath $Assets -Directory -Recurse -Force -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq '.cache' } | Sort-Object FullName -Descending | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

Write-Host '[release-stage] Static VoiceLab source-asset preflight through private Python'
$env:SOURCE_ROOT = $VoiceSource
& $privatePython -s -c "from pathlib import Path; import os; import voice_lab_gpt_sovits_build as v; a=v.source_assets(Path(os.environ['SOURCE_ROOT'])); print('[release-stage][voice] source assets PASS', sorted(a.keys()))"
if ($LASTEXITCODE -ne 0) { throw 'VoiceLab staged source asset validation failed.' }

Write-Host '[release-stage] Controlled input sizes'
foreach ($pair in @(
    @('PythonRuntime', $PythonRoot),
    @('ASR', $AsrRoot),
    @('Translation', $TranslationRoot),
    @('Voice', $VoiceSource),
    @('VB-CABLE', $VbPackage)
)) {
    $bytes = (Get-ChildItem -LiteralPath $pair[1] -File -Recurse | Measure-Object -Property Length -Sum).Sum
    Write-Host "[release-stage][size] $($pair[0]) bytes=$bytes"
}

Import-Module (Join-Path $PSScriptRoot 'ReleaseStaging.psm1') -Force
Publish-TranslateITReleaseStage -RepositoryRoot $RepoRoot -StageRoot $PayloadRoot
try {
    Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction Stop
} catch {
    Write-Warning "Verified release was committed, but disposable staging cleanup requires attention: $Temp"
}
Write-Host '[release-stage] controlled release input staging -> PASS'

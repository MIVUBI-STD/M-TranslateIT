# Deterministic local-directory fixture for release promotion and rollback.
# Uses disposable CI folders and no network, real models, UserData cleanup or system drivers.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'ReleaseStaging.psm1') -Force

$relativeRoots = @(
    "EngineData/Backend/LocalWorker/PythonRuntime",
    "EngineData/Backend/RuntimeAssets/ASR/ModelData",
    "EngineData/Backend/RuntimeAssets/Translation/ModelData",
    "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS/Source",
    "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package"
)
$base = Join-Path $env:RUNNER_TEMP ("translateit-release-staging-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $base | Out-Null
function New-Fixture([string]$name) {
    $root = Join-Path $base $name
    $stage = Join-Path $root ".workspace/staging/release-test/payload-tree"
    $lock = Join-Path $root ".workspace/staging/release-active"
    New-Item -ItemType Directory -Path $lock -Force | Out-Null
    $userSaved = Join-Path $root "UserData/SavedProject/VoiceLab"
    New-Item -ItemType Directory -Path $userSaved -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $userSaved "private.txt") -Value "preserve" -Encoding ascii
    foreach ($relative in $relativeRoots) {
        $target = Join-Path $stage $relative
        New-Item -ItemType Directory -Path $target -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $target "new.txt") -Value "new" -Encoding ascii
    }
    return [pscustomobject]@{ root=$root; stage=$stage; lock=$lock; saved=$userSaved }
}
function Assert-PrivateData([object]$fixture) {
    if ((Get-Content -LiteralPath (Join-Path $fixture.saved "private.txt") -Raw).Trim() -ne "preserve") {
        throw "Release promotion altered user-owned data."
    }
}
try {
    $success = New-Fixture "success"
    $previous = Join-Path $success.root $relativeRoots[0]
    New-Item -ItemType Directory -Path $previous -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $previous "old.txt") -Value "old" -Encoding ascii
    Publish-TranslateITReleaseStage -RepositoryRoot $success.root -StageRoot $success.stage
    if (-not (Test-Path -LiteralPath (Join-Path $previous "new.txt")) -or
        (Test-Path -LiteralPath (Join-Path $previous "old.txt"))) {
        throw "Release promotion did not replace the old Python runtime."
    }
    if (Test-Path -LiteralPath (Join-Path $success.root ".workspace/staging/release-rollback")) {
        throw "Successful staging left an unnecessary recovery backup."
    }
    Assert-PrivateData $success

    $blocked = New-Fixture "blocked"
    $old = Join-Path $blocked.root $relativeRoots[0]
    New-Item -ItemType Directory -Path $old -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $old "old.txt") -Value "old" -Encoding ascii
    $blockingParent = Join-Path $blocked.root "EngineData/Backend/RuntimeAssets/ASR"
    New-Item -ItemType Directory -Path (Split-Path -Parent $blockingParent) -Force | Out-Null
    Set-Content -LiteralPath $blockingParent -Value "conflicting file" -Encoding ascii

    $threw = $false
    try { Publish-TranslateITReleaseStage -RepositoryRoot $blocked.root -StageRoot $blocked.stage }
    catch { $threw = $true }
    if (-not $threw) { throw "Expected promotion to fail at a conflicting target parent." }
    if (-not (Test-Path -LiteralPath (Join-Path $old "old.txt")) -or
        (Test-Path -LiteralPath (Join-Path $old "new.txt"))) {
        throw "Failed promotion did not restore existing Python runtime."
    }
    Assert-PrivateData $blocked

    $unfinished = New-Fixture "unfinished"
    New-Item -ItemType Directory -Force -Path (Join-Path $unfinished.root ".workspace/staging/release-rollback") | Out-Null
    $refused = $false
    try { Assert-TranslateITReleaseStagingAvailable -RepositoryRoot $unfinished.root }
    catch { $refused = $true }
    if (-not $refused) { throw "An abandoned promotion backup must block another staging run." }
    Assert-PrivateData $unfinished
    Write-Host "Release staging promotion, rollback, fail-closed recovery and UserData isolation: PASS"
} finally {
    Remove-Item -LiteralPath $base -Recurse -Force -ErrorAction Stop
}

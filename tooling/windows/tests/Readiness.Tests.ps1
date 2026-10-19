# Windows-only: read-only inventory, truthful claims and explicit ignored report.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '..\modules\Readiness.psm1') -Force

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$temp = Join-Path $env:RUNNER_TEMP ("translateit-readiness-test-"+[guid]::NewGuid().ToString("N"))
$config = Join-Path $temp "tooling/windows/config"
$protected = Join-Path $temp "UserData/SavedProject/VoiceLab"
New-Item -ItemType Directory -Path $config,$protected -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repo "tooling/windows/config/requirements.json") -Destination $config -Force
$sentinel = Join-Path $protected "private.txt"
Set-Content -LiteralPath $sentinel -Value "do not change" -Encoding Ascii
try {
    $first = Get-TranslateITReadiness -RepositoryRoot $temp
    if ($first.schema -ne "translateit.windows.workspace_readiness.v1") { throw "Readiness schema drifted." }
    if (Test-Path -LiteralPath (Join-Path $temp ".workspace")) {
        throw "A read-only inventory created a workspace."
    }
    if ($first.proof.gpu_inference -ne "NOT_EXECUTED" -or
        $first.proof.microphone_and_virtual_audio -ne "NOT_EXECUTED" -or
        $first.proof.installed_product -ne "NOT_EXECUTED") {
        throw "Readiness claims unexecuted native tests passed."
    }
    if (@($first.lockfiles | Where-Object state -eq "MISSING").Count -ne 3) {
        throw "A bare fixture incorrectly claims lockfiles present."
    }
    $fakeAsr = Join-Path $temp "EngineData/Backend/RuntimeAssets/ASR/ModelData/faster-whisper-large-v3-turbo"
    New-Item -ItemType Directory -Path $fakeAsr -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $fakeAsr ".translateit_model_revision") -Value "untrusted"
    $second = Get-TranslateITReadiness -RepositoryRoot $temp
    if (@($second.runtime_asset_presence | Where-Object state -eq "PRESENT_UNVERIFIED").Count -ne 1) {
        throw "Model marker presence must not become verified model acceptance."
    }
    if (Test-Path -LiteralPath (Join-Path $temp ".workspace")) {
        throw "Repeated status must remain non-writing."
    }
    $reports = Join-Path $temp ".workspace/reports"
    New-Item -ItemType Directory -Path $reports -Force | Out-Null
    $null = Write-TranslateITReadinessReport -Readiness $second -WorkspaceRoot (Join-Path $temp ".workspace")
    $reportPath = Join-Path $reports "readiness.json"
    $saved = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    if ($saved.proof.gpu_inference -ne "NOT_EXECUTED" -or
        $saved.runtime_asset_presence[0].state -ne "PRESENT_UNVERIFIED") {
        throw "Saved readiness report elevated incomplete evidence."
    }
    if ((Get-Content -LiteralPath $sentinel -Raw).Trim() -ne "do not change") {
        throw "Readiness operation mutated private user data."
    }
    Write-Host "Readiness non-writing inventory, explicit report and proof boundary: PASS"
} finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction Stop
}

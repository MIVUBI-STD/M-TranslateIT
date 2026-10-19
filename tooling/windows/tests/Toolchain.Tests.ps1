# Offline Windows fixture for complete local tool groups and strict selection.
# Fake executable placeholders are never executed. UserData is never mutated.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '..\modules\Toolchain.psm1') -Force

$folder = Join-Path $env:RUNNER_TEMP ("translateit-toolchain-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $folder | Out-Null
$priorPath = $env:PATH
$priorCargo = [Environment]::GetEnvironmentVariable('CARGO_HOME','Process')
$priorRust = [Environment]::GetEnvironmentVariable('RUSTUP_HOME','Process')
try {
    $protected = Join-Path $folder 'UserData\SavedProject'
    New-Item -ItemType Directory -Path $protected -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $protected 'keep.txt') -Value 'private' -Encoding Ascii

    $initial = @(Get-TranslateITWorkspaceToolSelection -RepositoryRoot $folder)
    if ($initial.Count -ne 4 -or @($initial | Where-Object state -ne 'ABSENT').Count -ne 0) {
        throw 'An empty workspace should not claim a portable toolchain.'
    }
    if (Test-Path -LiteralPath (Join-Path $folder '.workspace')) {
        throw 'Toolchain observation created generated directories.'
    }

    $nodeDir = Join-Path $folder '.workspace\tools\node'
    New-Item -ItemType Directory -Path $nodeDir -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $nodeDir 'node.exe') | Out-Null
    $partial = Get-TranslateITPreferredTool -RepositoryRoot $folder -Name node
    if ($partial.state -ne 'INCOMPLETE_LOCAL') {
        throw 'Partial local Node must not fall back to system Node.'
    }
    $rejected = $false
    try { Use-TranslateITWorkspaceToolchain -RepositoryRoot $folder }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'Partial tool group was not rejected.' }
    if ($env:PATH -ne $priorPath) { throw 'Rejected tool selection modified PATH.' }

    New-Item -ItemType File -Path (Join-Path $nodeDir 'npm.cmd') | Out-Null
    $localNode = Get-TranslateITPreferredTool -RepositoryRoot $folder -Name node
    $localNpm = Get-TranslateITPreferredTool -RepositoryRoot $folder -Name npm
    if ($localNode.origin -ne 'workspace' -or $localNpm.origin -ne 'workspace' -or
        $localNode.state -ne 'SELECTED' -or $localNpm.state -ne 'SELECTED') {
        throw 'Complete Node/npm group must be selected together.'
    }
    $rustDir = Join-Path $folder '.workspace\tools\cargo\bin'
    New-Item -ItemType Directory -Path $rustDir -Force | Out-Null
    foreach ($exe in @('rustc.exe','cargo.exe','rustup.exe')) {
        New-Item -ItemType File -Path (Join-Path $rustDir $exe) | Out-Null
    }
    $rust = Get-TranslateITPreferredTool -RepositoryRoot $folder -Name rustc
    if ($rust.state -ne 'INCOMPLETE_LOCAL') { throw 'Rust proxy without toolchain must be rejected.' }
    $rejected = $false
    try { Use-TranslateITWorkspaceToolchain -RepositoryRoot $folder }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'Rust without a toolchain was not blocked.' }

    New-Item -ItemType Directory -Path (Join-Path $folder '.workspace\tools\rustup\toolchains') -Force | Out-Null
    Use-TranslateITWorkspaceToolchain -RepositoryRoot $folder
    if (-not $env:PATH.StartsWith($nodeDir + ';', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'A complete workspace Node group was not first in child-process PATH.'
    }
    if ($env:CARGO_HOME -ne (Join-Path $folder '.workspace\tools\cargo') -or
        $env:RUSTUP_HOME -ne (Join-Path $folder '.workspace\tools\rustup')) {
        throw 'Cargo/Rustup homes escaped the selected workspace.'
    }
    $again = $env:PATH
    Use-TranslateITWorkspaceToolchain -RepositoryRoot $folder
    if ($env:PATH -ne $again) {
        throw 'Repeated toolchain selection duplicated or re-ordered PATH.'
    }
    if ((Get-Content -LiteralPath (Join-Path $protected 'keep.txt') -Raw).Trim() -ne 'private') {
        throw 'Toolchain management modified user data.'
    }
    Write-Host 'Portable tool bundle completeness, precedence, idempotence and data isolation: PASS'
} finally {
    $env:PATH = $priorPath
    [Environment]::SetEnvironmentVariable('CARGO_HOME',$priorCargo,'Process')
    [Environment]::SetEnvironmentVariable('RUSTUP_HOME',$priorRust,'Process')
    Remove-Item -LiteralPath $folder -Recurse -Force -ErrorAction Stop
}

# Windows-only regression for the ignored development workspace.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Import-Module (Join-Path $PSScriptRoot "..\modules\Workspace.psm1") -Force
$testRoot = Join-Path $env:RUNNER_TEMP ("translateit-workspace-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$keys = @("TRANSLATEIT_WORKSPACE_ROOT","TEMP","TMP","npm_config_cache","UV_CACHE_DIR",
    "PIP_CACHE_DIR","HF_HOME","HF_HUB_CACHE","HF_XET_CACHE",
    "UV_PYTHON_INSTALL_DIR","UV_TOOL_DIR","UV_TOOL_BIN_DIR")
$previous = @{}
foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key,"Process") }

try {
    $expected = Join-Path $testRoot ".workspace"
    $root = Set-TranslateITWorkspaceEnvironment -RepositoryRoot $testRoot
    if ($root -ne $expected) { throw "Workspace did not resolve to the repository-local root." }
    foreach ($key in $keys) {
        $actual = [Environment]::GetEnvironmentVariable($key,"Process")
        if (-not $actual -or -not ($actual -eq $expected -or $actual.StartsWith($expected + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase))) {
            throw "Workspace path escaped the owned root: $key"
        }
    }
    if (Test-Path -LiteralPath (Join-Path $testRoot "UserData")) {
        throw "Workspace setup must not create user data."
    }
    $again = Set-TranslateITWorkspaceEnvironment -RepositoryRoot $testRoot
    if ($again -ne $root) { throw "Workspace setup is not idempotent." }

    $collisionRoot = Join-Path $env:RUNNER_TEMP ("translateit-workspace-collision-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $collisionRoot | Out-Null
    try {
        Set-Content -LiteralPath (Join-Path $collisionRoot ".workspace") -Value "untrusted" -Encoding Ascii
        $blocked = $false
        try { Set-TranslateITWorkspaceEnvironment -RepositoryRoot $collisionRoot | Out-Null }
        catch { $blocked = $true }
        if (-not $blocked) { throw "Workspace root file collision must be rejected." }
    } finally {
        Remove-Item -LiteralPath $collisionRoot -Recurse -Force
    }
    # Release staging may stage a separate runtime, but must not mutate the
    # developer's Python environment or install packages in system Python.
    $repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
    $releaseStage = Get-Content -LiteralPath (Join-Path $repo "EngineData\Frontend\RustApp\scripts\stage_release_inputs_impl.ps1") -Raw
    foreach ($marker in @("UV_PROJECT_ENVIRONMENT", "worker-release-venv", ".workspace\cache\uv", ".workspace\cache\huggingface")) {
        if (-not $releaseStage.Contains($marker)) { throw "Missing isolated staging marker: $marker" }
    }
    if ($releaseStage.Contains("-m pip install") -or $releaseStage.Contains('Join-Path $Worker ''.venv')) {
        throw "Release staging must not install system packages or consume the developer .venv."
    }
    Write-Host "Windows workspace isolation and idempotence: PASS"
} finally {
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key,$previous[$key],"Process") }
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}

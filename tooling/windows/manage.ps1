param(
    [Parameter(Position = 0)]
    [ValidateSet("help", "status", "prepare", "verify", "build", "run", "test", "package")]
    [string]$Command = "help",
    [Parameter(Position = 1)]
    [string]$QualityReadinessEvidence
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Frontend = Join-Path $Root "EngineData\Frontend\RustApp"
$Worker = Join-Path $Root "EngineData\Backend\LocalWorker\WorkerRuntime"
Import-Module (Join-Path $PSScriptRoot "modules\Workspace.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "modules\Readiness.psm1") -Force

function Invoke-Step {
    param([string]$Label, [scriptblock]$Body)
    Write-Host ""
    Write-Host "==> $Label"
    & $Body
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

function Require-Command {
    param([string]$Name)
    $resolved = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $resolved) { throw "Required command not found: $Name" }
    return $resolved
}

function Show-Help {
    Write-Host "TranslateIT Windows development commands"
    Write-Host ""
    Write-Host "  manage.cmd status   Inspect toolchain without installing or repairing"
    Write-Host "  manage.cmd prepare    Prepare locked frontend and Python development dependencies"
    Write-Host "  manage.cmd verify    Verify repository, frontend and Python source tests"
    Write-Host "  manage.cmd build    Build frontend and compile-check Rust on one clean source SHA"
    Write-Host "  manage.cmd run      Start the local Tauri development app; no release signing"
    Write-Host "  manage.cmd test     Run frontend, Rust, and worker tests"
    Write-Host '  manage.cmd package "<quality-evidence.json>"  Build controlled signed release only'
}

function Assert-DevelopmentPrerequisites {
    Require-Command node | Out-Null
    Require-Command npm | Out-Null
    Require-Command rustc | Out-Null
    Require-Command cargo | Out-Null
    Require-Command python | Out-Null
    Require-Command uv | Out-Null

    $nodeVersion = (& node --version).Trim().TrimStart("v")
    $pythonVersion = (& python --version 2>&1).ToString().Replace("Python ", "").Trim()
    $rustVersion = (& rustc --version).Trim()
    $uvVersion = (& uv --version).Trim()

    if (-not $nodeVersion.StartsWith("22.")) {
        throw "Node 22.x required by repository CI; found $nodeVersion"
    }
    if ($pythonVersion -ne "3.12.10") {
        throw "Python 3.12.10 required; found $pythonVersion"
    }

    $policyPath = Join-Path $PSScriptRoot "config\requirements.json"
    $policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
    $minimumRust = [Version]::Parse([string]$policy.rust.minimum)
    $minimumUv = [Version]::Parse([string]$policy.uv.minimum)
    $rustMatch = [regex]::Match($rustVersion, '^rustc\s+(\d+\.\d+\.\d+)(?:\s|$)')
    if (-not $rustMatch.Success) {
        throw "Could not read rustc semantic version: $rustVersion"
    }
    $installedRust = [Version]::Parse($rustMatch.Groups[1].Value)
    if ($installedRust -lt $minimumRust) {
        throw "Rust $minimumRust or newer required by the locked dependency baseline; found $rustVersion"
    }

    $uvMatch = [regex]::Match($uvVersion, '^uv\s+(\d+\.\d+\.\d+)(?:\s|$)')
    if (-not $uvMatch.Success) {
        throw "Could not read uv semantic version: $uvVersion"
    }
    $installedUv = [Version]::Parse($uvMatch.Groups[1].Value)
    if ($installedUv -lt $minimumUv) {
        throw "uv $minimumUv or newer required by the locked worker environment; found $uvVersion"
    }

    Write-Host "Node:   $nodeVersion"
    Write-Host "Python: $pythonVersion"
    Write-Host "Rust:   $rustVersion (minimum $minimumRust)"
    Write-Host "uv:     $uvVersion (minimum $minimumUv)"
    Write-Host "Prerequisites passed. No installation or repair was performed."
}

Set-Location $Root
# Generated caches are scoped to this process and child tools, never Windows-wide.
if ($Command -notin @("help", "status")) {
    Set-TranslateITWorkspaceEnvironment -RepositoryRoot $Root
}

switch ($Command) {
    "help" { Show-Help }
    "status" { Show-TranslateITReadiness -Readiness (Get-TranslateITReadiness -RepositoryRoot $Root) }
    "prepare" {
        Assert-DevelopmentPrerequisites
        Invoke-Step "Install locked frontend dependencies" { Set-Location $Frontend; npm ci }
        Invoke-Step "Sync locked worker development dependencies" { Set-Location $Worker; uv sync --dev --frozen }
    }
    "verify" {
        Invoke-Step "Repository contract" { Set-Location $Root; python tools\verify_repository.py }
        Invoke-Step "Frontend quick validation" { Set-Location $Frontend; npm run validate:quick }
        Invoke-Step "Worker compile/static/tests" {
            Set-Location $Worker
            uv run --frozen --no-sync python -m compileall -q .
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            uv run --frozen --no-sync ruff check .
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            uv run --frozen --no-sync pytest -q
        }
        if ($LASTEXITCODE -ne 0) { throw "Worker validation was incomplete." }
        $readiness = Get-TranslateITReadiness -RepositoryRoot $Root
        $null = Write-TranslateITReadinessReport -Readiness $readiness -WorkspaceRoot (Join-Path $Root ".workspace")
    }
    "build" {
        Invoke-Step "Pinned frontend and Rust compile check" {
            Set-Location $Frontend
            npm run check:tauri-rust-local
        }
    }
    "run" {
        Invoke-Step "Local Tauri development app" {
            Set-Location $Frontend
            npm run dev:app
        }
    }
    "test" {
        Invoke-Step "Frontend runtime tests" { Set-Location $Frontend; npm run test:frontend-runtime }
        Invoke-Step "Rust unit tests" { Set-Location (Join-Path $Frontend "src-tauri"); cargo test --locked }
        Invoke-Step "Worker tests" { Set-Location $Worker; uv run --frozen --no-sync pytest -q }
    }
    "package" {
        if ([string]::IsNullOrWhiteSpace($QualityReadinessEvidence)) {
            throw 'Signed release requires an explicit Quality Readiness report. Usage: manage.cmd package "<quality-evidence.json>"'
        }
        if (-not (Test-Path -LiteralPath $QualityReadinessEvidence -PathType Leaf)) {
            throw "Quality Readiness evidence file was not found: $QualityReadinessEvidence"
        }
        $EvidenceFile = (Resolve-Path -LiteralPath $QualityReadinessEvidence).Path
        Invoke-Step "Existing controlled signed TranslateIT release pipeline" {
            Set-Location $Frontend
            powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\build_release.ps1" -QualityReadinessEvidence $EvidenceFile
        }
    }
}

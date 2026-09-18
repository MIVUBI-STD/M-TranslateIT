param(
    [Parameter(Position = 0)]
    [ValidateSet("help", "doctor", "setup", "check", "build", "test", "package")]
    [string]$Command = "help"
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Frontend = Join-Path $Root "EngineData\Frontend\RustApp"
$Worker = Join-Path $Root "EngineData\Backend\LocalWorker\WorkerRuntime"

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
    Write-Host "TranslateIT developer entrypoint"
    Write-Host ""
    Write-Host "  DEV.cmd doctor   Check required toolchain; never installs or repairs"
    Write-Host "  DEV.cmd setup    Install locked frontend + worker development dependencies"
    Write-Host "  DEV.cmd check    Run repository + quick frontend + worker source checks"
    Write-Host "  DEV.cmd build    Build frontend and compile-check Rust"
    Write-Host "  DEV.cmd test     Run frontend, Rust, and worker tests"
    Write-Host "  DEV.cmd package  Delegate to the existing release owner"
}

function Invoke-Doctor {
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

    Write-Host "Node:   $nodeVersion"
    Write-Host "Python: $pythonVersion"
    Write-Host "Rust:   $rustVersion (Cargo.toml minimum 1.77)"
    Write-Host "uv:     $uvVersion (pyproject minimum 0.12.0)"
    Write-Host "Doctor passed. No installation or repair was performed."
}

Set-Location $Root

switch ($Command) {
    "help" { Show-Help }
    "doctor" { Invoke-Doctor }
    "setup" {
        Invoke-Doctor
        Invoke-Step "Install locked frontend dependencies" { Set-Location $Frontend; npm ci }
        Invoke-Step "Sync locked worker development dependencies" { Set-Location $Worker; uv sync --dev --frozen }
    }
    "check" {
        Invoke-Step "Repository contract" { Set-Location $Root; python tools\verify_repository.py }
        Invoke-Step "Frontend quick validation" { Set-Location $Frontend; npm run validate:quick }
        Invoke-Step "Worker compile/static/tests" {
            Set-Location $Worker
            uv run python -m compileall -q .
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            uv run ruff check .
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            uv run pytest -q
        }
    }
    "build" {
        Invoke-Step "Frontend production build" { Set-Location $Frontend; npm run build:frontend }
        Invoke-Step "Rust compile check" { Set-Location (Join-Path $Frontend "src-tauri"); cargo check --locked }
    }
    "test" {
        Invoke-Step "Frontend runtime tests" { Set-Location $Frontend; npm run test:frontend-runtime }
        Invoke-Step "Rust unit tests" { Set-Location (Join-Path $Frontend "src-tauri"); cargo test --locked }
        Invoke-Step "Worker tests" { Set-Location $Worker; uv run pytest -q }
    }
    "package" {
        Invoke-Step "Existing TranslateIT release pipeline" {
            Set-Location $Frontend
            powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\scripts\build_release.ps1"
        }
    }
}

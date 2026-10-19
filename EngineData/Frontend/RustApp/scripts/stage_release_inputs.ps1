$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Implementation = Join-Path $PSScriptRoot 'stage_release_inputs_impl.ps1'
if (-not (Test-Path -LiteralPath $Implementation -PathType Leaf)) {
    throw "Release staging implementation missing: $Implementation"
}
Import-Module (Join-Path $PSScriptRoot 'ReleaseStaging.psm1') -Force
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
Assert-TranslateITReleaseStagingAvailable -RepositoryRoot $RepoRoot
$stagingBase = Join-Path $RepoRoot '.workspace\staging'
New-Item -ItemType Directory -Force -Path $stagingBase | Out-Null
$lock = Join-Path $stagingBase 'release-active'
New-Item -ItemType Directory -Path $lock -ErrorAction Stop | Out-Null
$previousStageRoot = [Environment]::GetEnvironmentVariable('TRANSLATEIT_RELEASE_STAGE_ROOT', 'Process')
$env:TRANSLATEIT_RELEASE_STAGE_ROOT = Join-Path $stagingBase ('release-' + [guid]::NewGuid().ToString('N'))
# Staging contract remains delegated to stage_release_inputs_impl.ps1:
# prepare_model_assets.py stages milmmt-46-1b-v1.0 at
# revision 4fc480b6c58dec29c159dcdf9fde0f6d5c354995 plus the required ASR/voice/audio inputs.
# PowerShell 7+ supports `utf8NoBOM` directly. Windows PowerShell 5.1 does not.
# For 5.1 only, execute a temporary copy in the same directory so `$PSScriptRoot`
# and all repository-relative paths remain unchanged. The generated Python helper
# may contain a UTF-8 BOM under 5.1; Python 3.12 accepts UTF-8 BOM source files.
try {
    if ($PSVersionTable.PSVersion.Major -ge 6) {
        & $Implementation
    } else {
        $Source = Get-Content -LiteralPath $Implementation -Raw
        $Unsupported = '-Encoding utf8NoBOM'
        $Occurrences = ([regex]::Matches($Source, [regex]::Escape($Unsupported))).Count
        if ($Occurrences -ne 1) {
            throw "Expected exactly one Windows PowerShell-incompatible encoding token, found $Occurrences."
        }
        $Patched = $Source.Replace($Unsupported, '-Encoding UTF8')
        # Preserve PSScriptRoot semantics for the compatibility script.
        $Temporary = Join-Path $PSScriptRoot '.stage_release_inputs.ps51.generated.ps1'
        $Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($Temporary, $Patched, $Utf8NoBom)
        try { & $Temporary } finally {
            Remove-Item -LiteralPath $Temporary -Force -ErrorAction SilentlyContinue
        }
    }
} finally {
    [Environment]::SetEnvironmentVariable('TRANSLATEIT_RELEASE_STAGE_ROOT', $previousStageRoot, 'Process')
    Remove-Item -LiteralPath $lock -Force -Recurse -ErrorAction Stop
}


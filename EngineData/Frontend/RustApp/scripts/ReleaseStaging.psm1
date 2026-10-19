# Controlled release input promotion; staged payload and rollback stay on one volume.
# Only these five canonical runtime subtrees can be replaced. UserData is forbidden.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:ReleasePayloadRoots = @(
    "EngineData/Backend/LocalWorker/PythonRuntime",
    "EngineData/Backend/RuntimeAssets/ASR/ModelData",
    "EngineData/Backend/RuntimeAssets/Translation/ModelData",
    "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS/Source",
    "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package"
)

function Assert-TranslateITReleaseStagingAvailable {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    $workspace = Join-Path $RepositoryRoot ".workspace/staging"
    $backup = Join-Path $workspace "release-rollback"
    $lock = Join-Path $workspace "release-active"
    foreach ($p in @($workspace, $backup, $lock)) {
        if (Test-Path -LiteralPath $p) {
            $item = Get-Item -LiteralPath $p -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Release staging refuses reparse paths: $p"
            }
        }
    }
    if (Test-Path -LiteralPath $backup) {
        throw "Previous release promotion backup remains; inspect and recover before retry: $backup"
    }
    if (Test-Path -LiteralPath $lock) {
        throw "Another release staging operation or interrupted run owns: $lock"
    }
}

function Publish-TranslateITReleaseStage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$StageRoot
    )

    $repo = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $stage = (Resolve-Path -LiteralPath $StageRoot).Path
    $privateStaging = Join-Path $repo ".workspace/staging"
    if (-not $stage.StartsWith($privateStaging + [IO.Path]::DirectorySeparatorChar,
            [StringComparison]::OrdinalIgnoreCase)) {
        throw "Untrusted release stage: $stage"
    }
    $backup = Join-Path $privateStaging "release-rollback"
    if (Test-Path -LiteralPath $backup) {
        throw "Unfinished release rollback state is present: $backup"
    }
    foreach ($relative in $script:ReleasePayloadRoots) {
        $from = Join-Path $stage $relative
        if (-not (Test-Path -LiteralPath $from -PathType Container)) {
            throw "Missing verified staging subtree: $relative"
        }
        $source = Get-Item -LiteralPath $from -Force
        if ($source.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Staged subtree must not be a reparse point: $relative"
        }
        $dest = Join-Path $repo $relative
        if (Test-Path -LiteralPath $dest) {
            $existing = Get-Item -LiteralPath $dest -Force
            if (-not $existing.PSIsContainer -or
                ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Runtime destination is unsafe: $relative"
            }
        }
    }

    # Creating this directory is the atomic promotion reservation. If power is
    # interrupted, preserve it as a recovery signal; never silently erase it.
    New-Item -ItemType Directory -Path $backup -ErrorAction Stop | Out-Null
    $moved = New-Object System.Collections.Generic.List[object]
    try {
        foreach ($relative in $script:ReleasePayloadRoots) {
            $from = Join-Path $stage $relative
            $to = Join-Path $repo $relative
            $saved = Join-Path $backup $relative
            $item = [pscustomobject]@{
                from = $from; to = $to; saved = $saved;
                oldMoved = $false; newMoved = $false
            }
            $moved.Add($item)
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $to),
                (Split-Path -Parent $saved) | Out-Null
            if (Test-Path -LiteralPath $to) {
                Move-Item -LiteralPath $to -Destination $saved -ErrorAction Stop
                $item.oldMoved = $true
            }
            Move-Item -LiteralPath $from -Destination $to -ErrorAction Stop
            $item.newMoved = $true
        }
        foreach ($relative in $script:ReleasePayloadRoots) {
            if (-not (Test-Path -LiteralPath (Join-Path $repo $relative) -PathType Container)) {
                throw "Promoted release subtree went missing: $relative"
            }
        }
    } catch {
        $original = $_
        $rollbackFailed = $false
        for ($i = $moved.Count - 1; $i -ge 0; $i--) {
            $item = $moved[$i]
            try {
                if ($item.newMoved -and (Test-Path -LiteralPath $item.to)) {
                    Remove-Item -LiteralPath $item.to -Recurse -Force -ErrorAction Stop
                }
                if ($item.oldMoved) {
                    if (-not (Test-Path -LiteralPath $item.saved)) {
                        throw "Original runtime backup is missing: $($item.saved)"
                    }
                    Move-Item -LiteralPath $item.saved -Destination $item.to -ErrorAction Stop
                }
            } catch {
                $rollbackFailed = $true
                Write-Error -Message "Release staging rollback needs manual recovery: $($_.Exception.Message)" -ErrorAction Continue
            }
        }
        if (-not $rollbackFailed) {
            Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction Stop
        }
        throw "Release promotion failed. Rolled back: $(-not $rollbackFailed). Original error: $original"
    }
    Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction Stop
    Write-Host "[release-stage] Verified runtime subtrees committed; previous versions retired."
}

Export-ModuleMember -Function Assert-TranslateITReleaseStagingAvailable, Publish-TranslateITReleaseStage

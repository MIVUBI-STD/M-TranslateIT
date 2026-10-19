# Read-only Windows workspace inventory. Never infer AI/device acceptance.
Set-StrictMode -Version Latest

function Get-TranslateITToolObservation {
    param([string]$Name, [string]$WorkspaceRoot)
    $found = Get-Command -Name $Name -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandType -eq 'Application' } | Select-Object -First 1
    if ($null -eq $found) {
        return [pscustomobject]@{ name=$Name; state='MISSING'; origin='none'; version=$null; path=$null }
    }
    $path = [IO.Path]::GetFullPath($found.Source)
    $prefix = $WorkspaceRoot.TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
    $origin = if ($path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { 'workspace' } else { 'external' }
    $version = $null
    try {
        $output = (& $path --version 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -eq 0 -and $output) { $version = ($output -split '[\r\n]+')[0] }
    } catch {}
    return [pscustomobject]@{
        name=$Name
        state= if ($version) { 'DETECTED' } else { 'UNVERIFIED' }
        origin=$origin
        version=$version
        path=$path
    }
}

function Get-TranslateITReadiness {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$RepositoryRoot)
    $root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $workspace = Join-Path $root '.workspace'
    $policyPath = Join-Path $root 'tooling/windows/config/requirements.json'
    if (-not (Test-Path -LiteralPath $policyPath -PathType Leaf)) {
        throw 'Canonical Windows development requirements are missing.'
    }
    $policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
    $tools = @('node','npm','python','uv','rustc','cargo','git') | ForEach-Object {
        Get-TranslateITToolObservation -Name $_ -WorkspaceRoot $workspace
    }
    $locks = @(
        'EngineData/Frontend/RustApp/package-lock.json',
        'EngineData/Frontend/RustApp/src-tauri/Cargo.lock',
        'EngineData/Backend/LocalWorker/WorkerRuntime/uv.lock'
    ) | ForEach-Object {
        [pscustomobject]@{
            path=$_; state=if (Test-Path -LiteralPath (Join-Path $root $_) -PathType Leaf) { 'PRESENT' } else { 'MISSING' }
        }
    }
    $assets = @(
        'EngineData/Backend/RuntimeAssets/ASR/ModelData/faster-whisper-large-v3-turbo/.translateit_model_revision',
        'EngineData/Backend/RuntimeAssets/Translation/ModelData/xiaomi-research--MiLMMT-46-1B-v1.0/.translateit_model_revision',
        'EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/MaleVoice/reference.wav',
        'EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/FemaleVoice/reference.wav',
        'EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS/Source/TRANSLATEIT_GPTSOVITS_REVISION.txt'
    ) | ForEach-Object {
        [pscustomobject]@{
            path=$_; state=if (Test-Path -LiteralPath (Join-Path $root $_) -PathType Leaf) { 'PRESENT_UNVERIFIED' } else { 'MISSING' }
        }
    }
    $sha = $null
    $git = @($tools | Where-Object { $_.name -eq 'git' -and $_.state -eq 'DETECTED' })
    # A disposable/non-Git folder cannot have a verifiable source revision.
    # Do not execute git -C there and leak a failed native process exit code.
    if ($git.Count -eq 1 -and (Test-Path -LiteralPath (Join-Path $root '.git'))) {
        try {
            $revision = (& $git[0].path -C $root rev-parse HEAD 2>$null | Out-String).Trim()
            if ($revision -cmatch '^[0-9a-f]{40}$') { $sha=$revision }
        } catch {
            $sha = $null
        } finally {
            $global:LASTEXITCODE = 0
        }
    }
    $managed = @($tools | Where-Object { $_.state -ne 'DETECTED' -or $_.origin -ne 'workspace' }).Count -eq 0
    return [pscustomobject]@{
        schema='translateit.windows.workspace_readiness.v1'
        revision=$sha
        workspace=[pscustomobject]@{
            root=$workspace
            exists=(Test-Path -LiteralPath $workspace -PathType Container)
            toolchain_managed=$managed
        }
        declared_requirements=[pscustomobject]@{
            node_major=$policy.node.major
            python_version=$policy.python.version
            rust_minimum=$policy.rust.minimum
            uv_minimum=$policy.uv.minimum
        }
        tools=@($tools)
        lockfiles=@($locks)
        runtime_asset_presence=@($assets)
        proof=[pscustomobject]@{
            source_checks='NOT_EXECUTED_BY_STATUS'
            model_integrity='NOT_EXECUTED'
            gpu_inference='NOT_EXECUTED'
            microphone_and_virtual_audio='NOT_EXECUTED'
            installed_product='NOT_EXECUTED'
        }
    }
}

function Show-TranslateITReadiness {
    param([Parameter(Mandatory=$true)][object]$Readiness)
    Write-Host 'TranslateIT Windows workspace status (read-only)'
    $revision = if ($Readiness.revision) { $Readiness.revision } else { 'UNKNOWN' }
    Write-Host "Source commit: $revision"
    foreach ($item in $Readiness.tools) {
        Write-Host ('  {0,-7} {1,-11} {2,-9} {3}' -f $item.name,$item.state,$item.origin,$item.version)
    }
    $locksPresent=@($Readiness.lockfiles | Where-Object { $_.state -eq 'PRESENT' }).Count
    $assetsPresent=@($Readiness.runtime_asset_presence | Where-Object { $_.state -eq 'PRESENT_UNVERIFIED' }).Count
    Write-Host "Lockfiles: $locksPresent/$($Readiness.lockfiles.Count) present."
    Write-Host "Runtime asset markers: $assetsPresent/$($Readiness.runtime_asset_presence.Count) present; integrity NOT VERIFIED."
    if (-not $Readiness.workspace.toolchain_managed) {
        Write-Host 'Fully workspace-managed toolchain: NOT PROVEN (external/missing tools).'
    }
    Write-Host 'AI/GPU, actual audio, driver and installed app: NOT EXECUTED.'
}

function Write-TranslateITReadinessReport {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][object]$Readiness,
          [Parameter(Mandatory=$true)][string]$WorkspaceRoot)
    $dir=Join-Path $WorkspaceRoot 'reports'
    if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
        throw 'Workspace report directory is not initialized.'
    }
    $destination=Join-Path $dir 'readiness.json'
    $temporary=Join-Path $dir ('readiness-'+[guid]::NewGuid().ToString('N')+'.tmp')
    try {
        $Readiness | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $temporary -Encoding UTF8
        Move-Item -LiteralPath $temporary -Destination $destination -Force -ErrorAction Stop
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
    Write-Host "Workspace source-readiness report: $destination"
    return $destination
}

Export-ModuleMember -Function Get-TranslateITReadiness,Show-TranslateITReadiness,Write-TranslateITReadinessReport

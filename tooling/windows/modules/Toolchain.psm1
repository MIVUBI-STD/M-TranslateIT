# Windows developer tool selection. No downloads or global PATH mutations.
# A complete local group wins as a unit; never mix a partial local installation
# with Windows executables silently. Only the invoking process is modified.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-TranslateITRegularPath {
    param([string]$Path, [switch]$Directory)
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Workspace tool path may not be a symlink or junction: $Path"
    }
    if ($Directory -and -not $item.PSIsContainer) {
        throw "Expected a normal workspace tool directory: $Path"
    }
    if (-not $Directory -and $item.PSIsContainer) {
        throw "Expected a regular workspace tool file: $Path"
    }
    return $true
}

function Get-TranslateITWorkspaceToolSelection {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$RepositoryRoot)

    $root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $workspace = Join-Path $root '.workspace'
    $toolsRoot = Join-Path $workspace 'tools'
    foreach ($directory in @($workspace, $toolsRoot)) {
        $null = Assert-TranslateITRegularPath -Path $directory -Directory
    }

    $groups = @(
        [pscustomobject]@{
            name='node'; directory='node'
            names=@('node','npm'); files=@('node.exe','npm.cmd')
        },
        [pscustomobject]@{
            name='python'; directory='python'
            names=@('python'); files=@('python.exe')
        },
        [pscustomobject]@{
            name='uv'; directory='uv'
            names=@('uv'); files=@('uv.exe')
        },
        [pscustomobject]@{
            name='rust'; directory='cargo/bin'
            names=@('rustc','cargo','rustup')
            files=@('rustc.exe','cargo.exe','rustup.exe')
        }
    )
    $result = @()
    foreach ($group in $groups) {
        $folder = Join-Path $toolsRoot $group.directory
        $parent = Split-Path -Parent $folder
        if ($parent -ne $toolsRoot) {
            $null = Assert-TranslateITRegularPath -Path $parent -Directory
        }
        $null = Assert-TranslateITRegularPath -Path $folder -Directory
        $found = @()
        foreach ($file in $group.files) {
            $full = Join-Path $folder $file
            $found += [bool](Assert-TranslateITRegularPath -Path $full)
        }
        $present = @($found | Where-Object { $_ }).Count
        $commandFolder = $folder
        $resolvedPaths = @($group.files | ForEach-Object { Join-Path $folder $_ })
        if ($group.name -eq 'python' -and $present -eq 0 -and
            (Test-Path -LiteralPath $folder -PathType Container)) {
            # uv installs the managed CPython layout below UV_PYTHON_INSTALL_DIR,
            # not into a flat tools/python/python.exe shim.
            $managed = @(Get-ChildItem -LiteralPath $folder -Directory -Filter 'cpython-3.12.10-*')
            if ($managed.Count -eq 1) {
                $commandFolder = $managed[0].FullName
                $python = Join-Path $commandFolder 'python.exe'
                if (Assert-TranslateITRegularPath -Path $python) {
                    $present = 1
                    $resolvedPaths = @($python)
                } else {
                    $present = -1
                }
            } elseif ($managed.Count -gt 1) {
                $present = -1
            }
        }
        $complete = $present -eq $group.files.Count
        if ($group.name -eq 'rust' -and $present -gt 0) {
            $rustup = Join-Path $toolsRoot 'rustup'
            $toolchains = Join-Path $rustup 'toolchains'
            $null = Assert-TranslateITRegularPath -Path $rustup -Directory
            $complete = $complete -and
                [bool](Assert-TranslateITRegularPath -Path $toolchains -Directory)
        }
        $state = if ($present -eq 0) { 'ABSENT' } elseif ($complete) { 'COMPLETE' } else { 'INCOMPLETE' }
        $result += [pscustomobject]@{
            name=$group.name
            state=$state
            directory=$commandFolder
            programs=@($group.names)
            paths=$resolvedPaths
        }
    }
    return $result
}

function Get-TranslateITPreferredTool {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)][string]$RepositoryRoot,
        [Parameter(Mandatory=$true)][string]$Name
    )
    $groups = @(Get-TranslateITWorkspaceToolSelection -RepositoryRoot $RepositoryRoot)
    foreach ($group in $groups) {
        $index = [array]::IndexOf([string[]]$group.programs, $Name)
        if ($index -lt 0) { continue }
        if ($group.state -eq 'INCOMPLETE') {
            return [pscustomobject]@{
                name=$Name; state='INCOMPLETE_LOCAL'; origin='workspace'
                path=$null; directory=$group.directory
            }
        }
        if ($group.state -eq 'COMPLETE') {
            return [pscustomobject]@{
                name=$Name; state='SELECTED'; origin='workspace'
                path=$group.paths[$index]; directory=$group.directory
            }
        }
        break
    }

    # PATH-only fallback remains observable and is never called a portable tool.
    $queryName = if ($Name -eq 'npm') { 'npm.cmd' } else { $Name }
    $external = Get-Command -Name $queryName -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandType -eq 'Application' } | Select-Object -First 1
    if ($null -eq $external) {
        return [pscustomobject]@{
            name=$Name; state='MISSING'; origin='none'; path=$null; directory=$null
        }
    }
    return [pscustomobject]@{
        name=$Name; state='SELECTED'; origin='external'
        path=[IO.Path]::GetFullPath($external.Source); directory=$null
    }
}

function Use-TranslateITWorkspaceToolchain {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$RepositoryRoot)
    $groups = @(Get-TranslateITWorkspaceToolSelection -RepositoryRoot $RepositoryRoot)
    foreach ($group in $groups) {
        if ($group.state -eq 'INCOMPLETE') {
            throw "Incomplete workspace tool group '$($group.name)' at $($group.directory); refusing a mixed PATH/toolchain."
        }
    }
    $prepend = @()
    foreach ($group in $groups) {
        if ($group.state -eq 'COMPLETE') { $prepend += $group.directory }
    }
    $original = @($env:PATH -split ';' | Where-Object { $_ })
    $newParts = @($prepend)
    foreach ($segment in $original) {
        if (-not ($newParts | Where-Object { $_.TrimEnd('\') -ieq $segment.TrimEnd('\') })) {
            $newParts += $segment
        }
    }
    $env:PATH = $newParts -join ';'
    $workspace = Join-Path (Resolve-Path -LiteralPath $RepositoryRoot).Path '.workspace'
    $env:CARGO_HOME = Join-Path $workspace 'tools/cargo'
    $localRust = @($groups | Where-Object { $_.name -eq 'rust' -and $_.state -eq 'COMPLETE' })
    if ($localRust.Count -eq 1) {
        $env:RUSTUP_HOME = Join-Path $workspace 'tools/rustup'
    }
    # External rustup is intentionally reported as external. Its existing
    # compiler may still be required for source builds until portable staging
    # has been independently reviewed; no toolchain is installed here.
}

Export-ModuleMember -Function Get-TranslateITWorkspaceToolSelection,Get-TranslateITPreferredTool,Use-TranslateITWorkspaceToolchain

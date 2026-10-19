# Exact pinned portable Windows tool bootstrap.
# Caller explicitly requests 'prepare'; no global PATH/registry writes or UserData access.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Toolchain.psm1') -Force

function Get-TranslateITVerifiedToolArchive {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$Url,
          [Parameter(Mandatory=$true)][string]$ExpectedSha256,
          [Parameter(Mandatory=$true)][string]$CacheRoot)
    if ($ExpectedSha256 -cnotmatch '^[a-fA-F0-9]{64}$') { throw 'Portable tool SHA-256 pin missing.' }
    $uri = [Uri]$Url
    if ($uri.Scheme -ne 'https' -or $uri.Host -notin @('nodejs.org','github.com','static.rust-lang.org')) {
        throw "Untrusted portable tool source: $Url"
    }
    if (-not (Test-Path -LiteralPath $CacheRoot -PathType Container)) {
        throw 'Workspace download cache must be initialized first.'
    }
    $cacheDirectory = Get-Item -LiteralPath $CacheRoot -Force
    if ($cacheDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'Portable download cache must not be a filesystem redirect.'
    }
    $sha = $ExpectedSha256.ToLowerInvariant()
    $name = 'sha256-' + $sha + [IO.Path]::GetExtension($uri.AbsolutePath)
    $dest = Join-Path $CacheRoot $name
    if (Test-Path -LiteralPath $dest) {
        $file=Get-Item -LiteralPath $dest -Force
        if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Unsafe cached portable tool: $dest"
        }
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $dest).Hash.ToLowerInvariant() -ne $sha) {
            throw "Cached portable tool checksum mismatch. Preserve for review: $dest"
        }
        return $dest
    }
    $partial = Join-Path $CacheRoot ('.download-' + [guid]::NewGuid().ToString('N') + '.partial')
    # Preserve unfinished bytes for inspection; only the hash-verified archive
    # is ever promoted under its content-addressed final name.
    Invoke-WebRequest -Uri $Url -OutFile $partial
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $partial).Hash.ToLowerInvariant() -ne $sha) {
        throw "Portable tool download checksum mismatch: $Url"
    }
    Move-Item -LiteralPath $partial -Destination $dest -ErrorAction Stop
    return $dest
}

function Publish-TranslateITPortableArchive {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$Archive,
          [Parameter(Mandatory=$true)][string]$Stage,
          [Parameter(Mandatory=$true)][string]$SourceFolder,
          [Parameter(Mandatory=$true)][string]$TargetFolder,
          [Parameter(Mandatory=$true)][string[]]$RequiredFiles)
    if (Test-Path -LiteralPath $TargetFolder) { throw "Portable target already exists; never overwrite: $TargetFolder" }
    New-Item -ItemType Directory -Path $Stage -ErrorAction Stop | Out-Null
    $unpacked = Join-Path $Stage 'unpacked'
    Expand-Archive -LiteralPath $Archive -DestinationPath $unpacked -ErrorAction Stop
    $source = if ($SourceFolder -eq '.') { $unpacked } else { Join-Path $unpacked $SourceFolder }
    if (-not (Test-Path -LiteralPath $source -PathType Container)) {
        throw "Unexpected pinned portable archive structure: $SourceFolder"
    }
    foreach ($relative in $RequiredFiles) {
        $path=Join-Path $source $relative
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Pinned portable archive missing executable: $relative"
        }
        $item=Get-Item -LiteralPath $path -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Portable archive contains a redirected executable: $relative"
        }
    }
    Move-Item -LiteralPath $source -Destination $TargetFolder -ErrorAction Stop
}


function Get-TranslateITPortableDownloadPlan {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)][object]$Pins,
        [Parameter(Mandatory=$true)][string]$ToolsRoot,
        [Parameter(Mandatory=$true)][string]$CacheRoot
    )
    # Hash-check every missing public archive before publishing any tool.
    # A transient download failure cannot leave half the toolchain installed.
    $plan = @{}
    foreach ($entry in @(
        [pscustomobject]@{ name='node'; root='node'; pin=$Pins.node },
        [pscustomobject]@{ name='git'; root='git'; pin=$Pins.git },
        [pscustomobject]@{ name='uv'; root='uv'; pin=$Pins.uv },
        [pscustomobject]@{ name='rustup'; root='cargo/bin'; pin=$Pins.rustup }
    )) {
        if (Test-Path -LiteralPath (Join-Path $ToolsRoot $entry.root)) { continue }
        if ($null -eq $entry.pin -or -not $entry.pin.url -or -not $entry.pin.sha256) {
            throw "Missing pinned portable tool source: $($entry.name)"
        }
        $plan[$entry.name] = Get-TranslateITVerifiedToolArchive -Url $entry.pin.url -ExpectedSha256 $entry.pin.sha256 -CacheRoot $CacheRoot
    }
    return $plan
}

function Install-TranslateITPortableToolchain {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$RepositoryRoot)
    if (-not [Environment]::Is64BitOperatingSystem -or
        [Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'Portable tool bootstrap supports Windows x64 only.'
    }
    $root=(Resolve-Path -LiteralPath $RepositoryRoot).Path
    $workspace=Join-Path $root '.workspace'
    $tools=Join-Path $workspace 'tools'
    $cache=Join-Path $workspace 'downloads/verified'
    $stageRoot=Join-Path $workspace 'staging'
    # These parent directories are created by the canonical workspace owner.
    # Reject redirections at each named boundary, not just the final cache.
    foreach ($path in @($workspace,$tools,$cache,$stageRoot)) {
        if (-not (Test-Path -LiteralPath $path -PathType Container)) {
            New-Item -ItemType Directory -Path $path -Force | Out-Null
        }
        $item=Get-Item -LiteralPath $path -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Unsafe portable tool workspace directory: $path"
        }
    }
    # Do not allow a second installer to run concurrently or resume a damaged run
    # by deleting intermediate evidence. A stopped stage requires explicit review.
    $reservation=Join-Path $stageRoot 'portable-toolchain-active'
    New-Item -ItemType Directory -Path $reservation -ErrorAction Stop | Out-Null
    $run=Join-Path $stageRoot ('portable-toolchain-'+[guid]::NewGuid().ToString('N'))
    $finished=$false
    $installationStarted=$false
    try {
        $policy=Get-Content -LiteralPath (Join-Path $root 'tooling/windows/config/requirements.json') -Raw | ConvertFrom-Json
        $pins=$policy.portable
        if (-not $pins) { throw 'Portable tool pins are missing from canonical requirements.' }
        $existing = @(Get-TranslateITWorkspaceToolSelection -RepositoryRoot $root)
        $incomplete = @($existing | Where-Object { $_.state -eq 'INCOMPLETE' })
        if ($incomplete.Count -gt 0) {
            throw "Existing portable groups require review: $(($incomplete | ForEach-Object name) -join ', ')"
        }
        if ((Test-Path -LiteralPath (Join-Path $tools 'rustup')) -and
            -not (Test-Path -LiteralPath (Join-Path $tools 'cargo/bin'))) {
            throw 'Interrupted portable Rustup home requires explicit review.'
        }
        $archives = Get-TranslateITPortableDownloadPlan -Pins $pins -ToolsRoot $tools -CacheRoot $cache

        $node=Join-Path $tools 'node'
        if (-not (Test-Path -LiteralPath $node)) {
            $zip=$archives.node
            $source='node-v'+$pins.node.version+'-win-x64'
            $installationStarted=$true
            Publish-TranslateITPortableArchive -Archive $zip -Stage (Join-Path $run 'node') -SourceFolder $source -TargetFolder $node -RequiredFiles @('node.exe','npm.cmd')
            $actual=(& (Join-Path $node 'node.exe') --version).Trim()
            if ($LASTEXITCODE -ne 0 -or $actual -ne ('v'+$pins.node.version)) { throw 'Pinned portable Node version check failed.' }
        }

        # MinGit is the reviewed Git for Windows command-line-only distribution.
        # It is installed under this workspace and never writes an MSI/registry entry.
        $git=Join-Path $tools 'git'
        if (-not (Test-Path -LiteralPath $git)) {
            $zip=$archives.git
            $installationStarted=$true
            Publish-TranslateITPortableArchive -Archive $zip -Stage (Join-Path $run 'git') -SourceFolder '.' -TargetFolder $git -RequiredFiles @('cmd/git.exe')
            $actual=(& (Join-Path $git 'cmd/git.exe') --version | Out-String).Trim()
            if ($LASTEXITCODE -ne 0 -or $actual -ne ('git version '+$pins.git.version)) {
                throw 'Pinned portable Git version check failed.'
            }
        }

        $uv=Join-Path $tools 'uv'
        if (-not (Test-Path -LiteralPath $uv)) {
            $zip=$archives.uv
            $installationStarted=$true
            Publish-TranslateITPortableArchive -Archive $zip -Stage (Join-Path $run 'uv') -SourceFolder '.' -TargetFolder $uv -RequiredFiles @('uv.exe')
            $actual=(& (Join-Path $uv 'uv.exe') --version | Out-String).Trim()
            if ($LASTEXITCODE -ne 0 -or $actual -ne ('uv '+$pins.uv.version)) { throw 'Pinned portable uv version check failed.' }
        }

        # Existing locally-managed Python is acquired by the selected, pinned uv.
        # uv verifies its managed distribution; never download an unpinned
        # standalone interpreter through an unrelated script.
        $env:UV_PYTHON_INSTALL_DIR = Join-Path $tools 'python'
        $env:UV_PYTHON_INSTALL_BIN = '0'
        $env:UV_PYTHON_PREFERENCE = 'only-managed'
        $uvexe=Join-Path $uv 'uv.exe'
        $installationStarted=$true
        & $uvexe python install $policy.python.version --no-bin
        if ($LASTEXITCODE -ne 0) { throw 'Pinned uv-managed Python download/install failed.' }

        # Rustup's own distribution manager verifies toolchain component
        # manifests. The rustup-init bootstrap itself has an exact SHA-256 pin.
        $cargo=Join-Path $tools 'cargo'
        $rustup=Join-Path $tools 'rustup'
        $bin=Join-Path $cargo 'bin'
        if (-not (Test-Path -LiteralPath $bin)) {
            if (Test-Path -LiteralPath $rustup) {
                throw 'Incomplete portable Rustup home requires explicit recovery, not replacement.'
            }
            $verified=$archives.rustup
            $installationStarted=$true
            # rustup-init is a multi-call executable; preserve its required
            # basename when executing instead of invoking the SHA cache name.
            New-Item -ItemType Directory -Path $run -Force | Out-Null
            $bootstrap=Join-Path $run 'rustup-init.exe'
            Copy-Item -LiteralPath $verified -Destination $bootstrap -ErrorAction Stop
            $env:CARGO_HOME=$cargo
            $env:RUSTUP_HOME=$rustup
            & $bootstrap -y --no-modify-path --profile minimal --default-host $pins.rust.host --default-toolchain $pins.rust.version
            if ($LASTEXITCODE -ne 0) { throw 'Pinned portable rustup initialization failed.' }
        }
        $groups=@(Get-TranslateITWorkspaceToolSelection -RepositoryRoot $root)
        $invalid=@($groups | Where-Object { $_.state -ne 'COMPLETE' })
        if ($invalid.Count -gt 0) {
            throw "Portable tool groups remain incomplete: $(($invalid | ForEach-Object name) -join ', ')"
        }
        # Existing groups must meet the same exact version requirements as
        # newly provisioned ones. File presence alone never upgrades proof.
        $env:CARGO_HOME=Join-Path $tools 'cargo'
        $env:RUSTUP_HOME=Join-Path $tools 'rustup'
        foreach ($check in @(
            @('Node', (Join-Path $tools 'node/node.exe'), ('v'+$pins.node.version)),
            @('Git', (Join-Path $tools 'git/cmd/git.exe'), ('git version '+$pins.git.version)),
            @('uv', (Join-Path $tools 'uv/uv.exe'), ('uv '+$pins.uv.version))
        )) {
            $program=[string]$check[1]
            $actual=(& $program --version 2>&1 | Out-String).Trim()
            if ($LASTEXITCODE -ne 0 -or $actual -ne [string]$check[2]) {
                throw "Portable $($check[0]) does not match the pinned version."
            }
        }
        $python=(Get-TranslateITPreferredTool -RepositoryRoot $root -Name 'python')
        if ($python.state -ne 'SELECTED' -or $python.origin -ne 'workspace') {
            throw 'Pinned managed Python was not resolved from this workspace.'
        }
        $pythonVersion=(& $python.path --version 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or $pythonVersion -ne ('Python '+$policy.python.version)) {
            throw 'Portable managed Python version mismatches the canonical requirement.'
        }
        $rustVersion=(& (Join-Path $bin 'rustc.exe') --version 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $rustVersion.StartsWith(('rustc '+$pins.rust.version+' '))) {
            throw 'Portable Rust compiler does not match the pinned distribution.'
        }
        $rustupVersion=(& (Join-Path $bin 'rustup.exe') --version 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $rustupVersion.StartsWith(('rustup '+$pins.rustup.version+' '))) {
            throw 'Portable rustup version does not match the pinned bootstrap.'
        }
        $finished=$true
        Write-Host 'Portable Node, Git, uv, managed Python, Rustup and Rust toolchain are prepared in the selected repository workspace.'
    } finally {
        if ($finished) {
            if (Test-Path -LiteralPath $run) { Remove-Item -LiteralPath $run -Recurse -Force }
            Remove-Item -LiteralPath $reservation -Recurse -Force
        } elseif (-not $installationStarted) {
            # No public tool was modified. Keep unfinished download bytes for
            # diagnosis but allow a normal prepare retry after network failure.
            Remove-Item -LiteralPath $reservation -Recurse -Force
        }
        # If live installation started, retain the reservation; no overwrite
        # of an unverified partly installed toolchain is ever automatic.
    }
}

Export-ModuleMember -Function Get-TranslateITVerifiedToolArchive,Get-TranslateITPortableDownloadPlan,Publish-TranslateITPortableArchive,Install-TranslateITPortableToolchain

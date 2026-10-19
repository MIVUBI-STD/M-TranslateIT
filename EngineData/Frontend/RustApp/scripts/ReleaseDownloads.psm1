# Content-addressed, checked release archives. Never accept corrupt cached bytes.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-VerifiedReleaseArchive {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][string]$CacheRoot
    )
    if ($ExpectedSha256 -cnotmatch '^[0-9a-fA-F]{64}$') {
        throw "Release archive cache requires a pinned 64-character SHA-256."
    }
    if (-not [Uri]::IsWellFormedUriString($Url, [UriKind]::Absolute) -or
        ([Uri]$Url).Scheme -ne "https") {
        throw "Release archive source must use HTTPS."
    }
    New-Item -ItemType Directory -Path $CacheRoot -Force | Out-Null
    $cacheDir = Get-Item -LiteralPath $CacheRoot -Force
    if ($cacheDir.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing redirected release download cache."
    }

    $expected = $ExpectedSha256.ToLowerInvariant()
    $cacheFile = Join-Path $CacheRoot ("sha256-" + $expected + [IO.Path]::GetExtension($Destination))
    $cached = Test-Path -LiteralPath $cacheFile
    if ($cached) {
        $item = Get-Item -LiteralPath $cacheFile -Force
        if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Refusing unsafe cached release archive: $cacheFile"
        }
        $digest = (Get-FileHash -Algorithm SHA256 -LiteralPath $cacheFile).Hash.ToLowerInvariant()
        if ($digest -ne $expected) {
            throw "Cached archive checksum mismatch; preserve for diagnosis: $cacheFile"
        }
        Write-Host "[release-stage][reuse] Verified archive: $cacheFile"
    } else {
        $incoming = Join-Path $CacheRoot (".downloading-" + [guid]::NewGuid().ToString("N") + ".part")
        try {
            Invoke-WebRequest -Uri $Url -OutFile $incoming
            $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $incoming).Hash.ToLowerInvariant()
            if ($actual -ne $expected) { throw "Downloaded archive checksum mismatch: $Url" }
            # The staging owner holds an exclusive release-active reservation.
            Move-Item -LiteralPath $incoming -Destination $cacheFile -ErrorAction Stop
        } catch {
            # Preserve failed bytes for explicit investigation, do not put them
            # under a verified sha256 filename.
            throw
        }
    }
    Copy-Item -LiteralPath $cacheFile -Destination $Destination -Force -ErrorAction Stop
    return $Destination
}

Export-ModuleMember -Function Get-VerifiedReleaseArchive

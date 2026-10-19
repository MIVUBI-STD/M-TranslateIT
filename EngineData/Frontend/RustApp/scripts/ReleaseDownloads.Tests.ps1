# Offline fixture: content-addressed reuse and poisoned-cache rejection.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'ReleaseDownloads.psm1') -Force
$fixture = Join-Path $env:RUNNER_TEMP ("translateit-download-cache-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    $cache = Join-Path $fixture "cache"
    New-Item -ItemType Directory -Path $cache | Out-Null
    $bytes = [Text.Encoding]::UTF8.GetBytes("trusted immutable test archive")
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $sha = ([BitConverter]::ToString($hasher.ComputeHash($bytes))).Replace("-", "").ToLowerInvariant() }
    finally { $hasher.Dispose() }
    $cached = Join-Path $cache ("sha256-" + $sha + ".zip")
    [IO.File]::WriteAllBytes($cached,$bytes)
    $first = Join-Path $fixture "archive.zip"
    $null = Get-VerifiedReleaseArchive -Url 'https://example.invalid/offline-only.zip' -Destination $first -ExpectedSha256 $sha -CacheRoot $cache
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $first).Hash.ToLowerInvariant() -ne $sha) {
        throw "Verified cache did not provide the expected bytes."
    }
    [IO.File]::WriteAllBytes($cached,[byte[]](1,2,3))
    $blocked = $false
    try {
        $null = Get-VerifiedReleaseArchive -Url 'https://example.invalid/offline-only.zip' -Destination (Join-Path $fixture "second.zip") -ExpectedSha256 $sha -CacheRoot $cache
    } catch { $blocked = $true }
    if (-not $blocked) { throw "Corrupted cached bytes must be rejected, never trusted." }
    Write-Host "Release download verified cache reuse and corrupt-cache rejection: PASS"
} finally {
    Remove-Item -LiteralPath $fixture -Recurse -Force -ErrorAction Stop
}

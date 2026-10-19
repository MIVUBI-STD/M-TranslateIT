# Offline Windows bootstrap fixtures. No network access or real tool installation.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '..\modules\PortableToolchain.psm1') -Force

$root=Join-Path $env:RUNNER_TEMP ('translateit-portable-bootstrap-fixture-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
try {
    $cache=Join-Path $root '.workspace\downloads\verified'
    New-Item -ItemType Directory -Path $cache -Force | Out-Null
    $bytes=[Text.Encoding]::UTF8.GetBytes('fixture-checked-archive')
    $hasher=[Security.Cryptography.SHA256]::Create()
    try { $digest=([BitConverter]::ToString($hasher.ComputeHash($bytes))).Replace('-','').ToLowerInvariant() }
    finally { $hasher.Dispose() }
    $cached=Join-Path $cache ('sha256-'+$digest+'.zip')
    [IO.File]::WriteAllBytes($cached,$bytes)
    $url='https://nodejs.org/dist/v22.23.3/node-v22.23.3-win-x64.zip'
    $chosen=Get-TranslateITVerifiedToolArchive -Url $url -ExpectedSha256 $digest -CacheRoot $cache
    if ($chosen -ne $cached) { throw 'Verified content-addressed cache reuse failed.' }
    $rejected=$false
    try { $null=Get-TranslateITVerifiedToolArchive -Url 'http://nodejs.org/unknown' -ExpectedSha256 $digest -CacheRoot $cache }
    catch { $rejected=$true }
    if (-not $rejected) { throw 'Non-HTTPS tool source was accepted.' }
    $rejected=$false
    try { $null=Get-TranslateITVerifiedToolArchive -Url 'https://untrusted.example/file.zip' -ExpectedSha256 $digest -CacheRoot $cache }
    catch { $rejected=$true }
    if (-not $rejected) { throw 'Untrusted tool host was accepted.' }
    [IO.File]::WriteAllBytes($cached,([byte[]](0,1,2)))
    $rejected=$false
    try { $null=Get-TranslateITVerifiedToolArchive -Url $url -ExpectedSha256 $digest -CacheRoot $cache }
    catch { $rejected=$true }
    if (-not $rejected) { throw 'Poisoned cache must fail closed without overwriting.' }

    # Build a harmless archive with the public Node distribution's folder shape.
    $source=Join-Path $root 'fixture\node-v22.23.3-win-x64'
    New-Item -ItemType Directory -Path $source -Force | Out-Null
    foreach ($file in @('node.exe','npm.cmd')) {
        Set-Content -LiteralPath (Join-Path $source $file) -Value 'fake placeholder' -Encoding ASCII
    }
    $archive=Join-Path $root 'fake-node.zip'
    Compress-Archive -Path $source -DestinationPath $archive
    $dest=Join-Path $root '.workspace\tools\node'
    $stage=Join-Path $root '.workspace\staging\node-fixture'
    New-Item -ItemType Directory -Path (Split-Path -Parent $dest) -Force | Out-Null
    Publish-TranslateITPortableArchive -Archive $archive -Stage $stage -SourceFolder 'node-v22.23.3-win-x64' -TargetFolder $dest -RequiredFiles @('node.exe','npm.cmd')
    if (-not (Test-Path -LiteralPath (Join-Path $dest 'npm.cmd') -PathType Leaf)) {
        throw 'Verified local archive did not publish complete Node group.'
    }
    # MinGit is a normal ZIP with cmd/git.exe at the root of the archive.
    $gitSource = Join-Path $root 'git-fixture\cmd'
    New-Item -ItemType Directory -Path $gitSource -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $gitSource 'git.exe') -Value 'fake git binary' -Encoding Ascii
    $gitZip = Join-Path $root 'fake-git.zip'
    Compress-Archive -Path (Join-Path $root 'git-fixture\*') -DestinationPath $gitZip
    $gitTarget = Join-Path $root '.workspace\tools\git'
    Publish-TranslateITPortableArchive -Archive $gitZip -Stage (Join-Path $root 'git-stage') -SourceFolder '.' -TargetFolder $gitTarget -RequiredFiles @('cmd/git.exe')
    if (-not (Test-Path -LiteralPath (Join-Path $gitTarget 'cmd\git.exe') -PathType Leaf)) {
        throw 'Portable Git archive did not preserve the expected cmd layout.'
    }
    $rejected=$false
    try { Publish-TranslateITPortableArchive -Archive $archive -Stage (Join-Path $root 'second') -SourceFolder 'node-v22.23.3-win-x64' -TargetFolder $dest -RequiredFiles @('node.exe','npm.cmd') }
    catch { $rejected=$true }
    if (-not $rejected) { throw 'An existing portable tool folder was overwritten.' }
    Write-Host 'Portable pinned download, cache poison, archive staging and overwrite refusal: PASS'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction Stop
}

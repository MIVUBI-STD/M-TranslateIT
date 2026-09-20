param(
    [Parameter(Mandatory=$true)]
    [ValidateSet('Verify','Install','VerifyInstalled')]
    [string]$Mode,
    [string]$PayloadPath,
    [string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$ExpectedPayloadSchema,
    [Parameter(Mandatory=$true)][string]$ExpectedInstalledRuntimeSchema,
    [Parameter(Mandatory=$true)][string]$ExpectedAppVersion,
    [Int64]$ExpectedExpandedBytes=0,
    [string]$InstallRoot
)

$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest

$PythonVersion='3.12.10'
$TorchVersion='2.11.0'
$TransformersVersion='4.57.6'
$TokenizersVersion='0.22.2'
$MiLMMTRevision='4fc480b6c58dec29c159dcdf9fde0f6d5c354995'
$AsrRevision='0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf'
$GptRevision='d523079fc05d9a8028d6085bffe4a2757c32abb6'
$PayloadContract='EngineData\Backend\LocalWorker\PythonRuntime\TRANSLATEIT_PAYLOAD_CONTRACT.json'
$InstalledManifest='EngineData\Backend\TRANSLATEIT_INSTALLED_RUNTIME.json'
$PayloadRoots=@(
    'EngineData\Backend\LocalWorker\PythonRuntime',
    'EngineData\Backend\RuntimeAssets\ASR\ModelData',
    'EngineData\Backend\RuntimeAssets\Translation\ModelData',
    'EngineData\Backend\RuntimeAssets\Voice\BuiltInVoices',
    'EngineData\Backend\RuntimeAssets\Voice\GPTSoVITS',
    'EngineData\Backend\RuntimeAssets\AudioProvider\VBCABLE\Package'
)

function Fail([int]$Code,[string]$Message){[Console]::Error.WriteLine("[translateit-r3-installer] $Message");exit $Code}
function NeedFile([string]$Path,[int]$Code,[string]$Label){if(-not(Test-Path -LiteralPath $Path -PathType Leaf)){Fail $Code "$Label missing: $Path"}}
function NeedMarker([string]$Path,[string]$Expected,[int]$Code,[string]$Label){NeedFile $Path $Code $Label;if((Get-Content -LiteralPath $Path -Raw).Trim()-ne$Expected){Fail $Code "$Label mismatch"}}

function Get-TarProgram {
    $systemTar=Join-Path $env:SystemRoot 'System32\tar.exe'
    if(Test-Path -LiteralPath $systemTar -PathType Leaf){return $systemTar}
    try{return(Get-Command tar.exe -ErrorAction Stop).Source}catch{Fail 40 'Windows tar.exe/bsdtar is required.'}
}

function Read-PayloadContract([string]$Tar,[string]$Archive){
    $entry=$PayloadContract.Replace('\','/')
    $raw=@(&$Tar -xOf $Archive $entry 2>&1)
    if($LASTEXITCODE-ne 0-or$raw.Count-eq 0){Fail 43 'Payload contract missing or unreadable.'}
    try{return([string]::Join("`n",$raw)|ConvertFrom-Json)}catch{Fail 43 'Payload contract JSON invalid.'}
}

function Assert-Contract($Contract){
    if([string]$Contract.schema-ne$ExpectedPayloadSchema-or[string]$Contract.app_version-ne$ExpectedAppVersion){Fail 44 'Payload identity does not match Setup.'}
    if([string]$Contract.python_version-ne$PythonVersion-or[string]$Contract.torch_version-ne$TorchVersion-or[string]$Contract.transformers_version-ne$TransformersVersion-or[string]$Contract.tokenizers_version-ne$TokenizersVersion){Fail 44 'Payload dependency identity mismatch.'}
    if([string]$Contract.revisions.milmmt-ne$MiLMMTRevision-or[string]$Contract.revisions.faster_whisper-ne$AsrRevision-or[string]$Contract.revisions.gpt_sovits-ne$GptRevision){Fail 44 'Payload model revision identity mismatch.'}
}

function Assert-Runtime([string]$Root){
    $backend=Join-Path $Root 'EngineData\Backend'
    NeedFile (Join-Path $backend 'LocalWorker\PythonRuntime\python.exe') 50 'Python runtime'
    NeedMarker (Join-Path $backend 'RuntimeAssets\ASR\ModelData\faster-whisper-large-v3-turbo\.translateit_model_revision') $AsrRevision 51 'ASR revision'
    NeedMarker (Join-Path $backend 'RuntimeAssets\Translation\ModelData\xiaomi-research--MiLMMT-46-1B-v1.0\.translateit_model_revision') $MiLMMTRevision 52 'MiLMMT revision'
    NeedFile (Join-Path $backend 'RuntimeAssets\Voice\BuiltInVoices\MaleVoice\reference.wav') 53 'Built-in Male reference'
    NeedFile (Join-Path $backend 'RuntimeAssets\Voice\BuiltInVoices\FemaleVoice\reference.wav') 53 'Built-in Female reference'
    NeedFile (Join-Path $backend 'RuntimeAssets\Voice\BuiltInVoices\SOURCES.json') 53 'Built-in voice source manifest'
    NeedMarker (Join-Path $backend 'RuntimeAssets\Voice\GPTSoVITS\Source\TRANSLATEIT_GPTSOVITS_REVISION.txt') $GptRevision 53 'GPT-SoVITS revision'
    NeedFile (Join-Path $backend 'RuntimeAssets\AudioProvider\VBCABLE\Package\VBCABLE_Setup_x64.exe') 54 'VB-CABLE x64 installer'
    NeedFile (Join-Path $backend 'RuntimeAssets\AudioProvider\VBCABLE\Package\VBCABLE_Setup.exe') 54 'VB-CABLE x86 installer'
}

function Read-PythonMetadata([string]$Root){
    $python=Join-Path $Root 'EngineData\Backend\LocalWorker\PythonRuntime\python.exe'
    $code='import importlib.metadata as m,json,sys;print(json.dumps({"python":".".join(map(str,sys.version_info[:3])),"torch":m.version("torch"),"transformers":m.version("transformers"),"tokenizers":m.version("tokenizers")}))'
    $raw=(&$python -s -c $code 2>&1|Out-String).Trim()
    if($LASTEXITCODE-ne 0){Fail 55 'Private Python dependency metadata unavailable.'}
    try{$meta=$raw|ConvertFrom-Json}catch{Fail 55 'Private Python dependency metadata invalid.'}
    if($meta.python-ne$PythonVersion-or$meta.torch-ne$TorchVersion-or$meta.transformers-ne$TransformersVersion-or$meta.tokenizers-ne$TokenizersVersion){Fail 56 'Installed Python dependency versions do not match the payload contract.'}
    return $meta
}

function Assert-InstalledRuntimeManifest([string]$Root){
    $manifestPath=Join-Path $Root $InstalledManifest
    NeedFile $manifestPath 57 'Installed runtime manifest'
    try{$manifest=Get-Content -LiteralPath $manifestPath -Raw|ConvertFrom-Json}catch{Fail 57 'Installed runtime manifest invalid.'}
    if([string]$manifest.schema-ne$ExpectedInstalledRuntimeSchema-or -not [bool]$manifest.installed_complete){Fail 57 'Installed runtime manifest is incomplete or incompatible.'}
    if([string]$manifest.payload_schema-ne$ExpectedPayloadSchema){Fail 57 'Installed runtime payload schema is incompatible with this app update.'}
    if([string]$manifest.python-ne$PythonVersion-or[string]$manifest.torch-ne$TorchVersion-or[string]$manifest.transformers-ne$TransformersVersion-or[string]$manifest.tokenizers-ne$TokenizersVersion){Fail 57 'Installed runtime dependency identity is incompatible with this app update.'}
    if([string]$manifest.translation_revision-ne$MiLMMTRevision-or[string]$manifest.asr_revision-ne$AsrRevision-or[string]$manifest.voice_revision-ne$GptRevision){Fail 57 'Installed model/runtime revisions are incompatible with this app update.'}
}

function Verify-InstalledRuntime([string]$Root){
    if([string]::IsNullOrWhiteSpace($Root)){Fail 58 'InstallRoot required for installed-runtime verification.'}
    $full=[IO.Path]::GetFullPath($Root)
    Assert-Runtime $full
    [void](Read-PythonMetadata $full)
    Assert-InstalledRuntimeManifest $full
    if(-not(Test-VBCablePresent)){Fail 58 'Required VB-CABLE provider is not present.'}
    exit 0
}

function Test-VBCablePresent {
    $pnputil=Join-Path $env:SystemRoot 'System32\pnputil.exe'
    if(Test-Path -LiteralPath $pnputil -PathType Leaf){
        try{if((&$pnputil /enum-drivers 2>&1|Out-String)-match'(?i)vbmmecable|vb-audio.+virtual cable'){return $true}}catch{}
    }
    try{
        $devices=@(Get-CimInstance -ClassName Win32_SoundDevice -ErrorAction Stop)
        if(@($devices|Where-Object{[string]$_.Name-match'(?i)VB-Audio.*(?:CABLE|Virtual Cable)|^CABLE (?:Input|Output)'}).Count-gt 0){return $true}
    }catch{}
    return $false
}

function Install-VBCableIfNeeded([string]$StageRoot){
    if(Test-VBCablePresent){return $false}
    $package=Join-Path $StageRoot 'EngineData\Backend\RuntimeAssets\AudioProvider\VBCABLE\Package'
    $setup=if([Environment]::Is64BitOperatingSystem){Join-Path $package 'VBCABLE_Setup_x64.exe'}else{Join-Path $package 'VBCABLE_Setup.exe'}
    NeedFile $setup 60 'VB-CABLE vendor installer'
    Write-Host '[translateit-r3-installer] Installing required VB-CABLE provider; Windows driver consent may appear.'
    $process=Start-Process -FilePath $setup -ArgumentList @('-i','-h') -Wait -PassThru
    if($process.ExitCode-ne 0){Fail 61 "VB-CABLE installer failed or was declined (exit $($process.ExitCode))."}
    if(-not(Test-VBCablePresent)){Fail 61 'VB-CABLE installer returned success but the driver package is not present.'}
    return $true
}

function Ensure-FreeSpace([string]$Root){
    if($ExpectedExpandedBytes-le 0){return}
    $driveRoot=[IO.Path]::GetPathRoot([IO.Path]::GetFullPath($Root))
    $free=(New-Object IO.DriveInfo($driveRoot)).AvailableFreeSpace
    $required=[Int64]($ExpectedExpandedBytes*1.15)+536870912
    if($free-lt$required){Fail 62 "Insufficient disk space. Need at least $required bytes free."}
}

function Rollback-Payload([string]$Root,[string]$Backup,[System.Collections.IList]$States){
    for($i=$States.Count-1;$i-ge 0;$i--){
        $state=$States[$i]
        $target=Join-Path $Root $state.relative
        $saved=Join-Path $Backup $state.relative
        if($state.new_moved){Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction SilentlyContinue}
        if($state.backup_moved-and(Test-Path -LiteralPath $saved)){
            New-Item -ItemType Directory -Force -Path(Split-Path $target -Parent)|Out-Null
            Move-Item -LiteralPath $saved -Destination $target -Force
        }
    }
}

function Install-Payload([string]$Tar){
    if([string]::IsNullOrWhiteSpace($InstallRoot)){Fail 70 'InstallRoot required.'}
    $root=[IO.Path]::GetFullPath($InstallRoot)
    New-Item -ItemType Directory -Force -Path $root|Out-Null
    Ensure-FreeSpace $root
    $stage=Join-Path $root '.translateit-r3-stage'
    $backup=Join-Path $root '.translateit-r3-backup'
    Remove-Item -LiteralPath $stage,$backup -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $stage,$backup|Out-Null

    &$Tar -xf $script:PayloadPath -C $stage
    if($LASTEXITCODE-ne 0){Fail 71 'Payload extraction failed.'}
    Assert-Runtime $stage
    Assert-Contract (Get-Content -LiteralPath (Join-Path $stage $PayloadContract) -Raw | ConvertFrom-Json)

    # The driver is installed from the verified staging tree before application
    # runtime replacement. If the vendor/system flow fails, the existing app runtime
    # remains untouched. Driver consent is owned by Windows and is never bypassed.
    $restartRequired=Install-VBCableIfNeeded $stage
    $states=New-Object Collections.Generic.List[object]
    try{
        foreach($relative in $PayloadRoots){
            $source=Join-Path $stage $relative
            $target=Join-Path $root $relative
            $saved=Join-Path $backup $relative
            $state=[pscustomobject]@{relative=$relative;backup_moved=$false;new_moved=$false}
            $states.Add($state)
            New-Item -ItemType Directory -Force -Path(Split-Path $target -Parent),(Split-Path $saved -Parent)|Out-Null
            if(Test-Path -LiteralPath $target){Move-Item -LiteralPath $target -Destination $saved -Force;$state.backup_moved=$true}
            Move-Item -LiteralPath $source -Destination $target -Force
            $state.new_moved=$true
        }
        Assert-Runtime $root
        $meta=Read-PythonMetadata $root
        $manifest=[ordered]@{
            schema=$ExpectedInstalledRuntimeSchema
            installed_complete=$true
            app_version=$ExpectedAppVersion
            payload_schema=$ExpectedPayloadSchema
            payload_sha256=$ExpectedSha256.ToLowerInvariant()
            python=$meta.python
            torch=$meta.torch
            transformers=$meta.transformers
            tokenizers=$meta.tokenizers
            translation_model='xiaomi-research/MiLMMT-46-1B-v1.0'
            translation_revision=$MiLMMTRevision
            asr_revision=$AsrRevision
            voice_revision=$GptRevision
            vb_cable_restart_required=$restartRequired
            vb_cable_uninstall_policy='preserve_system_driver'
            user_data_policy='preserve_app_local_user_data'
            installed_utc=[DateTime]::UtcNow.ToString('o')
        }
        $manifestPath=Join-Path $root $InstalledManifest
        New-Item -ItemType Directory -Force -Path(Split-Path $manifestPath -Parent)|Out-Null
        $manifest|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $manifestPath -Encoding UTF8
        Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue
        if($restartRequired){exit 3010}
        exit 0
    }catch{
        [Console]::Error.WriteLine("[translateit-r3-installer] install failed; rolling back application runtime: $($_.Exception.Message)")
        Rollback-Payload $root $backup $states
        exit 80
    }finally{
        Remove-Item -LiteralPath $stage,$backup -Recurse -Force -ErrorAction SilentlyContinue
    }
}

try{
    if($Mode-eq'VerifyInstalled'){Verify-InstalledRuntime $InstallRoot}
    $script:PayloadPath=[IO.Path]::GetFullPath($PayloadPath)
    NeedFile $script:PayloadPath 42 'External payload'
    $actualHash=(Get-FileHash -Algorithm SHA256 -LiteralPath $script:PayloadPath).Hash.ToLowerInvariant()
    if($actualHash-ne$ExpectedSha256.ToLowerInvariant()){Fail 44 'External payload SHA-256 mismatch.'}
    $tar=Get-TarProgram
    $listing=@(&$tar -tf $script:PayloadPath 2>&1)
    if($LASTEXITCODE-ne 0-or$listing.Count-eq 0){Fail 45 'External payload is not a readable 7z archive.'}
    Assert-Contract(Read-PayloadContract $tar $script:PayloadPath)
    if($Mode-eq'Verify'){exit 0}
    Install-Payload $tar
}catch{
    [Console]::Error.WriteLine("[translateit-r3-installer] $($_.Exception.Message)")
    exit 90
}

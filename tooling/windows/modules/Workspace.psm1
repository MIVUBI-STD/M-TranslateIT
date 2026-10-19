# One generated development workspace. No UserData/SavedProject writes.
Set-StrictMode -Version Latest

function Set-TranslateITWorkspaceEnvironment {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    $root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $workspace = Join-Path $root ".workspace"
    foreach ($relative in @(".", "tools", "downloads", "cache", "staging", "temp",
        "reports", "cache/npm", "cache/uv", "cache/pip", "cache/huggingface")) {
        $path = if ($relative -eq ".") { $workspace } else { Join-Path $workspace $relative }
        if (Test-Path -LiteralPath $path) {
            $item = Get-Item -LiteralPath $path -Force
            if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Workspace directory is unsafe or redirected: $path"
            }
        } else {
            New-Item -ItemType Directory -Path $path -Force | Out-Null
        }
    }

    $env:TRANSLATEIT_WORKSPACE_ROOT = $workspace
    $env:TEMP = Join-Path $workspace "temp"
    $env:TMP = $env:TEMP
    $env:npm_config_cache = Join-Path $workspace "cache/npm"
    $env:UV_CACHE_DIR = Join-Path $workspace "cache/uv"
    $env:PIP_CACHE_DIR = Join-Path $workspace "cache/pip"
    $env:HF_HOME = Join-Path $workspace "cache/huggingface"
    $env:HF_HUB_CACHE = Join-Path $env:HF_HOME "hub"
    $env:HF_XET_CACHE = Join-Path $env:HF_HOME "xet"
    $env:UV_PYTHON_INSTALL_DIR = Join-Path $workspace "tools/python"
    $env:UV_TOOL_DIR = Join-Path $workspace "tools/uv"
    $env:UV_TOOL_BIN_DIR = Join-Path $workspace "tools/uv/bin"
    Write-Host "Development workspace: $workspace"
    # Rustup, Cargo, MSVC, WebView2 and Windows driver locations are not yet
    # under a proven relocation contract; do not claim system-drive isolation.
    return $workspace
}

Export-ModuleMember -Function Set-TranslateITWorkspaceEnvironment

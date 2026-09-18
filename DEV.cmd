@echo off
setlocal
cd /d "%~dp0"
if "%~1"=="" (
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tooling\windows-toolchain\dev.ps1" help
) else (
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tooling\windows-toolchain\dev.ps1" %*
)
set EXITCODE=%ERRORLEVEL%
exit /b %EXITCODE%

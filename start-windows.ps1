# ClipForge - start desktop development mode in Windows PowerShell.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw 'Установите Node.js 20+ с nodejs.org' }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Установите Rust stable с rustup.rs и Microsoft C++ Build Tools' }
if (-not (Test-Path 'node_modules')) {
    Write-Host 'Installing JavaScript dependencies...' -ForegroundColor Cyan
    npm install
    if ($LASTEXITCODE -ne 0) { throw 'npm install failed' }
}
Write-Host 'Launching ClipForge (Tauri)...' -ForegroundColor Magenta
npm run dev:desktop
if ($LASTEXITCODE -ne 0) { throw 'Tauri startup failed. See console output.' }

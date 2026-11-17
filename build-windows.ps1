# Compile ClipForge and generate a standalone Windows NSIS installer (.exe).
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Missing Rust/Cargo. Install Rust stable and Visual Studio C++ Build Tools.' }
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { throw 'Missing Node.js/npm.' }
& (Join-Path $PSScriptRoot 'scripts\prepare-ffmpeg.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Failed to prepare FFmpeg' }
if (-not (Test-Path 'node_modules')) { npm install --no-audit --no-fund; if ($LASTEXITCODE -ne 0) { throw 'npm install failed' } }
npm run build:desktop
if ($LASTEXITCODE -ne 0) { throw 'Windows EXE build failed' }
Write-Host 'EXE installer: src-tauri\target\release\bundle\nsis\' -ForegroundColor Green
Write-Host 'App binary: src-tauri\target\release\clipforge.exe' -ForegroundColor Green

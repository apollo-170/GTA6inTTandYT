# Fetch official-style FFmpeg Windows binary redistribution for inclusion in NSIS.
# This script must be run on Windows with network access.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$resources = Join-Path $PWD 'src-tauri\resources'
New-Item -ItemType Directory -Force $resources | Out-Null
$target = Join-Path $resources 'ffmpeg.exe'
if (Test-Path $target) { Write-Host 'Bundled ffmpeg.exe already exists'; exit 0 }
$tempDir = Join-Path $env:TEMP ('clipforge-ffmpeg-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tempDir | Out-Null
try {
  $zipPath = Join-Path $tempDir 'ffmpeg.zip'
  Write-Host 'Downloading FFmpeg essentials binary...'
  Invoke-WebRequest -Uri 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile $zipPath
  Expand-Archive -Path $zipPath -DestinationPath (Join-Path $tempDir 'expanded')
  $ffmpeg = Get-ChildItem (Join-Path $tempDir 'expanded') -Recurse -Filter 'ffmpeg.exe' -File | Select-Object -First 1
  if (-not $ffmpeg) { throw 'FFmpeg executable was not found in the downloaded archive.' }
  Copy-Item $ffmpeg.FullName $target -Force
  Write-Host ('FFmpeg ready: ' + $target)
} finally {
  Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}

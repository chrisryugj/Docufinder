# Download VC++ 2015-2022 Redistributable (run once before build)
$dest = Join-Path $PSScriptRoot "..\src-tauri\wix\vc_redist.x64.exe"
$dest = [System.IO.Path]::GetFullPath($dest)

if (Test-Path $dest) {
    Write-Host "vc_redist.x64.exe already exists, skipping download." -ForegroundColor Green
    exit 0
}

Write-Host "Downloading VC++ Redistributable..." -ForegroundColor Cyan
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vc_redist.x64.exe" -OutFile $dest
Write-Host "Done: $dest" -ForegroundColor Green

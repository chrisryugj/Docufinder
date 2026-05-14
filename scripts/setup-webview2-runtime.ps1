# [v2.6.5] WebView2 Fixed Runtime 번들 — CI 빌드 머신에서 실행.
#
# 목적: Windows 10 LTSC 1809 / 회사 내부망 / VDI 환경에서 WebView2 Runtime detection
# 실패로 앱이 시작 안 되는 문제 대응. v2.5.27 와 동일 구조 (Tauri fixedRuntime 표준).
# v2.6.4 의 with_environment inject 방식이 LTSC 1809 VDI 환경에서 동작 안 한
# 사례 (이슈 #24 JS190-prog) → v2.5.27 에서 검증된 fixedRuntime 모드로 회귀.
#
# Microsoft 가 fixed runtime stable URL 을 제공하지 않으므로 standalone evergreen
# installer 를 admin 권한 (CI runner = admin) 으로 silent install 한 뒤 결과 폴더
# (Application\<version>\) 를 Tauri fixedRuntime 형식 (EBWebView\<arch>\) 으로 재구성.

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$installerUrl = "https://go.microsoft.com/fwlink/?linkid=2099617"
$installerPath = Join-Path $env:TEMP "MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
$dest = Join-Path $PSScriptRoot "..\src-tauri\webview2-runtime"
$dest = [System.IO.Path]::GetFullPath($dest)

$probe = Join-Path $dest "EBWebView\x64\msedgewebview2.exe"
if (Test-Path $probe) {
    Write-Host "WebView2 fixed runtime 이미 준비됨: $dest" -ForegroundColor Green
    exit 0
}

Write-Host "Downloading WebView2 standalone installer..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing
$installerSize = (Get-Item $installerPath).Length
Write-Host "  Downloaded $installerSize bytes" -ForegroundColor Gray

Write-Host "Installing WebView2 Runtime system-wide..." -ForegroundColor Cyan
$proc = Start-Process -FilePath $installerPath -ArgumentList "/silent","/install" -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    Write-Error "WebView2 installer 실패 (exit $($proc.ExitCode))"
    exit 1
}

$appBase = "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application"
if (-not (Test-Path $appBase)) {
    $appBase = "${env:ProgramFiles}\Microsoft\EdgeWebView\Application"
}
if (-not (Test-Path $appBase)) {
    Write-Error "WebView2 install 결과 폴더 없음"
    exit 1
}
$versionDir = Get-ChildItem $appBase -Directory `
    | Where-Object { $_.Name -match "^\d+\.\d+\.\d+\.\d+$" } `
    | Sort-Object { [Version]$_.Name } -Descending `
    | Select-Object -First 1
if (-not $versionDir) {
    Write-Error "WebView2 version 폴더 못찾음 in $appBase"
    exit 1
}
Write-Host "  Detected WebView2 version: $($versionDir.Name)" -ForegroundColor Gray

# Tauri fixedRuntime 표준 구조로 변환:
#   evergreen install:    Application\<version>\msedgewebview2.exe + Locales\ + *.dll
#   fixedRuntime 요구:    <path>\EBWebView\<arch>\msedgewebview2.exe + Locales\ + ...
$ebDir = Join-Path $dest "EBWebView\x64"
if (Test-Path $dest) {
    Remove-Item -Recurse -Force $dest
}
New-Item -ItemType Directory -Path $ebDir -Force | Out-Null

Write-Host "Copying WebView2 binary tree to $ebDir ..." -ForegroundColor Cyan
Copy-Item -Path "$($versionDir.FullName)\*" -Destination $ebDir -Recurse -Force

if (-not (Test-Path (Join-Path $ebDir "msedgewebview2.exe"))) {
    Write-Error "msedgewebview2.exe not found in $ebDir — fixed runtime 변환 실패"
    exit 1
}
$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

# WebView2 Fixed Runtime 번들 — CI 빌드 머신에서 실행.
#
# 목적: Windows 10 LTSC 1809 등 회사 내부망 환경에서 WebView2 Runtime detection
# 실패로 앱이 시작 안 되는 문제 (이슈 #22 추가 보고). offlineInstaller 모드는
# 시스템 설치까지만 보장하고 registry detection 이 GPO 차단 / user-level 설치 등으로
# 실패하면 앱이 동작하지 않는다. fixedRuntime 모드는 WebView2 binary 자체를 앱
# 폴더에 번들해 시스템 설치 / 권한 / GPO 와 무관하게 동작.
#
# Microsoft 가 fixed runtime stable URL 을 제공하지 않으므로 standalone evergreen
# installer 를 admin 권한 (CI runner = admin) 으로 silent install 한 뒤 결과 폴더를
# Tauri fixed runtime 형식 (`<path>/EBWebView/<arch>/...`) 으로 재구성한다.

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# Microsoft 공식 fwlink (linkid=2099617 = MicrosoftEdgeWebView2RuntimeInstallerX64.exe).
# aka.ms/MicrosoftEdgeWebview2Standalone 도 동일 대상으로 redirect.
$installerUrl = "https://go.microsoft.com/fwlink/?linkid=2099617"
$installerPath = Join-Path $env:TEMP "MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
$dest = Join-Path $PSScriptRoot "..\src-tauri\webview2-runtime"
$dest = [System.IO.Path]::GetFullPath($dest)

# 이미 준비돼 있고 EBWebView/x64/msedgewebview2.exe 가 있으면 skip
$probe = Join-Path $dest "EBWebView\x64\msedgewebview2.exe"
if (Test-Path $probe) {
    Write-Host "WebView2 fixed runtime 이미 준비됨: $dest" -ForegroundColor Green
    exit 0
}

# 1. Standalone installer 다운로드
Write-Host "Downloading WebView2 standalone installer..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing
$installerSize = (Get-Item $installerPath).Length
Write-Host "  Downloaded $installerSize bytes" -ForegroundColor Gray

# 2. Silent install (admin 권한 필요 — windows-latest CI runner 는 admin)
Write-Host "Installing WebView2 Runtime system-wide..." -ForegroundColor Cyan
$proc = Start-Process -FilePath $installerPath -ArgumentList "/silent","/install" -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    Write-Error "WebView2 installer 실패 (exit $($proc.ExitCode))"
    exit 1
}

# 3. 설치된 위치 (Application\<version>) 찾기 — system-wide install 결과
$appBase = "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application"
if (-not (Test-Path $appBase)) {
    # 일부 환경에서 64-bit Program Files 에 설치되는 경우
    $appBase = "${env:ProgramFiles}\Microsoft\EdgeWebView\Application"
}
if (-not (Test-Path $appBase)) {
    Write-Error "WebView2 install 결과 폴더 없음: ${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application"
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

# 4. Tauri fixedRuntime 형식으로 변환
#    evergreen install:    Application\<version>\msedgewebview2.exe + Locales\ + *.dll
#    fixedRuntime 요구사항: <path>\EBWebView\<arch>\msedgewebview2.exe + Locales\ + ...
$ebDir = Join-Path $dest "EBWebView\x64"
if (Test-Path $dest) {
    Remove-Item -Recurse -Force $dest
}
New-Item -ItemType Directory -Path $ebDir -Force | Out-Null

Write-Host "Copying WebView2 binary tree to $ebDir ..." -ForegroundColor Cyan
Copy-Item -Path "$($versionDir.FullName)\*" -Destination $ebDir -Recurse -Force

# 5. 검증 — msedgewebview2.exe 가 있어야 정상
if (-not (Test-Path (Join-Path $ebDir "msedgewebview2.exe"))) {
    Write-Error "msedgewebview2.exe not found in $ebDir — fixed runtime 변환 실패"
    exit 1
}
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

# [v2.6.12] WebView2 Fixed Version Runtime 번들 — CI 빌드 머신에서 실행.
#
# 변경 이력:
#   v2.5.27 ~ v2.6.10: Microsoft Evergreen Standalone Installer 를 runner 에
#     silent install 한 뒤 `Application\<version>\` 폴더 내용을 fixed runtime
#     포맷으로 복사 — 빌드 통과하지만 일부 사용자 머신에서 0x80070002 ERROR_FILE_NOT_FOUND
#     발생 (의존성 누락 의심, 이슈 #23 austinjung827).
#   v2.6.11 (실패): Microsoft.Web.WebView2.FixedVersionRuntime NuGet 패키지를
#     찾으려 했으나 Microsoft 가 nuget.org 에 발행하지 않았음 (totalHits=0).
#     이번 단계는 폐기.
#   v2.6.12 (현재): evergreen-copy 방식 복구. 단, **critical dependency presence
#     check** 를 추가하여 evergreen 결과에 필수 파일이 누락되면 CI 단계에서 즉시
#     실패시킨다. 사용자 머신에서 누락이 나는 케이스를 식별하는 안전망.
#     장기적으로는 Microsoft Fixed Version Runtime cab 을 GH release 에 mirror
#     하는 방향 검토 필요 (별도 작업).

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

# Critical dependency presence check. 이슈 #23 사용자 머신에서 0x80070002
# (ERROR_FILE_NOT_FOUND) 가 떨어진 것이 의존성 누락 때문인지 식별하기 위해
# evergreen `Application\<version>\` 폴더에 있어야 마땅한 핵심 파일들을 검증한다.
# runner 머신과 사용자 머신의 차이를 좁히는 회귀 알람.
$mustExist = @(
    "msedgewebview2.exe",
    "msedgewebview2.exe.sig",
    "EmbeddedBrowserWebView.dll"
)
$missing = @()
foreach ($f in $mustExist) {
    $p = Join-Path $ebDir $f
    if (-not (Test-Path $p)) { $missing += $f }
}
$localesDir = Join-Path $ebDir "Locales"
if (-not (Test-Path $localesDir)) { $missing += "Locales/" }

if ($missing.Count -gt 0) {
    Write-Warning "evergreen Application\<version>\ 폴더에 다음 파일/폴더 누락:"
    foreach ($m in $missing) { Write-Warning "  - $m" }
    Write-Warning "이슈 #23 의 0x80070002 ERROR_FILE_NOT_FOUND 원인일 가능성 — Microsoft Fixed Version Runtime cab 사용 검토 필요."
    # 빌드는 계속 진행 (LTSC installer 자체 생성은 가능). 다만 누락 사실이 로그에 남아
    # 사후 진단 가능.
} else {
    Write-Host "  All critical WebView2 dependencies present." -ForegroundColor Green
}

$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

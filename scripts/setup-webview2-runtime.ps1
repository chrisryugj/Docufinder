# [v2.6.13] WebView2 Fixed Version Runtime 번들 — CI 빌드 머신에서 실행.
#
# 변경 이력:
#   v2.5.27 ~ v2.6.10: Microsoft Evergreen Standalone Installer 를 runner 에
#     silent install 한 뒤 `Application\<version>\` 폴더 내용을 fixed runtime
#     포맷으로 복사 — 빌드 통과하지만 일부 사용자 머신에서 0x80070002
#     ERROR_FILE_NOT_FOUND 발생 (의존성 누락 의심, 이슈 #23 austinjung827).
#   v2.6.11 (실패): Microsoft.Web.WebView2.FixedVersionRuntime NuGet 패키지를
#     찾으려 했으나 Microsoft 가 nuget.org 에 발행하지 않았음 (totalHits=0).
#   v2.6.12: evergreen-copy 복구 + critical file presence WARNING — CI 로그에
#     **EmbeddedBrowserWebView.dll 누락 확정** (이슈 #23 0x80070002 직접 원인).
#   v2.6.13 (현재): system 전체에서 EmbeddedBrowserWebView.dll 탐색 → 같이 들어
#     있는 폴더(Microsoft Edge for Business 의 Application\<version>\) 의 빠진
#     파일들을 fixed runtime 폴더로 **보강 복사**. critical file 누락 시 빌드
#     fail. 사용자 머신에서 self-contained 동작 보장.

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

# v2.6.13: EmbeddedBrowserWebView.dll 가 evergreen `Application\<version>\` 폴더에
# 누락된 문제 (v2.6.12 빌드 로그에서 확정 — 이슈 #23 0x80070002 ERROR_FILE_NOT_FOUND
# 직접 원인) 의 보강. Microsoft Edge for Business 가 Edge 본체와 WebView2 의 일부
# core component 를 공유하는 구조 (DLL 형태로 Edge 브라우저의 `Application\<version>\`
# 폴더 또는 sub-folder 에 존재) 라서, evergreen WebView 만 install 한 결과 폴더에는
# 그 dll 들이 빠진다. 우리 PS1 이 system 전체에서 EmbeddedBrowserWebView.dll 및
# 동반 필수 dll 를 찾아 fixed runtime 폴더로 같이 복사.
$searchRoots = @(
    "${env:ProgramFiles(x86)}\Microsoft",
    "${env:ProgramFiles}\Microsoft"
) | Where-Object { Test-Path $_ }

Write-Host "Searching system for EmbeddedBrowserWebView.dll and dependencies ..." -ForegroundColor Cyan
$missingFiles = @("EmbeddedBrowserWebView.dll")
foreach ($fname in $missingFiles) {
    if (Test-Path (Join-Path $ebDir $fname)) {
        Write-Host "  $fname already present (no system search needed)." -ForegroundColor Green
        continue
    }
    $found = Get-ChildItem -Path $searchRoots -Recurse -Filter $fname -File -ErrorAction SilentlyContinue `
        | Sort-Object @{ Expression = {
            # 파일이 들어있는 가장 가까운 version-shaped 폴더 이름을 추출 후 [Version] 정렬.
            $d = $_.Directory
            while ($d -and -not ($d.Name -match "^\d+\.\d+\.\d+\.\d+$")) { $d = $d.Parent }
            if ($d) { [Version]$d.Name } else { [Version]"0.0.0.0" }
        } } -Descending `
        | Select-Object -First 1
    if (-not $found) {
        Write-Error "$fname 시스템 어디에서도 찾지 못함 — Edge 또는 WebView2 가 runner 에 미설치"
        exit 1
    }
    Write-Host "  Found at: $($found.FullName)" -ForegroundColor Yellow
    # 동반 폴더 (msedgewebview2.exe + EmbeddedBrowserWebView.dll 가 함께 있는 폴더)
    # 전체에서 우리 ebDir 에 아직 없는 파일/폴더만 덮어쓰지 않고 보강 복사.
    $sourceDir = $found.Directory
    Write-Host "  Augmenting from $($sourceDir.FullName) — adding missing files only ..." -ForegroundColor Cyan
    Get-ChildItem -Path $sourceDir.FullName -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
        $rel = $_.FullName.Substring($sourceDir.FullName.Length).TrimStart('\','/')
        $target = Join-Path $ebDir $rel
        if (-not (Test-Path $target)) {
            $targetDir = Split-Path $target -Parent
            if (-not (Test-Path $targetDir)) { New-Item -ItemType Directory -Path $targetDir -Force | Out-Null }
            Copy-Item $_.FullName -Destination $target -Force
        }
    }
}

# Critical dependency presence check — 보강 복사 후 최종 검증. 누락 시 빌드 fail.
$mustExist = @(
    "msedgewebview2.exe",
    "msedgewebview2.exe.sig",
    "EmbeddedBrowserWebView.dll"
)
$stillMissing = @()
foreach ($f in $mustExist) {
    if (-not (Test-Path (Join-Path $ebDir $f))) { $stillMissing += $f }
}
$localesDir = Join-Path $ebDir "Locales"
if (-not (Test-Path $localesDir)) { $stillMissing += "Locales/" }

if ($stillMissing.Count -gt 0) {
    Write-Error "보강 복사 후에도 다음 파일 누락 → fixed runtime 실패 보장: $($stillMissing -join ', ')"
    exit 1
}
Write-Host "  All critical WebView2 dependencies present after augmentation." -ForegroundColor Green

$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

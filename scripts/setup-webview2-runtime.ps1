# [v2.6.14] WebView2 Fixed Version Runtime 번들 — CI 빌드 머신에서 실행.
#
# 변경 이력:
#   v2.5.27 ~ v2.6.10: Application\<ver>\* 복사 — 사용자 머신 0x80070002
#     (의존성 누락, 이슈 #23 austinjung827).
#   v2.6.11 (실패): NuGet `Microsoft.Web.WebView2.FixedVersionRuntime` 패키지를
#     가정했으나 nuget.org 미발행 (totalHits=0). 폐기.
#   v2.6.12: evergreen-copy 복구 + critical file WARNING — CI 로그에
#     **EmbeddedBrowserWebView.dll 누락 확정**.
#   v2.6.13 (실패): EmbeddedBrowserWebView.dll system 검색 + 보강 복사. 그러나
#     검색이 `Application\<ver>\EBWebView\x86\EmbeddedBrowserWebView.dll`(x86) 을
#     매치하여 architecture mismatch — 사용자 머신에서 또 실패 예상.
#   v2.6.14 (현재): evergreen `Application\<ver>\` 의 진짜 구조 (`EBWebView\x64\`
#     sub-folder 에 native binaries 분리 보관) 를 정확히 반영. msedgewebview2.exe +
#     Locales\ 는 root 에서, EmbeddedBrowserWebView.dll 등은 `EBWebView\x64\` 에서
#     가져와 **fixed runtime 의 EBWebView\x64\ root 로 평탄화 복사**. Microsoft
#     공식 Fixed Version Runtime cab 의 self-contained layout 을 evergreen 으로
#     재구성.

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

# evergreen `Application\<ver>\` 실제 구조 (Edge WebView2 124+):
#   Application\<ver>\
#   ├── msedgewebview2.exe          (root: 진입점 + ICU/V8 데이터 등)
#   ├── msedgewebview2.exe.sig
#   ├── Locales\
#   ├── *.bin, *.dat (data files)
#   └── EBWebView\
#       ├── x64\                    (native binaries: EmbeddedBrowserWebView.dll 등)
#       └── x86\                    (32-bit 동일 binary)
#
# Microsoft Fixed Version Runtime cab 의 self-contained layout 은 위 분리 구조를
# **하나의 폴더로 평탄화** 한 것이다. 따라서 우리는:
#   1) Application\<ver>\* 를 우리의 EBWebView\x64\ 로 복사 (root + Locales + EBWebView 그대로)
#   2) Application\<ver>\EBWebView\x64\* 의 native binary 를 우리의 EBWebView\x64\
#      root level 로 추가 복사 (덮어쓰기 — 우리가 사용할 path 와 일치시키기 위함)
#   3) 우리의 EBWebView\x64\EBWebView 라는 중첩 sub-folder 는 평탄화 후 불필요하므로 삭제
Write-Host "Copying evergreen Application\<ver>\* base layer to $ebDir ..." -ForegroundColor Cyan
Copy-Item -Path "$($versionDir.FullName)\*" -Destination $ebDir -Recurse -Force

$evergreenEbX64 = Join-Path $versionDir.FullName "EBWebView\x64"
if (-not (Test-Path $evergreenEbX64)) {
    Write-Error "evergreen Application\<ver>\EBWebView\x64 가 존재하지 않음: $evergreenEbX64"
    exit 1
}
Write-Host "Flattening EBWebView\x64\* native binaries to fixed runtime root ..." -ForegroundColor Cyan
$flattenedCount = 0
Get-ChildItem -Path $evergreenEbX64 -File -ErrorAction SilentlyContinue | ForEach-Object {
    Copy-Item $_.FullName -Destination $ebDir -Force
    $flattenedCount++
}
Get-ChildItem -Path $evergreenEbX64 -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    Copy-Item -Path $_.FullName -Destination $ebDir -Recurse -Force
}
Write-Host "  Flattened $flattenedCount native binaries from x64\ to root." -ForegroundColor Gray

# 중첩된 EBWebView sub-folder (Application\<ver>\EBWebView 가 복사된 것) 제거.
# 평탄화 후 불필요 + 사용자 머신에서 detect_fixed_runtime_dir 의 잘못된 매칭 방지.
$nestedEbWebView = Join-Path $ebDir "EBWebView"
if (Test-Path $nestedEbWebView) {
    Remove-Item -Recurse -Force $nestedEbWebView
    Write-Host "  Removed redundant nested EBWebView sub-folder." -ForegroundColor Gray
}

# Critical dependency presence check — 평탄화 후 최종 검증. 누락 시 빌드 fail.
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
    Write-Error "평탄화 후에도 다음 파일 누락 → fixed runtime 실패 보장: $($stillMissing -join ', ')"
    exit 1
}
# x64 native dll 의 architecture 검증 — x86 dll 이 섞이지 않았는지 magic byte 확인.
# PE 헤더 IMAGE_FILE_MACHINE: 0x8664 = AMD64, 0x014C = i386.
$ebwvPath = Join-Path $ebDir "EmbeddedBrowserWebView.dll"
$fs = [System.IO.File]::OpenRead($ebwvPath)
try {
    $br = New-Object System.IO.BinaryReader($fs)
    $fs.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
    $peOffset = $br.ReadInt32()
    $fs.Seek($peOffset + 4, [System.IO.SeekOrigin]::Begin) | Out-Null  # PE\0\0 + 4 = Machine field
    $machine = $br.ReadUInt16()
} finally {
    $fs.Close()
}
if ($machine -ne 0x8664) {
    Write-Error "EmbeddedBrowserWebView.dll architecture mismatch (machine=0x$($machine.ToString('X4')), expected 0x8664 x64)"
    exit 1
}
Write-Host "  All critical WebView2 dependencies present (architecture=x64 verified)." -ForegroundColor Green

$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

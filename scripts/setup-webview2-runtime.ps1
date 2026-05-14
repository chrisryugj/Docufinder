# [v2.6.4] CI runner 의 사전 설치된 Microsoft Edge 의 EBWebView 폴더를
# src-tauri/resources/EBWebView/ 로 복사한다.
#
# 용도: tauri.windows-ltsc.conf.json (override config) 로 LTSC 1809 / admin
# 권한 없는 환경 전용 installer 빌드 시 EBWebView 를 NSIS resources 에 포함.
# 사용자가 zip 풀거나 standalone installer 권한 부여 같은 수동 단계 없이
# installer 한 번이면 끝.
#
# 왜 webviewInstallMode:fixedRuntime 안 쓰는가:
# v2.5.27 에서 fixedRuntime 모드를 시도했지만 Tauri 의 NSIS template 이 그 모드
# 처리 시 EBWebView 폴더를 installer 에 누락시키는 회귀 발생 (이슈 #24 v2.5.27).
# bundle.resources 일반 파일 경로는 기존 kordoc / vcredist 가 안정적으로 처리
# 되므로 회귀 위험 없음. webview2_runtime.rs 가 `<exe_dir>/*/EBWebView` 를 detect
# 해 우리가 직접 만든 ICoreWebView2Environment 를 wry 에 inject.

$ErrorActionPreference = "Stop"

$base = "C:\Program Files (x86)\Microsoft\EdgeWebView\Application"
if (-not (Test-Path $base)) {
    Write-Error "Microsoft EdgeWebView not pre-installed at $base"
    exit 1
}

$versionDir = Get-ChildItem $base -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
    Sort-Object Name -Descending | Select-Object -First 1

if (-not $versionDir) {
    Write-Error "No EdgeWebView version directory found under $base"
    exit 1
}

$srcEBWebView = Join-Path $versionDir.FullName "EBWebView"
if (-not (Test-Path $srcEBWebView)) {
    Write-Error "EBWebView subdirectory missing in $($versionDir.FullName)"
    exit 1
}

$dstEBWebView = "src-tauri/resources/EBWebView"
if (Test-Path $dstEBWebView) {
    Remove-Item -Recurse -Force $dstEBWebView
}
New-Item -ItemType Directory -Force -Path $dstEBWebView | Out-Null

Write-Host "Copying WebView2 Runtime $($versionDir.Name)"
Write-Host "  src: $srcEBWebView"
Write-Host "  dst: $dstEBWebView"
Copy-Item -Path "$srcEBWebView/*" -Destination $dstEBWebView -Recurse -Force

$files = (Get-ChildItem $dstEBWebView -Recurse -File | Measure-Object).Count
$bytes = (Get-ChildItem $dstEBWebView -Recurse -File | Measure-Object -Property Length -Sum).Sum
$mb = [math]::Round($bytes / 1MB, 1)
Write-Host "EBWebView staged for LTSC build: $files files, $mb MB"

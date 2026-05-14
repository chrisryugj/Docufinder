# [v2.6.4] CI runner 의 사전 설치된 Microsoft Edge 의 WebView2 runtime 폴더를
# src-tauri/resources/webview2-runtime/ 로 복사한다.
#
# 핵심: WebView2 runtime 의 본체 (msedgewebview2.exe + msedge.dll + EBWebView.dll +
# locales 등) 는 `Application/<version>/` 폴더에 직접 있고, sub folder `EBWebView/`
# 는 거의 비어있음 (extension/cache 용). 따라서 `<version>` 폴더 전체를 복사한다.
#
# 그 결과는 `<install_dir>/resources/webview2-runtime/<files...>` 로 풀리고,
# Microsoft 의 `CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder=...)`
# API 는 msedgewebview2.exe 가 직접 들어있는 폴더 path 를 요구하므로 우리
# webview2_runtime.rs 가 `<install_dir>/resources/webview2-runtime/` 를 그대로 사용.

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

$srcExe = Join-Path $versionDir.FullName "msedgewebview2.exe"
if (-not (Test-Path $srcExe)) {
    Write-Error "msedgewebview2.exe missing in $($versionDir.FullName)"
    exit 1
}

$dstDir = "src-tauri/resources/webview2-runtime"
if (Test-Path $dstDir) {
    Remove-Item -Recurse -Force $dstDir
}
New-Item -ItemType Directory -Force -Path $dstDir | Out-Null

Write-Host "Copying WebView2 Runtime $($versionDir.Name)"
Write-Host "  src: $($versionDir.FullName)"
Write-Host "  dst: $dstDir"
Copy-Item -Path "$($versionDir.FullName)/*" -Destination $dstDir -Recurse -Force

$dstExe = Join-Path $dstDir "msedgewebview2.exe"
if (-not (Test-Path $dstExe)) {
    Write-Error "msedgewebview2.exe missing after copy at $dstExe"
    exit 1
}

$files = (Get-ChildItem $dstDir -Recurse -File | Measure-Object).Count
$bytes = (Get-ChildItem $dstDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$mb = [math]::Round($bytes / 1MB, 1)
Write-Host "WebView2 runtime staged for LTSC build: $files files, $mb MB"

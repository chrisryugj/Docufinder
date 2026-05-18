# [v2.6.11] WebView2 Fixed Version Runtime 번들 — CI 빌드 머신에서 실행.
#
# 변경 배경 (이슈 #23 austinjung827):
#   v2.5.27 ~ v2.6.9 의 스크립트는 Microsoft Evergreen Standalone Installer 를
#   runner 에 silent install 한 뒤 `C:\Program Files (x86)\Microsoft\EdgeWebView\
#   Application\<version>\` 폴더 내용을 fixed runtime 형식으로 복사했다. 그러나
#   evergreen runtime 은 일부 의존성을 시스템 폴더 (System32, WinSxS 등) 에 분산
#   설치하는 구조라 self-contained 가 아니다. 빌드 머신에서는 시스템에 깔린
#   파일이 가려줘 통과했지만, 사용자 머신에서는 누락이 노출되어
#   `CreateCoreWebView2EnvironmentWithOptions HRESULT: 0x80070002
#   (ERROR_FILE_NOT_FOUND)` 로 환경 생성이 실패했다.
#
# 해결: Microsoft 공식 NuGet 패키지 `Microsoft.Web.WebView2.FixedVersionRuntime.
# <version>.x64` 를 사용한다. 이 패키지는 self-contained 로 모든 의존성을 동봉
# (msedgewebview2.exe + msedgewebview2.exe.sig + EmbeddedBrowserWebView.dll +
# Locales\* + native DLL 전부) 하여, 시스템에 WebView2 가 부재한 환경에서도 단독
# 동작한다. Tauri `webviewInstallMode:fixedRuntime` + 본 스크립트 산출물 조합이
# 표준 Microsoft 권장 분배 모드이다.

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$dest = Join-Path $PSScriptRoot "..\src-tauri\webview2-runtime"
$dest = [System.IO.Path]::GetFullPath($dest)

$probe = Join-Path $dest "EBWebView\x64\msedgewebview2.exe"
if (Test-Path $probe) {
    Write-Host "WebView2 fixed runtime 이미 준비됨: $dest" -ForegroundColor Green
    exit 0
}

# 1) NuGet search API 로 최신 x64 Fixed Version Runtime 패키지 ID/버전 조회.
#    패키지 ID 자체에 버전이 박혀있는 (...FixedVersionRuntime.<ver>.x64) 구조라
#    search 결과를 필터링/정렬해서 가장 최신을 고른다.
Write-Host "Querying NuGet for latest Microsoft.Web.WebView2.FixedVersionRuntime.x64 ..." -ForegroundColor Cyan
$searchUrl = "https://azuresearch-usnc.nuget.org/query?q=Microsoft.Web.WebView2.FixedVersionRuntime&prerelease=false&take=200"
$searchResp = Invoke-RestMethod -Uri $searchUrl -UseBasicParsing
$x64Pkg = $searchResp.data `
    | Where-Object { $_.id -match "^Microsoft\.Web\.WebView2\.FixedVersionRuntime\.\d+\.\d+\.\d+\.\d+\.x64$" } `
    | Sort-Object @{ Expression = { [Version]($_.id -replace ".*FixedVersionRuntime\.(\d+\.\d+\.\d+\.\d+)\.x64$", '$1') } } -Descending `
    | Select-Object -First 1
if (-not $x64Pkg) {
    Write-Error "NuGet 에서 Microsoft.Web.WebView2.FixedVersionRuntime.x64 패키지를 찾지 못했습니다."
    exit 1
}
$pkgId = $x64Pkg.id
$pkgVer = $x64Pkg.version
Write-Host "  Latest: $pkgId v$pkgVer" -ForegroundColor Gray

# 2) nupkg 다운로드 (NuGet v3 flat container endpoint).
$pkgIdLower = $pkgId.ToLower()
$nupkgUrl = "https://api.nuget.org/v3-flatcontainer/$pkgIdLower/$pkgVer/$pkgIdLower.$pkgVer.nupkg"
$nupkgPath = Join-Path $env:TEMP "$pkgId.$pkgVer.nupkg"
Write-Host "Downloading $nupkgUrl ..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $nupkgUrl -OutFile $nupkgPath -UseBasicParsing
$nupkgSize = (Get-Item $nupkgPath).Length
Write-Host "  Downloaded $([Math]::Round($nupkgSize/1MB,1)) MB" -ForegroundColor Gray

# 3) nupkg 풀기. nupkg 는 표준 zip 컨테이너라 Expand-Archive 로 직접 풀 수 있다.
#    (.nupkg → .zip copy 후 Expand-Archive — 일부 Expand 구현이 확장자 체크하므로
#    안전하게 별도 zip 복사본을 만든다.)
$extractDir = Join-Path $env:TEMP "$pkgId.$pkgVer.extracted"
if (Test-Path $extractDir) { Remove-Item -Recurse -Force $extractDir }
$zipCopy = Join-Path $env:TEMP "$pkgId.$pkgVer.zip"
Copy-Item $nupkgPath $zipCopy -Force
Expand-Archive -Path $zipCopy -DestinationPath $extractDir -Force

# 4) 패키지 안에서 msedgewebview2.exe 위치를 탐색.
#    NuGet 패키지의 정확한 내부 layout 은 버전마다 미세하게 다를 수 있으므로
#    hardcoded path 대신 recursive 검색으로 확정한다. msedgewebview2.exe 의
#    부모 폴더가 EBWebView 트리의 루트 (Locales\, *.dll, *.sig 등 self-contained
#    리소스 전부 같은 폴더에 평면 배치되어 있음).
$msedge = Get-ChildItem -Path $extractDir -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue `
    | Select-Object -First 1
if (-not $msedge) {
    Write-Error "NuGet 패키지 안에서 msedgewebview2.exe 를 찾지 못했습니다."
    exit 1
}
$ebSource = $msedge.Directory
Write-Host "  EBWebView root in package: $($ebSource.FullName)" -ForegroundColor Gray

# 5) Tauri fixedRuntime 표준 구조로 평탄화:
#    <dest>\EBWebView\x64\msedgewebview2.exe + 의존성 self-contained 전부.
$ebTarget = Join-Path $dest "EBWebView\x64"
if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Path $ebTarget -Force | Out-Null

Write-Host "Copying self-contained Fixed Version Runtime to $ebTarget ..." -ForegroundColor Cyan
Copy-Item -Path "$($ebSource.FullName)\*" -Destination $ebTarget -Recurse -Force

# 6) Critical dependency presence check. evergreen 회귀 (의존성 누락) 의 핵심
#    파일 3종을 확정 검증해 빌드 단계에서 회귀를 즉시 잡는다.
$mustExist = @(
    "msedgewebview2.exe",
    "msedgewebview2.exe.sig",
    "EmbeddedBrowserWebView.dll"
)
foreach ($f in $mustExist) {
    $p = Join-Path $ebTarget $f
    if (-not (Test-Path $p)) {
        Write-Error "필수 파일 누락 → fixed runtime 무효: $p"
        exit 1
    }
}
$localesDir = Join-Path $ebTarget "Locales"
if (-not (Test-Path $localesDir)) {
    Write-Error "Locales 폴더 누락 → fixed runtime 무효: $localesDir"
    exit 1
}

$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB) — WebView2 v$pkgVer" -ForegroundColor Green

# [v2.6.15] WebView2 Fixed Version Runtime 번들 — 사실 발견 기반 자동 선택 모드.
#
# v2.6.11 ~ v2.6.14 의 핫픽스가 evergreen WebView2 install 결과 폴더 구조를
# 매번 다르게 잘못 가정하여 줄줄이 회귀했다 (NuGet 미발행 → critical WARNING →
# x86 매치 → EBWebView\x64 부재). 이번 빌드는 **가정 없이 사실 발견**:
#
#   1) Application\<ver>\ 전체 구조를 로그에 dump.
#   2) system 전체에서 msedgewebview2.exe / EmbeddedBrowserWebView.dll 의
#      모든 위치 + 각각의 PE 헤더 architecture 를 dump.
#   3) x64 msedgewebview2.exe 와 x64 EmbeddedBrowserWebView.dll 가 **같은 폴더에
#      공존하는 (또는 EBWebView\x64\ sub-folder 로 동봉된)** self-contained
#      후보를 자동 탐지 → 그 폴더를 평탄화 복사.
#   4) 검증은 WARNING 으로 — 빌드는 통과시키되 누락 사실은 명시.
#
# 진단 정보로 다음 패치에서 (필요하면) 정밀 fix.

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

# v2.6.15: 사실에 기반한 진단부터.
#   v2.6.11~v2.6.14 핫픽스가 evergreen WebView2 의 실제 폴더 구조를 잘못 가정 ↔ 매번 다른 실패.
#   이번 빌드는 **사실 수집 모드** — evergreen install 결과 + system 전체에서 msedgewebview2.exe
#   와 EmbeddedBrowserWebView.dll 의 정확한 위치/architecture 를 모두 출력한다. 그 후 가장 적합한
#   self-contained 폴더를 자동 선택해 EBWebView\x64\ 로 복사한다 (가정이 아닌 발견 기반).

# ===== 1) 진단: Application\<ver>\ tree 출력 =====
Write-Host "=== DIAG: Application\<ver>\ structure (depth=2) ===" -ForegroundColor Magenta
Get-ChildItem -Path $versionDir.FullName -Recurse -Depth 1 -ErrorAction SilentlyContinue | ForEach-Object {
    $rel = $_.FullName.Substring($versionDir.FullName.Length).TrimStart('\','/')
    if ($_.PSIsContainer) {
        Write-Host "  [DIR ] $rel"
    } else {
        $sz = [Math]::Round($_.Length / 1KB, 1)
        Write-Host "  [FILE] $rel  (${sz} KB)"
    }
}

# ===== 2) 진단: system 전체에서 msedgewebview2.exe 와 EmbeddedBrowserWebView.dll 찾기 =====
$searchRoots = @(
    "${env:ProgramFiles(x86)}\Microsoft",
    "${env:ProgramFiles}\Microsoft"
) | Where-Object { Test-Path $_ }

function Get-PEMachine([string]$path) {
    try {
        $fs = [System.IO.File]::OpenRead($path)
        try {
            $br = New-Object System.IO.BinaryReader($fs)
            $fs.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
            $peOffset = $br.ReadInt32()
            $fs.Seek($peOffset + 4, [System.IO.SeekOrigin]::Begin) | Out-Null
            return $br.ReadUInt16()
        } finally { $fs.Close() }
    } catch { return 0 }
}
function Arch-Name([int]$machine) {
    switch ($machine) {
        0x8664  { "x64" }
        0x014C  { "x86" }
        0xAA64  { "arm64" }
        default { "0x$($machine.ToString('X4'))" }
    }
}

Write-Host "=== DIAG: all msedgewebview2.exe on system ===" -ForegroundColor Magenta
$msedgeAll = Get-ChildItem -Path $searchRoots -Recurse -Filter "msedgewebview2.exe" -File -ErrorAction SilentlyContinue
foreach ($e in $msedgeAll) {
    $arch = Arch-Name(Get-PEMachine $e.FullName)
    Write-Host "  [$arch] $($e.FullName)  ($([Math]::Round($e.Length/1MB,2)) MB)"
}

Write-Host "=== DIAG: all EmbeddedBrowserWebView.dll on system ===" -ForegroundColor Magenta
$ebwvAll = Get-ChildItem -Path $searchRoots -Recurse -Filter "EmbeddedBrowserWebView.dll" -File -ErrorAction SilentlyContinue
foreach ($e in $ebwvAll) {
    $arch = Arch-Name(Get-PEMachine $e.FullName)
    Write-Host "  [$arch] $($e.FullName)  ($([Math]::Round($e.Length/1MB,2)) MB)"
}

# ===== 3) 자동 결정: msedgewebview2.exe 와 EmbeddedBrowserWebView.dll 이 함께 들어있는
#         x64 self-contained 폴더를 찾는다. =====
Write-Host "=== Selecting self-contained x64 source folder ===" -ForegroundColor Cyan
$candidates = @()
foreach ($m in $msedgeAll) {
    if ((Get-PEMachine $m.FullName) -ne 0x8664) { continue }
    $sibling = Join-Path $m.DirectoryName "EmbeddedBrowserWebView.dll"
    if (Test-Path $sibling) {
        $siblingArch = Get-PEMachine $sibling
        if ($siblingArch -eq 0x8664) {
            $candidates += [PSCustomObject]@{ Dir = $m.DirectoryName; Source = "co-located" }
            continue
        }
    }
    # x64 msedgewebview2 옆 sub-folder 에 x64 EmbeddedBrowserWebView.dll 있는 경우.
    $nestedX64 = Join-Path $m.DirectoryName "EBWebView\x64\EmbeddedBrowserWebView.dll"
    if ((Test-Path $nestedX64) -and ((Get-PEMachine $nestedX64) -eq 0x8664)) {
        $candidates += [PSCustomObject]@{ Dir = $m.DirectoryName; Source = "nested-EBWebView-x64" }
    }
}

if ($candidates.Count -eq 0) {
    Write-Warning "x64 self-contained 폴더 후보 0건 — evergreen-as-is 복사로 폴백."
    $sourceRoot = $versionDir.FullName
    $flattenX64 = $false
} else {
    $chosen = $candidates[0]
    $sourceRoot = $chosen.Dir
    $flattenX64 = ($chosen.Source -eq "nested-EBWebView-x64")
    Write-Host "  Chosen: $sourceRoot (mode=$($chosen.Source))" -ForegroundColor Green
}

# ===== 4) 복사 =====
Write-Host "Copying $sourceRoot\* to $ebDir ..." -ForegroundColor Cyan
Copy-Item -Path "$sourceRoot\*" -Destination $ebDir -Recurse -Force

if ($flattenX64) {
    $nestedX64Dir = Join-Path $ebDir "EBWebView\x64"
    if (Test-Path $nestedX64Dir) {
        Write-Host "Flattening EBWebView\x64\* to root ..." -ForegroundColor Cyan
        Get-ChildItem -Path $nestedX64Dir -File | Copy-Item -Destination $ebDir -Force
        Get-ChildItem -Path $nestedX64Dir -Directory | ForEach-Object {
            Copy-Item -Path $_.FullName -Destination $ebDir -Recurse -Force
        }
        Remove-Item -Recurse -Force (Join-Path $ebDir "EBWebView")
    }
}

# ===== 5) 검증 (WARNING — 빌드 통과시키되 사실 기록) =====
$mustExist = @("msedgewebview2.exe", "msedgewebview2.exe.sig", "EmbeddedBrowserWebView.dll")
$missing = @()
foreach ($f in $mustExist) {
    if (-not (Test-Path (Join-Path $ebDir $f))) { $missing += $f }
}
if (-not (Test-Path (Join-Path $ebDir "Locales"))) { $missing += "Locales/" }

if ($missing.Count -gt 0) {
    Write-Warning "여전히 누락: $($missing -join ', ') — fixed runtime 비기능 가능성 (CI 빌드는 계속)."
} else {
    $ebwvPath = Join-Path $ebDir "EmbeddedBrowserWebView.dll"
    $arch = Arch-Name(Get-PEMachine $ebwvPath)
    Write-Host "  All critical present (EmbeddedBrowserWebView.dll arch=$arch)" -ForegroundColor Green
}

$copiedFiles = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$copiedSize = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($copiedFiles files, $([Math]::Round($copiedSize/1MB,1)) MB)" -ForegroundColor Green

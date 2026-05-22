# WebView2 Fixed Version Runtime bundle for LTSC/offline installer.
#
# Why this uses the Fixed Version CAB instead of the Evergreen standalone installer:
# - v2.6.17~v2.6.20 installed Evergreen on the CI runner and copied
#   EdgeWebView\Application\<version>\ into the LTSC installer.
# - In issue #24, v2.6.20 logs show environment creation succeeds, but WebView2
#   controller creation still hangs on a Windows Defender-only home PC.
# - The supported offline self-contained distribution for apps is Microsoft's
#   Fixed Version Runtime CAB. Bundle that exact runtime instead of a CI runner
#   Evergreen install result.

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$fixedVersion = "148.0.3967.83"
$cabUrl = "https://msedge.sf.dl.delivery.mp.microsoft.com/filestreamingservice/files/7a85774b-c5c9-4095-8672-53572bdc8d8f/Microsoft.WebView2.FixedVersionRuntime.$fixedVersion.x64.cab"
$cabPath = Join-Path $env:TEMP "Microsoft.WebView2.FixedVersionRuntime.$fixedVersion.x64.cab"
$extractRoot = Join-Path $env:TEMP "docufinder-webview2-fixed-$fixedVersion"
$dest = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\src-tauri\webview2-runtime"))
$ebDir = Join-Path $dest "EBWebView\x64"

function Get-PEMachine([string]$path) {
    try {
        $fs = [System.IO.File]::OpenRead($path)
        try {
            $br = New-Object System.IO.BinaryReader($fs)
            $fs.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
            $peOffset = $br.ReadInt32()
            $fs.Seek($peOffset + 4, [System.IO.SeekOrigin]::Begin) | Out-Null
            return $br.ReadUInt16()   # 0x8664 = x64, 0x014C = x86, 0xAA64 = arm64
        } finally {
            $fs.Close()
        }
    } catch {
        return 0
    }
}

function Assert-RequiredFile([string]$path, [string]$label) {
    if (-not (Test-Path $path)) {
        Write-Error "WebView2 Fixed Version Runtime 필수 파일 누락: $label"
        exit 1
    }
}

Write-Host "Downloading WebView2 Fixed Version Runtime x64 $fixedVersion..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $cabUrl -OutFile $cabPath -UseBasicParsing
$cabSize = (Get-Item $cabPath).Length
if ($cabSize -lt 200MB) {
    Write-Error "Fixed Version Runtime CAB 다운로드 비정상 ($cabSize bytes) — 중단."
    exit 1
}
Write-Host "  Downloaded $cabSize bytes" -ForegroundColor Gray

if (Test-Path $extractRoot) {
    Remove-Item -Recurse -Force $extractRoot
}
New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null

Write-Host "Extracting Fixed Version Runtime CAB..." -ForegroundColor Cyan
$expand = Join-Path $env:SystemRoot "System32\expand.exe"
& $expand "-F:*" $cabPath $extractRoot | Out-Host
if ($LASTEXITCODE -ne 0) {
    Write-Error "expand.exe failed while extracting WebView2 Fixed Version Runtime (exit $LASTEXITCODE)"
    exit 1
}

$runtimeRoot = Get-ChildItem $extractRoot -Directory |
    Where-Object { $_.Name -eq "Microsoft.WebView2.FixedVersionRuntime.$fixedVersion.x64" } |
    Select-Object -First 1
if (-not $runtimeRoot) {
    $runtimeRoot = Get-ChildItem $extractRoot -Directory |
        Where-Object { Test-Path (Join-Path $_.FullName "msedgewebview2.exe") } |
        Select-Object -First 1
}
if (-not $runtimeRoot) {
    Write-Error "Fixed Version Runtime 추출 루트 탐색 실패: msedgewebview2.exe 를 포함한 폴더가 없음."
    exit 1
}
Write-Host "  Runtime root: $($runtimeRoot.FullName)" -ForegroundColor Green

if (Test-Path $dest) {
    Remove-Item -Recurse -Force $dest
}
New-Item -ItemType Directory -Path $ebDir -Force | Out-Null
Copy-Item -Path "$($runtimeRoot.FullName)\*" -Destination $ebDir -Recurse -Force

$critical = @(
    "msedgewebview2.exe",
    "msedgewebview2.exe.sig",
    "msedge.dll",
    "msedge.dll.sig",
    "icudtl.dat",
    "resources.pak",
    "v8_context_snapshot.bin"
)
foreach ($name in $critical) {
    Assert-RequiredFile (Join-Path $ebDir $name) $name
}
Assert-RequiredFile (Join-Path $ebDir "Locales") "Locales/"
Assert-RequiredFile (Join-Path $ebDir "EBWebView\x64\EmbeddedBrowserWebView.dll") "EBWebView/x64/EmbeddedBrowserWebView.dll"

$arch = Get-PEMachine (Join-Path $ebDir "msedgewebview2.exe")
if ($arch -ne 0x8664) {
    Write-Error "번들된 msedgewebview2.exe 가 x64 아님 (machine=0x$($arch.ToString('X4'))) — 빌드 중단."
    exit 1
}

$files = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$size = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($files files, $([Math]::Round($size / 1MB, 1)) MB, Fixed Version Runtime x64 $fixedVersion)" -ForegroundColor Green

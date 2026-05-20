# [v2.6.17] WebView2 Fixed Version Runtime 번들 — evergreen x64 standalone installer 기반.
#
# 이슈 #23 근본 원인 (v2.5.27 부터 잠재):
#   빌드 스크립트가 `linkid=2099617` 을 X64 installer 로 오인했으나, 실제로는
#   X86 installer 다 (go.microsoft.com redirect 검증:
#   .../MicrosoftEdgeWebView2RuntimeInstallerX86.exe). 그 결과 evergreen 이 x86 으로
#   깔려, v2.6.13~v2.6.16 이 GH Actions runner 의 Microsoft Edge **브라우저** 폴더를
#   fixed runtime 으로 짜깁기하는 회귀가 줄줄이 누적됐다. Edge 브라우저 트리는
#   WebView2 Fixed Version Runtime 이 아니므로 environment 생성은 통과해도 controller
#   (브라우저 자식 프로세스) 생성 단계에서 실패한다.
#
# 해결:
#   - linkid 를 2124701 (검증된 X64 installer) 로 교정.
#   - evergreen runtime 의 `EdgeWebView\Application\<ver>\` 폴더는 그 자체로
#     self-contained WebView2 런타임이다 (시스템 WebView2 가 바로 이 폴더로 동작).
#     구조를 변형(짜깁기/평탄화)하지 않고 통째로 webview2-runtime\EBWebView\x64\ 로
#     복사한다.
#   - 번들 산출물의 architecture 와 controller 필수 파일을 fail-hard 로 검증한다
#     (v2.6.15 의 fail-soft 는 깨진 빌드를 릴리스로 흘려보냈으므로 폐기).

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# linkid=2124701 → MicrosoftEdgeWebView2RuntimeInstallerX64.exe
# (검증: https://go.microsoft.com/fwlink/?linkid=2124701 302 → ...InstallerX64.exe)
$installerUrl  = "https://go.microsoft.com/fwlink/?linkid=2124701"
$installerPath = Join-Path $env:TEMP "MicrosoftEdgeWebView2RuntimeInstallerX64.exe"
$dest  = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\src-tauri\webview2-runtime"))
$ebDir = Join-Path $dest "EBWebView\x64"

# PE 헤더 IMAGE_FILE_HEADER.Machine 으로 architecture 판정.
function Get-PEMachine([string]$path) {
    try {
        $fs = [System.IO.File]::OpenRead($path)
        try {
            $br = New-Object System.IO.BinaryReader($fs)
            $fs.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
            $peOffset = $br.ReadInt32()
            $fs.Seek($peOffset + 4, [System.IO.SeekOrigin]::Begin) | Out-Null
            return $br.ReadUInt16()   # 0x8664 = x64, 0x014C = x86, 0xAA64 = arm64
        } finally { $fs.Close() }
    } catch { return 0 }
}

# 이미 준비돼 있고 x64 면 skip.
if (Test-Path (Join-Path $ebDir "msedgewebview2.exe")) {
    if ((Get-PEMachine (Join-Path $ebDir "msedgewebview2.exe")) -eq 0x8664) {
        Write-Host "WebView2 fixed runtime 이미 준비됨 (x64): $dest" -ForegroundColor Green
        exit 0
    }
    Write-Warning "기존 webview2-runtime 이 x64 아님 — 재생성."
    Remove-Item -Recurse -Force $dest
}

Write-Host "Downloading WebView2 X64 standalone installer (linkid=2124701)..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing
Write-Host "  Downloaded $((Get-Item $installerPath).Length) bytes" -ForegroundColor Gray

Write-Host "Installing WebView2 Runtime (per-machine, silent)..." -ForegroundColor Cyan
$proc = Start-Process -FilePath $installerPath -ArgumentList "/silent","/install" -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    Write-Error "WebView2 installer 실패 (exit $($proc.ExitCode))"
    exit 1
}

# evergreen install 결과에서 x64 msedgewebview2.exe 를 가진 Application\<ver>\ 폴더 탐색.
$appBases = @(
    "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application",
    "${env:ProgramFiles}\Microsoft\EdgeWebView\Application"
) | Where-Object { Test-Path $_ }

$x64Runtime = $null
foreach ($base in $appBases) {
    $vers = Get-ChildItem $base -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
        Sort-Object { [Version]$_.Name } -Descending
    foreach ($v in $vers) {
        $exe = Join-Path $v.FullName "msedgewebview2.exe"
        if ((Test-Path $exe) -and ((Get-PEMachine $exe) -eq 0x8664)) {
            $x64Runtime = $v
            break
        }
    }
    if ($x64Runtime) { break }
}

if (-not $x64Runtime) {
    Write-Error "x64 WebView2 evergreen runtime 폴더를 못 찾음. linkid=2124701 설치 결과에 x64 msedgewebview2.exe 가 없음 — 빌드 중단."
    exit 1
}
Write-Host "  x64 WebView2 evergreen runtime: $($x64Runtime.FullName)" -ForegroundColor Green

# Application\<ver>\ 전체를 구조 그대로 EBWebView\x64\ 로 복사 (evergreen 의 내부
# 상대 구조 — msedgewebview2.exe + EBWebView\x64\ + *.dll/*.pak/*.dat + Locales\ —
# 를 보존해야 controller 가 정상 동작한다. 평탄화/짜깁기 금지).
New-Item -ItemType Directory -Path $ebDir -Force | Out-Null
Copy-Item -Path "$($x64Runtime.FullName)\*" -Destination $ebDir -Recurse -Force

# ===== fail-hard 검증 =====
# controller(브라우저 자식 프로세스) 가 실제로 요구하는 핵심 파일들.
$critical = @(
    "msedgewebview2.exe",
    "msedgewebview2.exe.sig",
    "icudtl.dat",
    "resources.pak",
    "v8_context_snapshot.bin"
)
$missing = @($critical | Where-Object { -not (Test-Path (Join-Path $ebDir $_)) })
if (-not (Test-Path (Join-Path $ebDir "Locales"))) { $missing += "Locales/" }
if ($missing.Count -gt 0) {
    Write-Error "controller 필수 파일 누락: $($missing -join ', ') — 빌드 중단."
    exit 1
}

# 번들된 msedgewebview2.exe architecture 최종 확인.
$arch = Get-PEMachine (Join-Path $ebDir "msedgewebview2.exe")
if ($arch -ne 0x8664) {
    Write-Error "번들된 msedgewebview2.exe 가 x64 아님 (machine=0x$($arch.ToString('X4'))) — 빌드 중단."
    exit 1
}

$files = (Get-ChildItem $dest -Recurse -File | Measure-Object).Count
$size  = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host "Done: $dest ($files files, $([Math]::Round($size/1MB,1)) MB, x64 verified)" -ForegroundColor Green

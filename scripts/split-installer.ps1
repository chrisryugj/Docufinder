# 설치본을 내부망(망연계) 전송용 조각으로 나누고 SHA256 을 남긴다 (이슈 #51).
#
# 큰 설치 파일이 망연계를 지나며 깨지면 NSIS 가 "Installer integrity check has failed" 로 멈춘다
# (lite 286MB 는 설치되고 일반판 337MB 는 깨진 사례). 조각으로 옮겨 받는 PC 에서 `copy /b` 로 합치고
# certutil 로 원본과 같은지 확인하게 한다.
#
# 사용: split-installer.ps1 -Setup <setup.exe> -OutDir <폴더> [-PartMB 90]
# 산출: <OutDir>/<이름>.001… 조각, <이름>.README.txt(합치는 법), SHA256SUMS.txt(원본·조각 해시, 덧붙임)
# 표준 출력: 릴리스에 올릴 파일 경로(조각 + README). SHA256SUMS.txt 는 호출부가 마지막에 올린다.
param(
    [Parameter(Mandatory)][string]$Setup,
    [Parameter(Mandatory)][string]$OutDir,
    [int]$PartMB = 90
)

$ErrorActionPreference = "Stop"
# .NET 파일 API 는 PowerShell 현재 위치가 아니라 프로세스 기준으로 상대 경로를 풀므로 절대 경로로 바꿔 둔다.
$OutDir = (New-Item $OutDir -ItemType Directory -Force).FullName
$src = Get-Item $Setup
# 릴리스에 올라가는 이름 — GitHub 는 업로드할 때 공백을 점으로 바꾼다(lite: "Anything Lite_…" → "Anything.Lite_…").
# 조각·README·SHA256SUMS 가 받은 사람의 파일 이름과 같아야 copy /b·certutil 줄이 그대로 먹는다.
$name = $src.Name -replace ' ', '.'
$sums = Join-Path $OutDir "SHA256SUMS.txt"

function Add-Sum([string]$path, [string]$asName) {
    $h = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
    Add-Content $sums "$h  $asName" -Encoding ascii
    return $h
}

$fullHash = Add-Sum $src.FullName $name

$partSize = $PartMB * 1MB
$bytes = [System.IO.File]::ReadAllBytes($src.FullName)
$count = [int][math]::Ceiling($bytes.Length / $partSize)
$parts = @()
for ($i = 0; $i -lt $count; $i++) {
    $start = $i * $partSize
    $len = [math]::Min($partSize, $bytes.Length - $start)
    $chunk = New-Object byte[] $len
    [Array]::Copy($bytes, $start, $chunk, 0, $len)
    $part = Join-Path $OutDir ("{0}.{1:D3}" -f $name, ($i + 1))
    [System.IO.File]::WriteAllBytes($part, $chunk)
    Add-Sum $part (Split-Path $part -Leaf) | Out-Null
    $parts += $part
}

$partList = ($parts | ForEach-Object { "    {0}  ({1:N0} 바이트)" -f (Split-Path $_ -Leaf), (Get-Item $_).Length }) -join "`n"
$plus = ($parts | ForEach-Object { Split-Path $_ -Leaf }) -join "+"
$readme = @"
$name — 내부망(망연계) 전송용 분할 파일 $count 개

큰 설치 파일이 전송 중 깨지면 설치할 때 "Installer integrity check has failed" 가 뜹니다.
그럴 때는 원본 대신 아래 조각들을 옮겨 내부망 PC 에서 합치세요.

1) 조각 $count 개를 모두 같은 폴더에 둡니다.
$partList

2) 그 폴더에서 명령 프롬프트(cmd)를 열고 아래 한 줄로 합칩니다.
    copy /b $plus $name

3) 합친 파일이 원본과 같은지 확인한 뒤 설치합니다. 출력된 SHA256 값이 아래와 같아야 합니다.
    certutil -hashfile $name SHA256
    $fullHash

   값이 다르면 조각 하나가 전송 중 깨진 것입니다. 조각마다 certutil 로 값을 내어
   SHA256SUMS.txt 와 대조하고, 다른 조각만 다시 옮기세요.
   원본 설치 파일을 그대로 옮겼을 때도 같은 방법으로 깨졌는지 확인할 수 있습니다.
"@
$readmePath = Join-Path $OutDir "$name.README.txt"
Set-Content -Path $readmePath -Value ($readme -replace "`r?`n", "`r`n") -Encoding utf8BOM

Write-Host "split: $name ($($bytes.Length) bytes) → $count 조각, SHA256 $fullHash"
$parts
$readmePath

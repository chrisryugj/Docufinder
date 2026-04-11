# kordoc + Node.js 런타임을 Tauri 번들 리소스로 준비
# 용도: pnpm tauri:build 전에 실행
# 결과: src-tauri/resources/kordoc/ (cli.js + chunks + node_modules subset)
#        src-tauri/resources/node.exe

param(
    [string]$KordocDir = "c:\github_project\kordoc",
    [string]$OutputDir = "$PSScriptRoot\..\src-tauri\resources"
)

$ErrorActionPreference = "Stop"

Write-Host "=== kordoc 번들 준비 ===" -ForegroundColor Cyan

# 1. node.exe 복사
$nodeExe = (Get-Command node -ErrorAction SilentlyContinue).Source
if (-not $nodeExe) {
    Write-Error "Node.js가 설치되어 있지 않습니다"
    exit 1
}
Write-Host "Node.js: $nodeExe"
Copy-Item $nodeExe "$OutputDir\node.exe" -Force
Write-Host "  -> node.exe 복사 완료"

# 2. kordoc dist 복사 (sourcemap 제외)
$kordocOut = "$OutputDir\kordoc"
if (Test-Path $kordocOut) { Remove-Item $kordocOut -Recurse -Force }
New-Item $kordocOut -ItemType Directory -Force | Out-Null

# dist 파일 복사 (.map 제외, 서브디렉토리 유지)
Get-ChildItem "$KordocDir\dist" -Recurse -File |
    Where-Object { $_.Extension -ne ".map" -and $_.Extension -ne ".cts" -and $_.Name -ne "index.d.ts" } |
    ForEach-Object {
        $relPath = $_.FullName.Substring("$KordocDir\dist\".Length)
        $destPath = Join-Path $kordocOut $relPath
        $destDir = Split-Path $destPath -Parent
        if (-not (Test-Path $destDir)) { New-Item $destDir -ItemType Directory -Force | Out-Null }
        Copy-Item $_.FullName $destPath -Force
    }
Write-Host "  -> kordoc dist 복사 완료"

# 3. package.json (ESM 모드)
@'
{"type":"module","name":"kordoc-bundle","private":true}
'@ | Set-Content "$kordocOut\package.json" -Encoding UTF8

# 4. 런타임 node_modules 설치 (최소한)
Push-Location $kordocOut
# kordoc의 dependencies + pdfjs-dist (peer dep)
$deps = @("@xmldom/xmldom", "commander", "jszip", "zod", "cfb", "pdfjs-dist@4")
Write-Host "  -> node_modules 설치 중: $($deps -join ', ')"
& npm install --omit=dev --no-package-lock --no-fund --no-audit $deps 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    Write-Error "npm install 실패 (exit $LASTEXITCODE)"
    exit 1
}
Pop-Location

# 불필요한 파일 정리 (typescript defs, docs, tests)
if (Test-Path "$kordocOut\node_modules") {
    Get-ChildItem "$kordocOut\node_modules" -Recurse -Include "*.d.ts","*.d.mts","*.md","LICENSE*","CHANGELOG*","*.map","tsconfig*" |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

$totalSize = (Get-ChildItem "$OutputDir\kordoc" -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
$nodeSize = (Get-Item "$OutputDir\node.exe").Length / 1MB
Write-Host ""
Write-Host "=== 번들 완료 ===" -ForegroundColor Green
Write-Host "  kordoc: $([math]::Round($totalSize, 1)) MB"
Write-Host "  node.exe: $([math]::Round($nodeSize, 1)) MB"
Write-Host "  합계: $([math]::Round($totalSize + $nodeSize, 1)) MB"

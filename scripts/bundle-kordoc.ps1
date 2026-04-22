# Bundle kordoc + Node.js runtime into Tauri resources
# Run before: pnpm tauri:build
# Output: src-tauri/resources/kordoc/ (cli.js + chunks + node_modules subset)
#         src-tauri/resources/node.exe

param(
    [string]$KordocDir = "",
    [string]$OutputDir = "$PSScriptRoot\..\src-tauri\resources"
)

$ErrorActionPreference = "Stop"

Write-Host "=== kordoc bundle ===" -ForegroundColor Cyan

# Resolve kordoc source directory (priority: -KordocDir > $env:KORDOC_DIR > known locations)
if (-not $KordocDir) { $KordocDir = $env:KORDOC_DIR }
if (-not $KordocDir) {
    $candidates = @(
        "c:\github_project\kordoc",
        "d:\AI_Project\kordoc",
        "$PSScriptRoot\..\..\kordoc"
    )
    foreach ($c in $candidates) {
        if (Test-Path "$c\dist") { $KordocDir = (Resolve-Path $c).Path; break }
    }
}
if (-not $KordocDir -or -not (Test-Path "$KordocDir\dist")) {
    Write-Error "kordoc source not found. Tried: -KordocDir param, `$env:KORDOC_DIR, $($candidates -join ', '). Clone https://github.com/chrisryugj/kordoc and run 'npm install && npm run build' first."
    exit 1
}
Write-Host "kordoc source: $KordocDir"

# 1. Copy node.exe
$nodeExe = (Get-Command node -ErrorAction SilentlyContinue).Source
if (-not $nodeExe) {
    Write-Error "Node.js is not installed"
    exit 1
}
Write-Host "Node.js: $nodeExe"
if (-not (Test-Path $OutputDir)) { New-Item $OutputDir -ItemType Directory -Force | Out-Null }
Copy-Item $nodeExe "$OutputDir\node.exe" -Force
Write-Host "  -> node.exe copied"

# 2. Copy kordoc dist (exclude sourcemaps)
$kordocOut = "$OutputDir\kordoc"
if (Test-Path $kordocOut) { Remove-Item $kordocOut -Recurse -Force }
New-Item $kordocOut -ItemType Directory -Force | Out-Null

Get-ChildItem "$KordocDir\dist" -Recurse -File |
    Where-Object { $_.Extension -ne ".map" -and $_.Extension -ne ".cts" -and $_.Name -ne "index.d.ts" } |
    ForEach-Object {
        $relPath = $_.FullName.Substring("$KordocDir\dist\".Length)
        $destPath = Join-Path $kordocOut $relPath
        $destDir = Split-Path $destPath -Parent
        if (-not (Test-Path $destDir)) { New-Item $destDir -ItemType Directory -Force | Out-Null }
        Copy-Item $_.FullName $destPath -Force
    }
Write-Host "  -> kordoc dist copied"

# 3. package.json (ESM mode)
@'
{"type":"module","name":"kordoc-bundle","private":true}
'@ | Set-Content "$kordocOut\package.json" -Encoding UTF8

# 4. Install runtime node_modules (minimal)
#
# 수식 OCR optional deps:
#   @hyzyla/pdfium        — PDF 페이지 → 비트맵 렌더
#   onnxruntime-node       — MFD + MFR ONNX 추론
#   sharp                  — 수식 영역 crop + raw RGBA 변환
#   @huggingface/transformers — XLMRoberta tokenizer (tokenizer.json 로드)
# 이들이 번들에 포함되지 않으면 `--formula-ocr` 플래그가 tryImport 단계에서 실패.
# 모델(~155MB)은 런타임 HuggingFace 다운로드이므로 여기서는 SDK 바이너리만 포함.
Push-Location $kordocOut
$deps = @(
    "@xmldom/xmldom", "commander", "jszip", "zod", "cfb", "pdfjs-dist@4",
    "@hyzyla/pdfium@^2", "onnxruntime-node@^1.24", "sharp@^0.34", "@huggingface/transformers@^4"
)
Write-Host "  -> Installing node_modules: $($deps -join ', ')"
# npm이 stderr에 warn을 써도 Stop 모드에서 죽지 않도록 이 블록만 Continue로 전환
$prevErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Continue"
& npm.cmd install --omit=dev --no-package-lock --no-fund --no-audit --loglevel=error $deps 2>&1 | ForEach-Object { Write-Host $_ }
$npmExit = $LASTEXITCODE
$ErrorActionPreference = $prevErrorAction
if ($npmExit -ne 0) {
    Pop-Location
    Write-Error "npm install failed (exit $npmExit)"
    exit 1
}
Pop-Location

# Clean up unnecessary files (typescript defs, docs, tests)
if (Test-Path "$kordocOut\node_modules") {
    Get-ChildItem "$kordocOut\node_modules" -Recurse -Include "*.d.ts","*.d.mts","*.md","LICENSE*","CHANGELOG*","*.map","tsconfig*" |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

$totalSize = (Get-ChildItem "$OutputDir\kordoc" -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
$nodeSize = (Get-Item "$OutputDir\node.exe").Length / 1MB
Write-Host ""
Write-Host "=== Bundle complete ===" -ForegroundColor Green
Write-Host "  kordoc: $([math]::Round($totalSize, 1)) MB"
Write-Host "  node.exe: $([math]::Round($nodeSize, 1)) MB"
Write-Host "  total: $([math]::Round($totalSize + $nodeSize, 1)) MB"

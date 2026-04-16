# Bundle kordoc + Node.js runtime into Tauri resources
# Run before: pnpm tauri:build
# Output: src-tauri/resources/kordoc/ (cli.js + chunks + node_modules subset)
#         src-tauri/resources/node.exe

param(
    [string]$KordocDir = "c:\github_project\kordoc",
    [string]$OutputDir = "$PSScriptRoot\..\src-tauri\resources"
)

$ErrorActionPreference = "Stop"

Write-Host "=== kordoc bundle ===" -ForegroundColor Cyan

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
Push-Location $kordocOut
$deps = @("@xmldom/xmldom", "commander", "jszip", "zod", "cfb", "pdfjs-dist@4")
Write-Host "  -> Installing node_modules: $($deps -join ', ')"
& npm install --omit=dev --no-package-lock --no-fund --no-audit $deps 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    Write-Error "npm install failed (exit $LASTEXITCODE)"
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

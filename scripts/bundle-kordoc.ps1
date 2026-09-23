# Bundle kordoc + Node.js runtime into Tauri resources
# Run before: pnpm tauri:build
# Output: src-tauri/resources/kordoc/ (cli.js + chunks + node_modules subset)
#         src-tauri/resources/node.exe

param(
    [string]$KordocDir = "",
    [string]$OutputDir = "$PSScriptRoot\..\src-tauri\resources",
    # Anything Lite(내부망) 번들. 수식 OCR optional deps 4종을 제외한다 —
    # 서명 없는 네이티브 바이너리(.node/.dll) 수십 개를 설치 폴더에서 빼고,
    # 어차피 lite 는 --formula-ocr 를 CLI 에 넘기지 않는다. docs/LITE-BUILD.md 참고.
    [switch]$Lite
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

# 3. package.json (ESM mode) + 런타임 의존성
# 목록·버전 범위는 kordoc package.json 이 정본 (scripts/kordoc-runtime-deps.cjs). 범위를 npm 명령줄 인자로
# 넘기지 않는다: npm.cmd(배치 파일)를 거치며 cmd.exe 가 ^ 를 이스케이프로 먹어 onnxruntime-node@^1.24.0 이
# 없는 버전 1.24.0 이 되었다(v3.8.9 윈도우 배포 실패). dependencies 에 적고 아래에서 인자 없이 설치한다.
$depsArgs = @("$PSScriptRoot\kordoc-runtime-deps.cjs", "$KordocDir\package.json", "--json")
if ($Lite) {
    Write-Host "  -> LITE mode: 수식 OCR optional deps 제외 (네이티브 바이너리 미포함)" -ForegroundColor Yellow
    $depsArgs += "--lite"
}
$depsJson = (& $nodeExe @depsArgs | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or -not $depsJson.StartsWith("{") -or $depsJson -eq "{}") {
    Write-Error "kordoc 의존성 목록 생성 실패 ($KordocDir\package.json)"
    exit 1
}
"{""type"":""module"",""name"":""kordoc-bundle"",""private"":true,""dependencies"":$depsJson}" |
    Set-Content "$kordocOut\package.json" -Encoding UTF8

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
Write-Host "  -> Installing node_modules: $depsJson"
# npm이 stderr에 warn을 써도 Stop 모드에서 죽지 않도록 이 블록만 Continue로 전환
$prevErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Continue"
& npm.cmd install --omit=dev --no-package-lock --no-fund --no-audit --loglevel=error 2>&1 | ForEach-Object { Write-Host $_ }
$npmExit = $LASTEXITCODE
$ErrorActionPreference = $prevErrorAction
if ($npmExit -ne 0) {
    Pop-Location
    Write-Error "npm install failed (exit $npmExit)"
    exit 1
}
Pop-Location

# Clean up unnecessary files (typescript defs, docs, tests)
# LICENSE*/NOTICE*/COPYING* 는 지우지 않는다 — 이 node_modules 는 배포본에 그대로
# 번들되므로, MIT·Apache-2.0 등 대부분의 라이선스가 요구하는 저작권 고지가
# 수령자에게 함께 전달되어야 한다.
if (Test-Path "$kordocOut\node_modules") {
    Get-ChildItem "$kordocOut\node_modules" -Recurse -Include "*.d.ts","*.d.mts","*.md","CHANGELOG*","*.map","tsconfig*" |
        Where-Object { $_.Name -notmatch '^(LICEN[CS]E|NOTICE|COPYING)' } |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

# 다른 플랫폼 바이너리 제거. onnxruntime-node 는 darwin·linux·win32 바이너리를 모두 담아
# 온다(약 290MB). 로더는 bin\napi-v*\<platform>\<arch> 하나만 읽는다 (Windows 빌드는 x64).
$ortBin = "$kordocOut\node_modules\onnxruntime-node\bin"
if (Test-Path $ortBin) {
    Get-ChildItem $ortBin -Directory | ForEach-Object {
        Get-ChildItem $_.FullName -Directory | ForEach-Object {
            if ($_.Name -ne "win32") {
                Remove-Item $_.FullName -Recurse -Force
            } else {
                Get-ChildItem $_.FullName -Directory | Where-Object { $_.Name -ne "x64" } |
                    Remove-Item -Recurse -Force
            }
        }
    }
}
# onnxruntime-web(약 120MB)은 브라우저용. kordoc 은 Node 에서 onnxruntime-node 만 쓰고,
# 수식 OCR 토크나이저(@huggingface/transformers)도 이것 없이 로드된다(실측).
Remove-Item "$kordocOut\node_modules\onnxruntime-web" -Recurse -Force -ErrorAction SilentlyContinue

$totalSize = (Get-ChildItem "$OutputDir\kordoc" -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
$nodeSize = (Get-Item "$OutputDir\node.exe").Length / 1MB
Write-Host ""
Write-Host "=== Bundle complete ===" -ForegroundColor Green
Write-Host "  kordoc: $([math]::Round($totalSize, 1)) MB"
Write-Host "  node.exe: $([math]::Round($nodeSize, 1)) MB"
Write-Host "  total: $([math]::Round($totalSize + $nodeSize, 1)) MB"

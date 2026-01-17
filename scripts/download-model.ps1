# e5-small-v2 모델 다운로드 스크립트
# Usage: .\scripts\download-model.ps1

$ErrorActionPreference = "Stop"

$MODEL_DIR = Join-Path $PSScriptRoot "..\models"
$MODEL_URL = "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx"
$TOKENIZER_URL = "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json"
$ONNX_RUNTIME_URL = "https://github.com/microsoft/onnxruntime/releases/download/v1.19.2/onnxruntime-win-x64-1.19.2.zip"

# models 폴더 생성
if (-not (Test-Path $MODEL_DIR)) {
    New-Item -ItemType Directory -Path $MODEL_DIR | Out-Null
}

Write-Host "=== DocuFinder 시맨틱 검색 모델 다운로드 ===" -ForegroundColor Cyan
Write-Host ""

# 1. ONNX Runtime 다운로드
$onnxDll = Join-Path $MODEL_DIR "onnxruntime.dll"
if (-not (Test-Path $onnxDll)) {
    Write-Host "[1/3] ONNX Runtime 다운로드 중..." -ForegroundColor Yellow
    $zipPath = Join-Path $env:TEMP "onnxruntime.zip"
    Invoke-WebRequest -Uri $ONNX_RUNTIME_URL -OutFile $zipPath

    $extractPath = Join-Path $env:TEMP "onnxruntime"
    Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

    $dllSource = Get-ChildItem -Path $extractPath -Recurse -Filter "onnxruntime.dll" | Select-Object -First 1
    Copy-Item $dllSource.FullName -Destination $onnxDll

    Remove-Item $zipPath -Force
    Remove-Item $extractPath -Recurse -Force

    Write-Host "  -> onnxruntime.dll 설치 완료" -ForegroundColor Green
} else {
    Write-Host "[1/3] ONNX Runtime 이미 존재" -ForegroundColor Gray
}

# 2. 모델 다운로드
$modelPath = Join-Path $MODEL_DIR "model.onnx"
if (-not (Test-Path $modelPath)) {
    Write-Host "[2/3] e5-small 모델 다운로드 중 (~90MB)..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $MODEL_URL -OutFile $modelPath
    Write-Host "  -> model.onnx 다운로드 완료" -ForegroundColor Green
} else {
    Write-Host "[2/3] model.onnx 이미 존재" -ForegroundColor Gray
}

# 3. Tokenizer 다운로드
$tokenizerPath = Join-Path $MODEL_DIR "tokenizer.json"
if (-not (Test-Path $tokenizerPath)) {
    Write-Host "[3/3] Tokenizer 다운로드 중..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $TOKENIZER_URL -OutFile $tokenizerPath
    Write-Host "  -> tokenizer.json 다운로드 완료" -ForegroundColor Green
} else {
    Write-Host "[3/3] tokenizer.json 이미 존재" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=== 다운로드 완료 ===" -ForegroundColor Cyan
Write-Host "모델 경로: $MODEL_DIR" -ForegroundColor White
Get-ChildItem $MODEL_DIR | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor Gray
}

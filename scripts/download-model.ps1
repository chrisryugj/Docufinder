# 시맨틱 검색 모델 다운로드 스크립트
# Usage: .\scripts\download-model.ps1

$ErrorActionPreference = "Stop"

# === KoSimCSE-roberta-multitask 임베딩 모델 ===
# 주의: KoSimCSE ONNX 모델은 HuggingFace에 직접 배포되지 않으므로
# model.onnx와 tokenizer.json을 수동으로 배치해야 합니다.
# 번들 전용: tauri:build 시 이 디렉토리에서 리소스를 패키징합니다.
$EMBED_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\kosimcse-roberta-multitask"
$EMBED_MODEL_URL = ""
$EMBED_TOKENIZER_URL = ""

# === Cross-Encoder Reranking 모델 ===
$RERANK_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\ms-marco-MiniLM-L6-v2"
$RERANK_MODEL_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model_quantized.onnx"
$RERANK_TOKENIZER_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json"

# === ONNX Runtime ===
$ONNX_RUNTIME_URL = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip"

# 공통 변수 (호환성 유지)
$MODEL_DIR = $EMBED_MODEL_DIR
$MODEL_URL = $EMBED_MODEL_URL
$TOKENIZER_URL = $EMBED_TOKENIZER_URL

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

# 2. 임베딩 모델 확인 (KoSimCSE - 수동 배치 필요)
$modelPath = Join-Path $MODEL_DIR "model.onnx"
if (-not (Test-Path $modelPath)) {
    if ($MODEL_URL -eq "") {
        Write-Host "[2/3] KoSimCSE model.onnx가 없습니다 (수동 배치 필요)" -ForegroundColor Red
        Write-Host "  -> $MODEL_DIR\model.onnx 에 ONNX 모델 파일을 배치하세요" -ForegroundColor Yellow
    } else {
        Write-Host "[2/3] 임베딩 모델 다운로드 중..." -ForegroundColor Yellow
        Invoke-WebRequest -Uri $MODEL_URL -OutFile $modelPath
        Write-Host "  -> model.onnx 다운로드 완료" -ForegroundColor Green
    }
} else {
    Write-Host "[2/3] model.onnx 이미 존재" -ForegroundColor Gray
}

# 3. Tokenizer 확인
$tokenizerPath = Join-Path $MODEL_DIR "tokenizer.json"
if (-not (Test-Path $tokenizerPath)) {
    if ($TOKENIZER_URL -eq "") {
        Write-Host "[3/3] tokenizer.json이 없습니다 (수동 배치 필요)" -ForegroundColor Red
        Write-Host "  -> $MODEL_DIR\tokenizer.json 에 토크나이저 파일을 배치하세요" -ForegroundColor Yellow
    } else {
        Write-Host "[3/3] Tokenizer 다운로드 중..." -ForegroundColor Yellow
        Invoke-WebRequest -Uri $TOKENIZER_URL -OutFile $tokenizerPath
        Write-Host "  -> tokenizer.json 다운로드 완료" -ForegroundColor Green
    }
} else {
    Write-Host "[3/3] tokenizer.json 이미 존재" -ForegroundColor Gray
}

# 4. Cross-Encoder Reranking 모델 다운로드
Write-Host ""
Write-Host "=== Cross-Encoder Reranking 모델 ===" -ForegroundColor Cyan

# rerank models 폴더 생성
if (-not (Test-Path $RERANK_MODEL_DIR)) {
    New-Item -ItemType Directory -Path $RERANK_MODEL_DIR | Out-Null
}

$RERANK_MODEL_SHA256 = "13d18cce0f3c0b1115f11ce42c2078cc73b6e0bbe7d8b4ba6e6b8b3dd1ebb49b"

$rerankModelPath = Join-Path $RERANK_MODEL_DIR "model.onnx"
if (-not (Test-Path $rerankModelPath)) {
    Write-Host "[4/5] Cross-Encoder 모델 다운로드 중 (~23MB)..." -ForegroundColor Yellow
    $tempPath = Join-Path $RERANK_MODEL_DIR "model.onnx.tmp"
    Invoke-WebRequest -Uri $RERANK_MODEL_URL -OutFile $tempPath
    $hash = (Get-FileHash -Path $tempPath -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $RERANK_MODEL_SHA256) {
        Remove-Item $tempPath -Force
        throw "Reranker 모델 SHA-256 검증 실패! 예상: $RERANK_MODEL_SHA256, 실제: $hash"
    }
    Move-Item $tempPath $rerankModelPath -Force
    Write-Host "  -> model.onnx 다운로드 + SHA-256 검증 완료" -ForegroundColor Green
} else {
    Write-Host "[4/5] Reranker model.onnx 이미 존재" -ForegroundColor Gray
}

$rerankTokenizerPath = Join-Path $RERANK_MODEL_DIR "tokenizer.json"
if (-not (Test-Path $rerankTokenizerPath)) {
    Write-Host "[5/5] Cross-Encoder Tokenizer 다운로드 중..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $RERANK_TOKENIZER_URL -OutFile $rerankTokenizerPath
    Write-Host "  -> tokenizer.json 다운로드 완료" -ForegroundColor Green
} else {
    Write-Host "[5/5] Reranker tokenizer.json 이미 존재" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=== 다운로드 완료 ===" -ForegroundColor Cyan

Write-Host ""
Write-Host "임베딩 모델 경로: $EMBED_MODEL_DIR" -ForegroundColor White
Get-ChildItem $EMBED_MODEL_DIR | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "Reranker 모델 경로: $RERANK_MODEL_DIR" -ForegroundColor White
Get-ChildItem $RERANK_MODEL_DIR | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor Gray
}

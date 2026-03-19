# 시맨틱 검색 모델 다운로드 스크립트
# Usage: .\scripts\download-model.ps1

$ErrorActionPreference = "Stop"

# === KoSimCSE-roberta-multitask 임베딩 모델 (INT8 양자화) ===
$EMBED_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\kosimcse-roberta-multitask"
$EMBED_MODEL_URL = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model_int8.onnx"
$EMBED_MODEL_SHA256 = "877e43d3f3a2ee09a58c08a0d1720f99b3496962e92569c5846299f862ac0f33"
$EMBED_TOKENIZER_URL = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/tokenizer.json"

# === Cross-Encoder Reranking 모델 ===
$RERANK_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\ms-marco-MiniLM-L6-v2"
$RERANK_MODEL_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model_quantized.onnx"
$RERANK_TOKENIZER_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json"

# === ONNX Runtime ===
$ONNX_RUNTIME_URL = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip"
$ONNX_RUNTIME_SHA256 = ""  # TODO: 첫 다운로드 후 해시 기록 (Get-FileHash로 계산)

# models 폴더 생성
if (-not (Test-Path $EMBED_MODEL_DIR)) {
    New-Item -ItemType Directory -Path $EMBED_MODEL_DIR | Out-Null
}

Write-Host "=== DocuFinder 시맨틱 검색 모델 다운로드 ===" -ForegroundColor Cyan
Write-Host ""

# 1. ONNX Runtime 다운로드
$onnxDll = Join-Path $EMBED_MODEL_DIR "onnxruntime.dll"
if (-not (Test-Path $onnxDll)) {
    Write-Host "[1/3] ONNX Runtime 다운로드 중..." -ForegroundColor Yellow
    $zipPath = Join-Path $env:TEMP "onnxruntime.zip"
    Invoke-WebRequest -Uri $ONNX_RUNTIME_URL -OutFile $zipPath

    # SHA-256 검증 (해시가 설정된 경우)
    if ($ONNX_RUNTIME_SHA256 -ne "") {
        $hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()
        if ($hash -ne $ONNX_RUNTIME_SHA256) {
            Remove-Item $zipPath -Force
            throw "ONNX Runtime ZIP SHA-256 검증 실패! 예상: $ONNX_RUNTIME_SHA256, 실제: $hash"
        }
        Write-Host "  -> SHA-256 검증 통과" -ForegroundColor Green
    } else {
        $hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()
        Write-Host "  -> SHA-256 해시 (스크립트에 기록 권장): $hash" -ForegroundColor Yellow
    }

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

# 2. 임베딩 모델 다운로드 (INT8 양자화, ~106MB)
$modelInt8Path = Join-Path $EMBED_MODEL_DIR "model_int8.onnx"
$modelF32Path = Join-Path $EMBED_MODEL_DIR "model.onnx"
if (-not (Test-Path $modelInt8Path) -and -not (Test-Path $modelF32Path)) {
    Write-Host "[2/3] KoSimCSE INT8 모델 다운로드 중 (~106MB)..." -ForegroundColor Yellow
    $tempPath = Join-Path $EMBED_MODEL_DIR "model_int8.onnx.tmp"
    Invoke-WebRequest -Uri $EMBED_MODEL_URL -OutFile $tempPath
    $hash = (Get-FileHash -Path $tempPath -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $EMBED_MODEL_SHA256) {
        Remove-Item $tempPath -Force
        throw "임베딩 모델 SHA-256 검증 실패! 예상: $EMBED_MODEL_SHA256, 실제: $hash"
    }
    Move-Item $tempPath $modelInt8Path -Force
    Write-Host "  -> model_int8.onnx 다운로드 + SHA-256 검증 완료" -ForegroundColor Green
} elseif (Test-Path $modelInt8Path) {
    Write-Host "[2/3] model_int8.onnx 이미 존재" -ForegroundColor Gray
} else {
    Write-Host "[2/3] model.onnx (F32 원본) 이미 존재 - INT8 다운로드 생략" -ForegroundColor Gray
}

# 3. Tokenizer 확인
$tokenizerPath = Join-Path $EMBED_MODEL_DIR "tokenizer.json"
if (-not (Test-Path $tokenizerPath)) {
    Write-Host "[3/3] Tokenizer 다운로드 중..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $EMBED_TOKENIZER_URL -OutFile $tokenizerPath
    Write-Host "  -> tokenizer.json 다운로드 완료" -ForegroundColor Green
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

$RERANK_MODEL_SHA256 = "e9d8ebf845c413e981c175bfe49a3bfa9b3dcce2a3ba54875ee5df5a58639fbe"

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

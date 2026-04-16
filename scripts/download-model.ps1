# Semantic search model download script
# Usage: .\scripts\download-model.ps1

$ErrorActionPreference = "Stop"

# === KoSimCSE-roberta-multitask embedding model (INT8 quantized) ===
$EMBED_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\kosimcse-roberta-multitask"
$EMBED_MODEL_URL = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/model_int8.onnx"
$EMBED_MODEL_SHA256 = "877e43d3f3a2ee09a58c08a0d1720f99b3496962e92569c5846299f862ac0f33"
$EMBED_TOKENIZER_URL = "https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx/resolve/main/tokenizer.json"

# === Cross-Encoder Reranking model ===
$RERANK_MODEL_DIR = Join-Path $PSScriptRoot "..\src-tauri\models\ms-marco-MiniLM-L6-v2"
$RERANK_MODEL_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model_quantized.onnx"
$RERANK_TOKENIZER_URL = "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json"

# === ONNX Runtime ===
$ONNX_RUNTIME_URL = "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip"
$ONNX_RUNTIME_SHA256 = "78d447051e48bd2e1e778bba378bec4ece11191c9e538cf7b2c4a4565e8f5581"

# Create models directory
if (-not (Test-Path $EMBED_MODEL_DIR)) {
    New-Item -ItemType Directory -Path $EMBED_MODEL_DIR | Out-Null
}

Write-Host "=== DocuFinder Semantic Search Model Download ===" -ForegroundColor Cyan
Write-Host ""

# 1. Download ONNX Runtime
$onnxDll = Join-Path $EMBED_MODEL_DIR "onnxruntime.dll"
if (-not (Test-Path $onnxDll)) {
    Write-Host "[1/3] Downloading ONNX Runtime..." -ForegroundColor Yellow
    $zipPath = Join-Path $env:TEMP "onnxruntime.zip"
    Invoke-WebRequest -Uri $ONNX_RUNTIME_URL -OutFile $zipPath

    # SHA-256 verification (if hash is configured)
    if ($ONNX_RUNTIME_SHA256 -ne "") {
        $hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()
        if ($hash -ne $ONNX_RUNTIME_SHA256) {
            Remove-Item $zipPath -Force
            throw "ONNX Runtime ZIP SHA-256 verification failed! Expected: $ONNX_RUNTIME_SHA256, Actual: $hash"
        }
        Write-Host "  -> SHA-256 verification passed" -ForegroundColor Green
    } else {
        $hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()
        Write-Host "  -> SHA-256 hash (recommend adding to script): $hash" -ForegroundColor Yellow
    }

    $extractPath = Join-Path $env:TEMP "onnxruntime"
    Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

    $dllSource = Get-ChildItem -Path $extractPath -Recurse -Filter "onnxruntime.dll" | Select-Object -First 1
    Copy-Item $dllSource.FullName -Destination $onnxDll

    Remove-Item $zipPath -Force
    Remove-Item $extractPath -Recurse -Force

    Write-Host "  -> onnxruntime.dll installed" -ForegroundColor Green
} else {
    Write-Host "[1/3] ONNX Runtime already exists" -ForegroundColor Gray
}

# 2. Download embedding model (INT8 quantized, ~106MB)
$modelInt8Path = Join-Path $EMBED_MODEL_DIR "model_int8.onnx"
$modelF32Path = Join-Path $EMBED_MODEL_DIR "model.onnx"
if (-not (Test-Path $modelInt8Path) -and -not (Test-Path $modelF32Path)) {
    Write-Host "[2/3] Downloading KoSimCSE INT8 model (~106MB)..." -ForegroundColor Yellow
    $tempPath = Join-Path $EMBED_MODEL_DIR "model_int8.onnx.tmp"
    Invoke-WebRequest -Uri $EMBED_MODEL_URL -OutFile $tempPath
    $hash = (Get-FileHash -Path $tempPath -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $EMBED_MODEL_SHA256) {
        Remove-Item $tempPath -Force
        throw "Embedding model SHA-256 verification failed! Expected: $EMBED_MODEL_SHA256, Actual: $hash"
    }
    Move-Item $tempPath $modelInt8Path -Force
    Write-Host "  -> model_int8.onnx downloaded + SHA-256 verified" -ForegroundColor Green
} elseif (Test-Path $modelInt8Path) {
    Write-Host "[2/3] model_int8.onnx already exists" -ForegroundColor Gray
} else {
    Write-Host "[2/3] model.onnx (F32 original) already exists - skipping INT8 download" -ForegroundColor Gray
}

# 3. Check Tokenizer
$tokenizerPath = Join-Path $EMBED_MODEL_DIR "tokenizer.json"
if (-not (Test-Path $tokenizerPath)) {
    Write-Host "[3/3] Downloading Tokenizer..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $EMBED_TOKENIZER_URL -OutFile $tokenizerPath
    Write-Host "  -> tokenizer.json downloaded" -ForegroundColor Green
} else {
    Write-Host "[3/3] tokenizer.json already exists" -ForegroundColor Gray
}

# 4. Download Cross-Encoder Reranking model
Write-Host ""
Write-Host "=== Cross-Encoder Reranking Model ===" -ForegroundColor Cyan

# Create rerank models directory
if (-not (Test-Path $RERANK_MODEL_DIR)) {
    New-Item -ItemType Directory -Path $RERANK_MODEL_DIR | Out-Null
}

$RERANK_MODEL_SHA256 = "e9d8ebf845c413e981c175bfe49a3bfa9b3dcce2a3ba54875ee5df5a58639fbe"

$rerankModelPath = Join-Path $RERANK_MODEL_DIR "model.onnx"
if (-not (Test-Path $rerankModelPath)) {
    Write-Host "[4/5] Downloading Cross-Encoder model (~23MB)..." -ForegroundColor Yellow
    $tempPath = Join-Path $RERANK_MODEL_DIR "model.onnx.tmp"
    Invoke-WebRequest -Uri $RERANK_MODEL_URL -OutFile $tempPath
    $hash = (Get-FileHash -Path $tempPath -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $RERANK_MODEL_SHA256) {
        Remove-Item $tempPath -Force
        throw "Reranker model SHA-256 verification failed! Expected: $RERANK_MODEL_SHA256, Actual: $hash"
    }
    Move-Item $tempPath $rerankModelPath -Force
    Write-Host "  -> model.onnx downloaded + SHA-256 verified" -ForegroundColor Green
} else {
    Write-Host "[4/5] Reranker model.onnx already exists" -ForegroundColor Gray
}

$rerankTokenizerPath = Join-Path $RERANK_MODEL_DIR "tokenizer.json"
if (-not (Test-Path $rerankTokenizerPath)) {
    Write-Host "[5/5] Downloading Cross-Encoder Tokenizer..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $RERANK_TOKENIZER_URL -OutFile $rerankTokenizerPath
    Write-Host "  -> tokenizer.json downloaded" -ForegroundColor Green
} else {
    Write-Host "[5/5] Reranker tokenizer.json already exists" -ForegroundColor Gray
}

Write-Host ""
Write-Host "=== Download Complete ===" -ForegroundColor Cyan

Write-Host ""
Write-Host "Embedding model path: $EMBED_MODEL_DIR" -ForegroundColor White
Get-ChildItem $EMBED_MODEL_DIR | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "Reranker model path: $RERANK_MODEL_DIR" -ForegroundColor White
Get-ChildItem $RERANK_MODEL_DIR | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor Gray
}

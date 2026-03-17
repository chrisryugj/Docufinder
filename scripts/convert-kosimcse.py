"""KoSimCSE-roberta-multitask PyTorch -> ONNX 변환 스크립트

Usage: PYTHONIOENCODING=utf-8 python scripts/convert-kosimcse.py

필요 패키지: torch, transformers, optimum[onnxruntime]
"""

import subprocess
import sys

def install(package):
    subprocess.check_call([sys.executable, "-m", "pip", "install", package, "-q"])

print("=== 의존성 확인 ===")
for pkg in ["torch", "transformers", "optimum"]:
    try:
        __import__(pkg)
    except ImportError:
        name = "optimum[onnxruntime]" if pkg == "optimum" else pkg
        print(f"  설치 중: {name}")
        install(name)

from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer
from pathlib import Path
import shutil, hashlib, os

MODEL_NAME = "BM-K/KoSimCSE-roberta-multitask"
OUTPUT_DIR = Path(__file__).parent.parent / "src-tauri" / "models" / "kosimcse-roberta-multitask"
TMP_DIR = Path(__file__).parent.parent / "_onnx_tmp"

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

# 1. Optimum으로 ONNX 변환
print(f"\n=== {MODEL_NAME} ONNX 변환 ===")
model = ORTModelForFeatureExtraction.from_pretrained(MODEL_NAME, export=True)
tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)

model.save_pretrained(str(TMP_DIR))
tokenizer.save_pretrained(str(TMP_DIR))

# 2. 필요한 파일만 복사
KEEP_FILES = {"model.onnx", "model.onnx.data", "tokenizer.json"}
for name in KEEP_FILES:
    src = TMP_DIR / name
    if src.exists():
        shutil.copy2(str(src), str(OUTPUT_DIR / name))

# 3. 임시 폴더 삭제
shutil.rmtree(str(TMP_DIR), ignore_errors=True)

# 4. SHA-256 출력
print("\n=== SHA-256 ===")
def sha256(filepath):
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()

for name in sorted(KEEP_FILES):
    fp = OUTPUT_DIR / name
    if fp.exists():
        size = fp.stat().st_size / (1024 * 1024)
        print(f"  {name}: {size:.1f} MB  SHA-256: {sha256(fp)}")

print("\n=== 완료 ===")

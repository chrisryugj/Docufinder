"""KoSimCSE-roberta-multitask ONNX 모델 INT8 동적 양자화 스크립트"""

import os
import hashlib
from pathlib import Path

from onnxruntime.quantization import quantize_dynamic, QuantType

MODEL_DIR = Path(__file__).parent.parent / "src-tauri" / "models" / "kosimcse-roberta-multitask"
INPUT_MODEL = MODEL_DIR / "model.onnx"
OUTPUT_MODEL = MODEL_DIR / "model_int8.onnx"


def compute_sha256(filepath: Path) -> str:
    sha256 = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            sha256.update(chunk)
    return sha256.hexdigest()


def main():
    if not INPUT_MODEL.exists():
        print(f"ERROR: {INPUT_MODEL} not found")
        return

    print(f"Input model: {INPUT_MODEL}")
    print(f"Input size: {INPUT_MODEL.stat().st_size / 1024 / 1024:.1f} MB")

    data_file = MODEL_DIR / "model.onnx.data"
    if data_file.exists():
        total = INPUT_MODEL.stat().st_size + data_file.stat().st_size
        print(f"External data: {data_file.stat().st_size / 1024 / 1024:.1f} MB")
        print(f"Total F32 size: {total / 1024 / 1024:.1f} MB")

    print("\nQuantizing to INT8 (dynamic)...")
    quantize_dynamic(
        model_input=str(INPUT_MODEL),
        model_output=str(OUTPUT_MODEL),
        weight_type=QuantType.QInt8,
    )

    output_size = OUTPUT_MODEL.stat().st_size
    print(f"\nOutput model: {OUTPUT_MODEL}")
    print(f"Output size: {output_size / 1024 / 1024:.1f} MB")

    # SHA-256 계산
    sha = compute_sha256(OUTPUT_MODEL)
    print(f"SHA-256: {sha}")

    # 외부 .data 파일 생성 여부 확인
    output_data = OUTPUT_MODEL.with_suffix(".onnx.data")
    if output_data.exists():
        print(f"External data also created: {output_data.stat().st_size / 1024 / 1024:.1f} MB")
        print(f"External data SHA-256: {compute_sha256(output_data)}")

    print("\nDone! Update Rust code to load model_int8.onnx")


if __name__ == "__main__":
    main()

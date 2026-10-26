#!/usr/bin/env python3
"""임베딩 속도·메모리 — 앱 임베더 조건(embedder/mod.rs: CPU EP, 최적화 Level3, 서브배치 8)으로 회귀셋 청크를 임베딩한다.

  venv/bin/python speed.py --model kosimcse --max-len 128 --threads 4

모델마다 따로 실행한다(최대 메모리는 프로세스 단위). 첫 한 번은 데우기로 빼고 청크당 ms 를 낸다.
"""
import argparse
import json
import resource
import sys
import time

import numpy as np

from bench import EG2_DOC, EG2_FILES, EG2_REPO, HERE, MODEL_REPO, pipeline


def session(path, threads):
    import onnxruntime as ort

    so = ort.SessionOptions()
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    so.intra_op_num_threads = threads
    return ort.InferenceSession(path, so, providers=["CPUExecutionProvider"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="kosimcse", choices=["kosimcse", *EG2_FILES])
    ap.add_argument("--max-len", type=int, default=128)
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()

    from huggingface_hub import hf_hub_download
    from tokenizers import Tokenizer

    t0 = time.perf_counter()
    if args.model == "kosimcse":
        mdir = HERE / "model"
        path = hf_hub_download(MODEL_REPO, "model_int8.onnx", local_dir=mdir)
        tok = Tokenizer.from_file(hf_hub_download(MODEL_REPO, "tokenizer.json", local_dir=mdir))
        prefix = ""
    else:
        mdir, name = HERE / "model-eg2", EG2_FILES[args.model]
        path = hf_hub_download(EG2_REPO, f"onnx/{name}.onnx", local_dir=mdir)
        hf_hub_download(EG2_REPO, f"onnx/{name}.onnx_data", local_dir=mdir)
        tok = Tokenizer.from_file(hf_hub_download(EG2_REPO, "tokenizer.json", local_dir=mdir))
        prefix = EG2_DOC
    tok.enable_truncation(args.max_len)
    tok.enable_padding()
    sess = session(path, args.threads)
    load_ms = (time.perf_counter() - t0) * 1000
    names = [i.name for i in sess.get_inputs()]
    extra = {n: np.zeros((0, 512), dtype=np.float32) for n in names if n.endswith("_features")}

    manifest = json.loads((HERE / "manifest.json").read_text())
    chunks = [c for did in manifest["docs"] for c in pipeline((HERE / "corpus" / f"{did}.md").read_text())]

    def run(batch):
        enc = tok.encode_batch([prefix + t for t in batch])
        ids = np.array([e.ids for e in enc], dtype=np.int64)
        am = np.array([e.attention_mask for e in enc], dtype=np.int64)
        feed = {"input_ids": ids, "attention_mask": am, **extra}
        if "token_type_ids" in names:
            feed["token_type_ids"] = np.zeros_like(ids)
        sess.run(None, feed)

    run(chunks[:8])  # 데우기
    t1 = time.perf_counter()
    for i in range(0, len(chunks), 8):
        run(chunks[i : i + 8])
    ms = (time.perf_counter() - t1) * 1000 / len(chunks)
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / (1024 * 1024 if sys.platform == "darwin" else 1024)
    print(f"{args.model:9s} len={args.max_len:3d} threads={args.threads} chunks={len(chunks)} load={load_ms:6.0f}ms {ms:6.1f}ms/chunk peakRSS={rss:5.0f}MB")


if __name__ == "__main__":
    main()

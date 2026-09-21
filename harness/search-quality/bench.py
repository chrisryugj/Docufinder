#!/usr/bin/env python3
"""검색 품질 회귀 벤치 — 벡터 검색 top-k 적중률.

docufinder 인덱싱 파이프라인을 파이썬으로 재현한다:
  kordoc 마크다운 → html_tables_to_text → chunk_text(600/150) → normalize_text
  → collapse_masking_runs → KoSimCSE(int8) mean-pool → 문서 점수 = max chunk cosine.
Rust 쪽 함수를 바꾸면 여기도 같이 맞춰야 한다 (parsers/mod.rs, parsers/kordoc.rs).

  python bench.py            # raw(별표 유지) vs current(현행) 비교 + 게이트
  python bench.py --all      # 실험 변형(결재선 표 제거·제목 접두)까지
게이트: current 의 top-1 ≥ 0.85, top-3 ≥ 0.95 아니면 exit 1.
"""
import argparse
import json
import re
import sys
import unicodedata
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
MODEL_REPO = "chrisryugj/kosimcse-roberta-multitask-onnx"
GATE_TOP1, GATE_TOP3 = 0.85, 0.95

# ── Rust 파이프라인 재현 ────────────────────────────────────────────
TABLE = re.compile(r"(?is)<table[^>]*>.*?</table>")
ROW = re.compile(r"(?is)<tr[^>]*>(.*?)</tr>")
CELL = re.compile(r"(?is)<t[dh][^>]*>(.*?)</t[dh]>")
INNER = re.compile(r"(?is)<[^>]+>")
LEFTOVER = re.compile(r"(?is)</?(?:table|thead|tbody|tfoot|tr|td|th|col|colgroup|br)[^>]*>")
UNDERLINE = re.compile(r"(?i)</?u>")
MASK = re.compile(r"(?:\\?\*){3,}")  # parsers/mod.rs collapse_masking_runs


def html_tables_to_text(md: str) -> str:  # parsers/kordoc.rs
    def rep(m):
        rows = []
        for r in ROW.finditer(m.group(0)):
            cells = [" ".join(INNER.sub(" ", c.group(1)).split()) for c in CELL.finditer(r.group(1))]
            cells = [c for c in cells if c]
            if cells:
                rows.append(" ".join(cells))
        return "\n" + "\n".join(rows) + "\n" if rows else ""

    return UNDERLINE.sub("", LEFTOVER.sub(" ", TABLE.sub(rep, md)))


def _boundary(ch, start, limit):
    best, i = None, start
    while i < limit:
        c = ch[i]
        if c == "\n" and i + 1 < limit and ch[i + 1] == "\n":
            best, i = i + 2, i + 2
            continue
        if c in ".!?다요" and i + 1 < len(ch) and ch[i + 1] in " \n\r":
            best = i + 1
        i += 1
    return best


def chunk_text(text: str, size=600, overlap=150):  # parsers/mod.rs
    ch, n, out, s = list(text), len(text), [], 0
    while s < n:
        raw_end = min(s + size, n)
        search_start = s + min(size * 4 // 5, raw_end - s)
        end = (_boundary(ch, search_start, raw_end) or raw_end) if raw_end < n else raw_end
        out.append("".join(ch[s:end]))
        s = max(end - overlap if end > overlap else end, s + 1)
        if end >= n:
            break
    return out


def normalize_text(t: str) -> str:  # utils/text_normalize.rs (요지)
    t = t.replace("\r\n", "\n").replace("\r", "\n")
    t = re.sub("[​‌‍﻿­]", "", t)
    t = re.sub("[  　]", " ", t)
    return unicodedata.normalize("NFC", t)


def collapse_masking_runs(t: str) -> str:
    return MASK.sub(" ", t)


# ── 실험 변형 (--all) ───────────────────────────────────────────────
HDR = re.compile(r"^\s*(문서번호|결재일자|공개여부|방침번호|시행\s|접수\s|협조자|주무관|전결|代|수신\s*내부결재|\(경유\))")
NAMES = re.compile(r"\s*[가-힣]{2,4}(\s+[가-힣]{2,4}){0,3}\s*(\d\d/\d\d)?\s*")


def strip_header(t: str) -> str:
    lines = t.split("\n")
    return "\n".join(l for i, l in enumerate(lines) if not (i < 40 and (HDR.match(l) or NAMES.fullmatch(l))))


def title_of(md: str) -> str:
    m = re.search(r"<t[hd][^>]*>\s*제\s*목\s*</t[hd]>\s*<t[hd][^>]*>(.*?)</t[hd]>", md, re.S)
    if m:
        return " ".join(re.sub(r"<[^>]+>", " ", m.group(1)).split())
    m = re.search(r"^제목\s+(.+)$", html_tables_to_text(md), re.M)
    return m.group(1).strip() if m else ""


def pipeline(md: str, mask=True, header=False, title=False):
    text = html_tables_to_text(md)
    if header:
        text = strip_header(text)
    chunks = [normalize_text(c) for c in chunk_text(text)]
    if mask:
        chunks = [collapse_masking_runs(c) for c in chunks]
    if title and (t := title_of(md)):
        chunks = [t + "\n" + c for c in chunks]
    return [c for c in chunks if c.strip()]


VARIANTS = {
    "raw (별표 유지, v3.8.7 이전)": dict(mask=False),
    "current": dict(),
    "+결재선 표 제거": dict(header=True),
    "+제목 접두": dict(title=True),
}

# ── 임베딩 ─────────────────────────────────────────────────────────


def load_embedder():
    import onnxruntime as ort
    from huggingface_hub import hf_hub_download
    from tokenizers import Tokenizer

    mdir = HERE / "model"
    model = hf_hub_download(MODEL_REPO, "model_int8.onnx", local_dir=mdir)
    tokj = hf_hub_download(MODEL_REPO, "tokenizer.json", local_dir=mdir)
    tok = Tokenizer.from_file(tokj)
    tok.enable_truncation(512)
    tok.enable_padding()
    sess = ort.InferenceSession(model)
    names = [i.name for i in sess.get_inputs()]

    def embed(texts):
        out = []
        for i in range(0, len(texts), 32):
            enc = tok.encode_batch(texts[i : i + 32])
            ids = np.array([e.ids for e in enc], dtype=np.int64)
            am = np.array([e.attention_mask for e in enc], dtype=np.int64)
            feed = {"input_ids": ids, "attention_mask": am}
            if "token_type_ids" in names:
                feed["token_type_ids"] = np.zeros_like(ids)
            h = sess.run(None, feed)[0]
            if h.ndim == 3:
                m = am[..., None]
                h = (h * m).sum(1) / m.sum(1)
            out.append(h / np.linalg.norm(h, axis=1, keepdims=True))
        return np.vstack(out)

    return embed


def evaluate(embed, corpus, queries, qv, **opts):
    ids, vecs = [], []
    for did, md in corpus.items():
        cs = pipeline(md, **opts)
        vecs.append(embed(cs))
        ids += [did] * len(cs)
    V, ids = np.vstack(vecs), np.array(ids)
    S = qv @ V.T
    top1 = top3 = mrr = 0
    misses = []
    for qi, q in enumerate(queries):
        order = {}
        for j in np.argsort(-S[qi]):
            order.setdefault(ids[j], None)
        rank = list(order).index(q["expect"]) + 1
        top1 += rank == 1
        top3 += rank <= 3
        mrr += 1 / rank
        if rank > 1:
            misses.append((rank, q["query"], list(order)[0]))
    n = len(queries)
    return dict(chunks=len(ids), top1=top1 / n, top3=top3 / n, mrr=mrr / n, misses=misses)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true", help="실험 변형까지 실행")
    ap.add_argument("-v", "--verbose", action="store_true", help="미스 쿼리 출력")
    args = ap.parse_args()

    manifest = json.loads((HERE / "manifest.json").read_text())
    queries = json.loads((HERE / "queries.json").read_text())
    corpus = {did: (HERE / "corpus" / f"{did}.md").read_text() for did in manifest["docs"]}
    embed = load_embedder()
    qv = embed([normalize_text(q["query"]) for q in queries])

    names = list(VARIANTS) if args.all else list(VARIANTS)[:2]
    results = {}
    print(f"docs={len(corpus)} queries={len(queries)}")
    print(f"{'variant':32s} chunks  top-1  top-3  MRR")
    for name in names:
        r = evaluate(embed, corpus, queries, qv, **VARIANTS[name])
        results[name] = r
        print(f"{name:32s} {r['chunks']:5d}  {r['top1']:5.0%}  {r['top3']:5.0%}  {r['mrr']:.2f}")
        if args.verbose:
            for rank, q, top in r["misses"]:
                print(f"    rank={rank:2d} q={q!r} top={manifest['docs'][top]['title'][:40]}")

    cur = results["current"]
    ok = cur["top1"] >= GATE_TOP1 and cur["top3"] >= GATE_TOP3
    print(f"\n게이트 top-1≥{GATE_TOP1:.0%} top-3≥{GATE_TOP3:.0%}: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()

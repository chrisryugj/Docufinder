#!/usr/bin/env bash
# macOS arm64 빌드 리소스 셋업 — node, kordoc, onnxruntime dylib 을 src-tauri/resources/ 에 채운다.
# - Node v20 darwin-arm64
# - kordoc dist + node_modules (prod)
# - ONNX Runtime v1.23.0 osx-arm64 dylib
#
# 사용:
#   bash scripts/setup-macos-resources.sh
#   KORDOC_DIR=/path/to/kordoc bash scripts/setup-macos-resources.sh
set -euo pipefail

NODE_VERSION="v20.18.0"
ORT_VERSION="1.23.0"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RES_DIR="$REPO_ROOT/src-tauri/resources"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$RES_DIR/onnxruntime" "$RES_DIR/paddleocr"

echo "==> [1/3] Node ${NODE_VERSION} darwin-arm64"
NODE_DEST="$RES_DIR/node"
if [[ -x "$NODE_DEST" ]] && "$NODE_DEST" --version 2>/dev/null | grep -q "${NODE_VERSION}"; then
    echo "  skip — already $("$NODE_DEST" --version)"
else
    NODE_TGZ="$TMP_DIR/node.tar.gz"
    curl -fL --retry 3 -o "$NODE_TGZ" \
        "https://nodejs.org/dist/${NODE_VERSION}/node-${NODE_VERSION}-darwin-arm64.tar.gz"
    tar -xzf "$NODE_TGZ" -C "$TMP_DIR"
    cp "$TMP_DIR/node-${NODE_VERSION}-darwin-arm64/bin/node" "$NODE_DEST"
    chmod +x "$NODE_DEST"
    echo "  -> $("$NODE_DEST" --version)"
fi

echo "==> [2/3] kordoc bundle"
KORDOC_DEST="$RES_DIR/kordoc"
KORDOC_SRC="${KORDOC_DIR:-}"
if [[ -z "$KORDOC_SRC" ]]; then
    for c in "$REPO_ROOT/../kordoc" "$HOME/workspace/kordoc"; do
        if [[ -d "$c" ]]; then KORDOC_SRC="$(cd "$c" && pwd)"; break; fi
    done
fi
if [[ -z "$KORDOC_SRC" || ! -d "$KORDOC_SRC" ]]; then
    echo "ERROR: kordoc 소스 미발견. KORDOC_DIR=/path/to/kordoc 설정하거나 ../kordoc 에 두세요." >&2
    exit 1
fi
echo "  source: $KORDOC_SRC"

if [[ ! -d "$KORDOC_SRC/dist" ]]; then
    echo "  building kordoc dist (pnpm install + build)…"
    (cd "$KORDOC_SRC" && pnpm install --frozen-lockfile=false && pnpm run build)
fi

rm -rf "$KORDOC_DEST"
mkdir -p "$KORDOC_DEST"
# dist 내용 복사 (sourcemap/타입 정의 제외)
# macOS BSD find 는 -exec cp --parents 미지원 → 디렉토리 구조 보존하며 직접 복사
(cd "$KORDOC_SRC/dist" && find . -type f \
    ! -name '*.map' ! -name '*.cts' ! -name 'index.d.ts' \
    -print0 | while IFS= read -r -d '' f; do
        mkdir -p "$KORDOC_DEST/$(dirname "$f")"
        cp "$f" "$KORDOC_DEST/$f"
    done)
# 검증 — cli.js 가 복사됐어야 한다 (없으면 hwp/hwpx/docx/pdf 파싱 실패)
[[ -f "$KORDOC_DEST/cli.js" ]] || { echo "ERROR: kordoc dist 복사 실패 ($KORDOC_DEST/cli.js 미존재)" >&2; exit 1; }

# package.json (ESM)
cat > "$KORDOC_DEST/package.json" <<'EOF'
{"type":"module","name":"kordoc-bundle","private":true}
EOF

echo "  installing kordoc runtime deps (npm, prod-only)…"
(cd "$KORDOC_DEST" && npm install --omit=dev --no-package-lock --no-fund --no-audit --loglevel=error \
    "@xmldom/xmldom" "commander" "jszip" "zod" "cfb" "pdfjs-dist@4" \
    "@hyzyla/pdfium@^2" "onnxruntime-node@^1.24" "sharp@^0.34" "@huggingface/transformers@^4")

# trim 불필요 파일
find "$KORDOC_DEST/node_modules" -type f \( \
    -name '*.d.ts' -o -name '*.d.mts' -o -name '*.md' \
    -o -name 'LICENSE*' -o -name 'CHANGELOG*' -o -name '*.map' \
    -o -name 'tsconfig*' \) -delete 2>/dev/null || true

echo "==> [3/3] ONNX Runtime ${ORT_VERSION} osx-arm64"
DYLIB_DEST="$RES_DIR/onnxruntime/libonnxruntime.dylib"
if [[ -f "$DYLIB_DEST" ]]; then
    echo "  skip — already exists ($(du -h "$DYLIB_DEST" | cut -f1))"
else
    ORT_TGZ="$TMP_DIR/ort.tgz"
    curl -fL --retry 3 -o "$ORT_TGZ" \
        "https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-arm64-${ORT_VERSION}.tgz"
    tar -xzf "$ORT_TGZ" -C "$TMP_DIR"
    SRC="$TMP_DIR/onnxruntime-osx-arm64-${ORT_VERSION}/lib/libonnxruntime.${ORT_VERSION}.dylib"
    [[ -f "$SRC" ]] || { echo "ERROR: dylib 미발견: $SRC" >&2; exit 1; }
    cp "$SRC" "$DYLIB_DEST"
    # 자기 install_name 을 절대경로 → @rpath 로 (앱 번들 내부 로드용)
    install_name_tool -id "@rpath/libonnxruntime.dylib" "$DYLIB_DEST" 2>/dev/null || true
    echo "  -> $(du -h "$DYLIB_DEST" | cut -f1)"
fi

echo ""
echo "=== 완료 ==="
echo "  $RES_DIR/node"
echo "  $RES_DIR/kordoc/"
echo "  $RES_DIR/onnxruntime/libonnxruntime.dylib"

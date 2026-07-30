# 제3자 저작물 고지 (Third-Party Notices)

Docufinder(Anything) 배포본에는 아래 제3자 저작물이 포함되거나 함께 배포된다.
각 저작물은 원 저작권자에게 권리가 있으며, 아래 표시된 각자의 라이선스를 따른다.
본 제품의 BUSL-1.1 라이선스는 이들 저작물에 적용되지 않는다.

---

## 번들 바이너리 / 리소스

### PDFium
- 출처: https://pdfium.googlesource.com/pdfium/ (Rust 바인딩: `@hyzyla/pdfium`)
- Copyright (c) 2014 The PDFium Authors. Portions Copyright (c) 2011 The Chromium Authors.
- 라이선스: BSD 3-Clause
- 포함 위치: `resources/pdfium/pdfium.dll` (Windows), macOS 리소스 디렉터리

### ONNX Runtime
- 출처: https://github.com/microsoft/onnxruntime
- Copyright (c) Microsoft Corporation
- 라이선스: MIT
- 포함 위치: `resources/onnxruntime/onnxruntime.dll` (Windows), `libonnxruntime.dylib` (macOS)

### PaddleOCR (검출·인식 모델 및 사전)
- 출처: https://github.com/PaddlePaddle/PaddleOCR
- Copyright (c) 2020 PaddlePaddle Authors
- 라이선스: Apache License 2.0
- 포함 위치: `resources/paddleocr/det.onnx`, `rec.onnx`, `dict.txt`

### Node.js
- 출처: https://nodejs.org
- Copyright Node.js contributors
- 라이선스: MIT
- 포함 위치: `resources/node.exe` — kordoc 사이드카 실행용

### kordoc 및 그 의존성
- 출처: https://github.com/chrisryugj/kordoc
- 라이선스: MIT
- 포함 위치: `resources/kordoc/**`
- kordoc 이 사용하는 npm 의존성(`@xmldom/xmldom`, `commander`, `jszip`, `zod`, `cfb`,
  `markdown-it`, `pdfjs-dist`, `@hyzyla/pdfium`, `onnxruntime-node`, `sharp`,
  `@huggingface/transformers` 등)은 각 패키지 디렉터리의 `LICENSE` 파일을 그대로
  동봉한다.

### Pretendard
- 출처: https://github.com/orioncactus/pretendard
- Copyright (c) 2021 Kil Hyung-jin
- 라이선스: SIL Open Font License 1.1 — 전문: `src/assets/fonts/OFL.txt`
- 포함 위치: `src/assets/fonts/PretendardVariable.woff2`

---

## 별도 다운로드 리소스

### KoSimCSE-roberta-multitask (임베딩 모델)
- 원저작물: https://huggingface.co/BM-K/KoSimCSE-roberta-multitask (저자 BM-K)
- 본 제품은 위 모델을 ONNX 로 변환하고 INT8 로 양자화한 파생물을
  https://huggingface.co/chrisryugj/kosimcse-roberta-multitask-onnx 에서 내려받아 사용한다.
  변환·양자화 스크립트는 `scripts/convert-kosimcse.py`, `scripts/quantize-kosimcse.py` 다.
- 원 모델 저장소에는 라이선스가 명시되어 있지 않다. 따라서 파생물의 재배포 조건 역시
  확정되지 않은 상태이며, 원저작자의 명시적 허락이 확인되기 전까지 이 파생물의
  라이선스는 본 제품의 라이선스와 무관하게 **미확정**으로 표시한다.

---

## 소스 참조 / 이식

### rhwp
- 출처: https://github.com/edwardkim/rhwp
- Copyright (c) 2025-2026 Edward Kim
- 라이선스: **MIT**
- HWP 3.0 파싱 로직 일부를 참조·이식했다. Cargo 의존성으로도 사용한다.

### Rust 크레이트 의존성
`src-tauri/Cargo.toml` 에 선언된 크레이트들은 각자의 라이선스(대부분 MIT 또는
Apache-2.0)를 따른다. 전체 목록과 라이선스는 다음으로 생성할 수 있다.

```bash
cargo install cargo-about
cargo about generate about.hbs
```

---

문의: 누락되었거나 잘못 표시된 고지가 있으면 이슈로 알려주면 정정한다.

# OCR 레이아웃 분석 설계 — 스캔 PDF·이미지 OCR 고도화

> 세션 인계용 설계문서. 이 문서만 읽고 바로 구현 착수 가능하도록 작성.
> 작성 기준일 2026-07-04 / 대상 브랜치 `main` (커밋 `22063ae` 시점)

---

## 0. TL;DR

현재 docufinder OCR은 **텍스트 영역을 검출 순서대로 `\n`으로 이어붙일 뿐** 읽기순서·레이아웃·표구조가 없다. 다단(multi-column) 스캔본은 좌우가 섞이고, 표는 뭉개지고, 머리글/바닥글이 본문에 섞여 검색 품질을 떨어뜨린다.

**docling**(docling-project/docling)이 이 문제를 딥러닝 레이아웃 모델로 푼다 — 하지만 **Python 스택이라 Tauri 데스크톱 앱에 번들 불가**. 그래서 **docling을 의존성으로 붙이지 않는다.** 대신 docling이 제공하는 *능력*(region 분류 + 읽기순서 + 표구조)을 **이미 깔린 `ort` ONNX 런타임에 PP-Structure 계열 모델로 이식**한다. docufinder는 이미 PaddleOCR(DBNet det + SVTR rec) 스택이므로 같은 생태계의 **PP-DocLayout(레이아웃) + SLANet(표구조)** 를 얹는 게 가장 정합적이다.

- **docling = 북극성/벤치마크** (능력 명세와 GT 검증 소스)
- **PP-Structure ONNX = 실제 구현 수단** (기존 `ort`·`model_downloader` 패턴 그대로 재사용)
- **docling-serve 사이드카 = optional 탈출구** (Tier-2, 기본 경로 아님, kordoc 사이드카 패턴 재활용)

검색엔진 관점의 가치: 읽기순서 복원 → chunk 응집도↑ → 임베딩·시맨틱 검색 품질↑ / 표구조 → 키워드 히트↑ / 머리글·바닥글 제거 → 노이즈↓.

---

## 1. 현재 상태 (팩트)

### 1.1 OCR 파이프라인
`src-tauri/src/ocr/` — 순수 Rust + `ort`(ONNX Runtime), 외부 프로세스 없음.

```
detection(DBNet) → crop_quad → recognition(SVTR) → CTC 디코딩 → 조립
```

- [`ocr/mod.rs`](../src-tauri/src/ocr/mod.rs) — `OcrEngine`, `recognize_image()` 오케스트레이션
- [`ocr/detection.rs`](../src-tauri/src/ocr/detection.rs) — `detect()` → `Vec<Quad>`
- [`ocr/recognition.rs`](../src-tauri/src/ocr/recognition.rs) — `recognize_batch()` → `Vec<RecognitionResult>`
- [`ocr/geometry.rs`](../src-tauri/src/ocr/geometry.rs) — `Quad { points:[(f32,f32);4], score }`, `crop_quad()`

### 1.2 **핵심 병목 — bbox가 버려진다**
`recognize_image()` (ocr/mod.rs) 조립부:

```rust
// boxes[i] ↔ crops[i] ↔ rec_results[i] 는 인덱스 1:1 대응이지만
let regions: Vec<OcrRegion> = rec_results
    .iter()
    .filter(|r| !r.text.trim().is_empty())
    .map(|r| OcrRegion { text: r.text.clone(), confidence: r.confidence })  // ← box 소실
    .collect();
let text = regions.iter().map(|r| r.text.as_str())
    .collect::<Vec<_>>().join("\n");  // ← 검출 순서 그대로, 읽기순서 없음
```

`OcrRegion`은 현재 `{ text, confidence }`만 보유. **`Quad` 좌표가 파이프라인엔 존재하나 이 지점에서 폐기됨.** → 레이아웃 작업의 **전제조건 = 이 bbox를 살리는 것**.

### 1.3 소비 지점
- [`parsers/image_ocr.rs`](../src-tauri/src/parsers/image_ocr.rs) — 이미지(jpg/png/bmp/tiff) → `recognize_file()` → `result.text`
- [`parsers/pdf.rs`](../src-tauri/src/parsers/pdf.rs) — 스캔 페이지 판정(`SCANNED_PAGE_CHAR_THRESHOLD=10`) → 래스터라이즈 → `recognize_image()`. 상한: `MAX_OCR_PAGES=20`, `MAX_OCR_FILE_SIZE=100MB`, `MAX_OCR_IMAGE_WIDTH=2000`
- [`parsers/pdf_sniff.rs`](../src-tauri/src/parsers/pdf_sniff.rs) — 이미지 PDF 사전 판정
- 두 경로 모두 최종 소비는 `OcrResult.text`(평문). 아래 청킹 → FTS5 + 벡터 인덱싱.

### 1.4 모델 자산 패턴
- [`model_downloader.rs`](../src-tauri/src/model_downloader.rs) `ensure_ocr_models()` — `models_dir/paddleocr/{det.onnx, rec.onnx, dict.txt}`. URL+SHA-256 상수는 `constants.rs` 계열. 없으면 다운로드, 있으면 해시검증.
- [`constants.rs`](../src-tauri/src/constants.rs) — `OCR_IMAGE_EXTENSIONS`, (OCR_*_URL/SHA는 model_downloader에 상수).
- 세션 생성 패턴: `Session::builder().with_execution_providers([CPU]).with_optimization_level(Level3).with_intra_threads(2..4).commit_from_file(path)` — layout 세션도 동일 복제.

---

## 2. 목표 / 비목표

### 목표
1. **읽기순서 복원** — 다단·복합 레이아웃 스캔본에서 사람이 읽는 순서대로 텍스트 조립.
2. **머리글/바닥글/페이지번호 분리** — 본문 인덱스에서 제외(또는 별도 필드), 검색 노이즈 제거.
3. **표구조 복원(Phase 2)** — `table` region → 마크다운 표. 셀 단위 검색 가능.
4. **미리보기 연동(Phase 3, optional)** — 스캔본/이미지에 대해서도 kordoc `LayoutView`(인라인 SVG)에 region 박스 오버레이.

### 비목표
- docling(Python) 번들·의존. (기본 경로에서 명시적 배제)
- 수식/차트 이해(VLM 필요). kordoc `formula-ocr` optional 경로와 중복 — 별도 백로그.
- OCR 정확도(rec 모델) 자체 개선 — 이번 범위 아님.

---

## 3. 아키텍처 결정

### 3.1 왜 docling을 안 붙이나 (기록)
| | docufinder OCR | docling |
|---|---|---|
| 런타임 | 순수 Rust + `ort` ONNX | Python 3.10+, PyTorch |
| 배포 | Tauri 단일 실행파일, 모델만 다운로드 | pip + 모델 수백MB~GB |
| 레이아웃 | (없음) | RT-DETR(DocLayNet) + TableFormer |

Tauri 데스크톱 앱에 Python 런타임을 얹는 순간 배포·크래시·용량이 붕괴. **불가.**

### 3.2 채택 — PP-Structure 계열 ONNX (in-process, 기본)
이미 PaddleOCR 생태계이므로 다음을 얹는다:
- **PP-DocLayout / PP-DocLayout-L** — 레이아웃 검출 ONNX. region class(text·title·list·table·figure·header·footer·caption 등) + bbox 출력. RT-DETR/PicoDet 계열, ONNX export 가능.
- **SLANet / SLANeXt**(Phase 2) — 표구조 인식 ONNX. 셀 그리드 + 텍스트 매핑.

이유: `ort` 세션·`model_downloader`·`geometry` 전부 재사용, 딕셔너리/전처리 규약 일관, 추가 런타임 0.

#### 모델 실물 (확정 — 블로커 해소됨)
**[GreatV/oar-ocr](https://github.com/GreatV/oar-ocr)** — Rust + `ort` 기반 OCR 라이브러리가 PP-DocLayout·SLANet ONNX를 GitHub 릴리스로 **직배포(Apache-2.0)**. docufinder와 동일 런타임(`ort`)이라 **전처리·후처리 Rust 코드까지 참조 가능** = 구현 리스크 최소.

레이아웃 (RT-DETR/PicoDet 계열, 23 class: text·title·header·footer·page number·table·figure·caption·formula·seal 등):

| 모델 | URL | 크기 | 읽기순서 | 비고 |
|---|---|---|---|---|
| PP-DocLayout-**S** | `.../v0.3.0/pp-doclayout-s.onnx` | 4.7MB | ✗ | 저사양 폴백 |
| PP-DocLayout-**M** | `.../v0.3.0/pp-doclayout-m.onnx` | 22.4MB | ✗ | **★기본 채택** (기존 rec 모델과 유사 footprint, CPU 데스크톱 균형) |
| PP-DocLayout-L | `.../v0.3.0/pp-doclayout-l.onnx` | 123.4MB | ✗ | 고정밀(90.4 mAP), 무거움 |
| PP-DocLayoutV2 | `.../v0.3.0/pp-doclayoutv2.onnx` | 204MB | ✓ (col,row) | 읽기순서 in-model, 대형 → optional 업그레이드 |
| SLANet (표) | `.../v0.3.0/slanet.onnx` | **7.4MB** | — | Phase 2, 초경량 |
| SLANeXt wired | `.../v0.3.0/slanext_wired.onnx` | 350MB | — | 과대, 미채택 |

- 릴리스 베이스: `https://github.com/GreatV/oar-ocr/releases/download/`
- **기본 = PP-DocLayout-M(22.4MB) + kordoc XY-Cut 읽기순서**(Phase 0에서 이미 이식). V2(204MB)의 in-model 읽기순서는 데스크톱 CPU엔 과함 → optional 상위티어로만.
- **표 = SLANet(7.4MB)** — 초경량, 즉시 채택 가능.
- SHA-256: `model_downloader`의 `download_file_optional_hash`가 빈 해시 허용 → 최초 다운로드로 실물 받아 `shasum -a 256`으로 값 확정 후 상수 박기(기존 OCR 모델과 동일 절차). 착수 시 빈 문자열로 시작 가능 = 블로커 아님.

### 3.3 Tier-2 — docling-serve 사이드카 (optional, 기본 아님)
파워유저가 로컬에 docling-serve(HTTP)를 띄운 경우에만, 설정 토글로 외부 위임. kordoc Node.js 사이드카(`parsers/kordoc.rs`)와 동일한 spawn/파이프 패턴. **기본 비활성**, 실패 시 in-process로 폴백. 스캔 PDF 대량 처리 시 `pdf_sniff` 조기차단 교훈(자식 프로세스 누적 → 크래시, #17) 반드시 승계 — per-file spawn 금지, 단일 데몬 재사용.

---

## 4. 데이터 구조 변경

### 4.1 `OcrRegion`에 bbox·분류 추가 (ocr/mod.rs)
```rust
pub struct OcrRegion {
    pub text: String,
    pub confidence: f32,
    pub bbox: BBox,                 // [신규] axis-aligned, Quad에서 유도
    pub kind: RegionKind,           // [신규] 레이아웃 분류 (기본 Text)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionKind {
    Text, Title, List, Table, Figure, Caption, Header, Footer, PageNumber,
}

#[derive(Debug, Clone, Copy)]
pub struct BBox { pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32 }
```

`OcrResult`에 순서화된 본문과 부가필드 분리:
```rust
pub struct OcrResult {
    pub text: String,          // 읽기순서로 재조립된 본문 (Header/Footer/PageNumber 제외)
    pub regions: Vec<OcrRegion>,
    pub confidence: f32,
    pub tables: Vec<TableMarkdown>,   // [Phase 2]
}
```
> 하위호환: `text` 필드 시그니처 불변 → `image_ocr.rs`/`pdf.rs` 소비부 무변경으로 즉시 품질 개선 흡수. `regions`/`tables`는 추가 소비(미리보기·표검색)용.

---

## 5. 구현 단계

### Phase 0 — bbox 살리기 (레이아웃 없이도 즉효)
**목표**: 레이아웃 모델 없이, 기존 `Quad`만으로 읽기순서 근사 → 다단 스크램블 완화.

1. `recognize_image()` 조립부에서 `boxes[i]`를 `OcrRegion.bbox`로 보존(filter 전에 zip).
2. 신규 `ocr/reading_order.rs` — kordoc의 검증된 **XY-Cut++** 이식(순수 알고리즘, Apache-2.0):
   - 참조: `~/workspace/kordoc/src/pdf/xy-cut.ts` (`xyCutOrder`)
   - region bbox 집합 → 재귀적 X/Y 절단 → 읽기순서 정렬.
3. 조립: XY-Cut 순서로 region 나열, 같은 밴드 내 top→bottom, left→right.
4. 게이트: 2단 스캔본 샘플에서 좌우 섞임 해소 확인.

**이 단계만으로 다단 문서 검색 품질 유의미 개선.** 레이아웃 모델 없이 가능 = 블로커 없음, 즉시 착수 가능.

### Phase 1 — 레이아웃 분류 (PP-DocLayout-M)
> **Rust 레퍼런스**: [GreatV/oar-ocr](https://github.com/GreatV/oar-ocr) `src/` — 동일 `ort` 런타임의 PP-DocLayout 전처리(리사이즈·정규화)·후처리(디코드·NMS·class 매핑)를 이미 구현. 착수 전 이 크레이트의 layout 모듈을 정독해 규약 복제(포팅 or 참조).

1. `model_downloader.rs`: `ensure_layout_model()` 추가 — `models_dir/paddleocr/layout.onnx` + SHA. `ensure_ocr_models` 미러. URL = `.../oar-ocr/releases/download/v0.3.0/pp-doclayout-m.onnx`.
2. `constants.rs`: `OCR_LAYOUT_URL`/`OCR_LAYOUT_SHA256`(초기 빈 문자열 허용).
3. 신규 `ocr/layout.rs`:
   ```rust
   pub struct LayoutRegion { pub bbox: BBox, pub kind: RegionKind, pub score: f32 }
   pub fn analyze(session: &Session, image: &DynamicImage) -> Result<Vec<LayoutRegion>, OcrError>;
   ```
   전처리(리사이즈·정규화)·후처리(NMS·class 매핑)는 PP-DocLayout 규약 따름.
4. `OcrEngine`에 `layout_session: Option<Mutex<Session>>` 추가(모델 없으면 `None` → Phase 0 경로로 우아하게 폴백).
5. 융합: OCR text region(det box) → layout region에 **IoU/containment 최대**로 귀속 → `OcrRegion.kind` 채움. 읽기순서는 layout region 단위 XY-Cut, 텍스트는 그 안에서 정렬.
6. **Header/Footer/PageNumber → `text` 본문에서 제외** (regions엔 유지).
7. 게이트: DocLayNet 소형 GT 또는 docling 출력과 대조(§7 벤치).

### Phase 2 — 표구조 (SLANet)
1. `ensure_table_model()` + `OCR_TABLE_URL`.
2. `ocr/table.rs`: `table` region crop → SLANet ONNX → 셀 그리드. 셀 텍스트는 해당 region 내 OCR box를 셀 bbox에 매핑(kordoc `cell-text.ts` `getIntersectionPercent` 방식 참고).
3. **폴백**: SLANet 없거나 실패 시, kordoc의 선없는 표 추론(`cluster-detector.ts`) 알고리즘을 OCR box에 적용 — 순수 알고리즘이라 모델 불필요.
4. `TableMarkdown` 생성 → `OcrResult.tables`. 본문 `text`에는 마크다운 표로 인라인.

### Phase 3 — 미리보기 연동 (optional)
- kordoc `LayoutView`(인라인 SVG 뷰어, 커밋 `c31bcd2`)에 스캔본 region 박스 전달 → 하이라이트 오버레이.
- `commands/preview.rs` 확장, `OcrResult.regions` 직렬화.

---

## 6. 성능·안전 가드 (기존 교훈 승계)

- 레이아웃 세션은 **지연 로드**(첫 OCR 시). 모델 부재 = 기능만 off, 크래시 금지.
- `MAX_OCR_PAGES`/`MAX_OCR_FILE_SIZE`/`MAX_OCR_IMAGE_WIDTH` 상한 그대로 적용 — 레이아웃 추론도 이 상한 안에서.
- layout 추론 1회 = det과 유사 비용. 페이지당 det+layout+rec → 20페이지 상한 유지 시 감내 가능. 벤치로 실측(§7).
- 스레드: 기존 `intra_threads 2..4` 복제. det/layout 병렬 실행 여지 있으나 CPU 코어 경쟁 주의 — 초기엔 순차.
- Tier-2 사이드카 채택 시 **단일 데몬 재사용, per-file spawn 절대 금지** (#17 재발 방지).

---

## 7. 검증 / CI

- CI는 `TypeScript 빌드 + Rust check/test/clippy` (`.github/workflows/`). **clippy·fmt 통과 필수** (docufinder는 커밋마다 `cargo fmt --check` 게이트 — lesson-docufinder-fmt-gate).
- 신규 벤치 `src-tauri/tests/ocr_layout.rs`:
  1. 2단 스캔본 fixture → 읽기순서 정렬 결과가 GT 순서와 일치(순서 정확도).
  2. 머리글/바닥글 fixture → 본문 `text`에서 제외 확인.
  3. 표 fixture(Phase 2) → 셀 텍스트 재현율.
- **GT 소스**: docling으로 동일 PDF를 처리한 마크다운을 오라클로 사용(docling은 벤치용으로만, 런타임 의존 아님). DocLayNet 공개 샘플 병행.
- 회귀: 기존 평문 OCR 결과가 퇴행하지 않는지(텍스트 누락 0) 스냅샷 비교.

---

## 8. 착수 순서 (권장)

1. **Phase 0 먼저** — 블로커 없음. bbox 보존 + XY-Cut 이식만으로 체감 개선. (0.5~1일)
2. Phase 1 착수 **전 블로커 해소**: PP-DocLayout ONNX 실물 URL+SHA 확보. 없으면 여기서 멈추고 모델 확보부터. (§3.2 노트)
3. Phase 1 → 벤치 통과 → Phase 2 → Phase 3 순.

## 9. 열린 질문 (착수 전 확정 필요)

1. ~~PP-DocLayout ONNX 배포처~~ **해소** — GreatV/oar-ocr 릴리스 직배포(Apache-2.0), §3.2 표 참조. 남은 건 M vs S 기본값 최종 결정(권장 M).
2. Header/Footer를 완전 폐기 vs 별도 인덱스 필드(예: 문서 제목/페이지 힌트로 활용)? → 검색 UX 결정.
3. 표를 본문 인라인 마크다운 vs 별도 `tables` 필드만 → FTS5 청킹 전략과 연동.
4. Tier-2 docling-serve 어댑터를 이번 스코프에 포함할지(기본은 제외 권장).
5. 읽기순서를 kordoc XY-Cut(경량, 기본) vs PP-DocLayoutV2 in-model(204MB) — 데스크톱 CPU 감안 XY-Cut 권장, V2는 정확도 부족 시에만.

---

## 참조

- 현재 OCR: [`ocr/mod.rs`](../src-tauri/src/ocr/mod.rs), [`ocr/geometry.rs`](../src-tauri/src/ocr/geometry.rs)
- 소비부: [`parsers/image_ocr.rs`](../src-tauri/src/parsers/image_ocr.rs), [`parsers/pdf.rs`](../src-tauri/src/parsers/pdf.rs), [`parsers/pdf_sniff.rs`](../src-tauri/src/parsers/pdf_sniff.rs)
- 모델 패턴: [`model_downloader.rs`](../src-tauri/src/model_downloader.rs) `ensure_ocr_models`
- 알고리즘 이식 원본(kordoc, Apache-2.0): `~/workspace/kordoc/src/pdf/{xy-cut,cluster-detector,cell-text}.ts`
- **Rust 레퍼런스 구현(동일 `ort` 런타임, Apache-2.0)**: [GreatV/oar-ocr](https://github.com/GreatV/oar-ocr) — PP-DocLayout·SLANet 전처리/후처리 + ONNX 릴리스 배포
- 북극성/벤치: docling — https://github.com/docling-project/docling

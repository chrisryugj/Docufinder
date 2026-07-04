# 다음 세션 — docufinder 잔여 (발행·검증 중심)

> **트리거**: "docufinder 이어서" / "발행" / "OCR 레이아웃" → 이 파일로 시작.
> **직전 세션(2026-07-04, ultracode)**: A(재인덱싱 프롬프트)·B(pdfium 리팩터+검증)·C(뷰어 줌 3종)·
> E(OCR 레이아웃 Phase 1, opt-in)·kordoc README 완료·커밋. D(발행)만 지시 대기.

---

## 0. 직전 세션 완료 (origin/main 대비 ahead 15, 전부 미푸시)

| 커밋 | 내용 | 게이트 |
|---|---|---|
| `d3e6a37` | **C. 팝업 뷰어 줌 UX 3종** — 커서기준 줌(휠·±)·더블클릭 토글·너비↔페이지 순환. width-% 스크롤 모델 + useLayoutEffect 보정. 프론트 only. | tsc/vite |
| `762ee71` | **B. pdfium 문서당 1회 바인딩** — 페이지당 dlopen → 문서당 지연 1회 재사용. thread_safe 로 Send+Sync. 동작 동일. | fmt/clippy/test |
| `85a7dd9` | **A. OCR 재인덱싱 프롬프트** — count_ocr_reindex_candidates(읽기 전용) + SearchTab OCR 토글 ON 시 후보 폴더만 재인덱싱. | 전체 |
| `9266bbf` | **B 감사** — pdfium 4플랫폼 tgz SHA-256 + 내부경로 실측 검증(전부 일치). | — |
| `004408b` | **E. OCR 레이아웃 Phase 1** — PP-DocLayout-M ONNX 포팅(opt-in `ocr_layout_enabled`, 기본 off). 텍스트 영역 분류 + 본문 머리글/바닥글/페이지번호 제외. | fmt/clippy/test |
| `a2cff0b` | **A 수정(리뷰)** — 폴더 쿼리 Windows 역슬래시 + LIKE 메타문자 대응(substr 정확 프리픽스). ★high | fmt/clippy/test |
| `b00f38d` | **C 수정(리뷰)** — 커서 앵커에서 컨테이너 패딩·mx-auto 여백 보정(드리프트 제거). | tsc/vite |
| `ea81f63` | **E 수정(리뷰)** — 레이아웃 로드를 파일존재 대신 설정 기반 게이팅(다운로드 레이스 무관 off 보장). | fmt/clippy/test |

기본 off·모델 부재 시 E 는 완전 무동작(recognize_image 기존과 동일, 249 test 무회귀 확인).
**적대적 리뷰(C/B/A/E 각 발견 독립 검증) 확정 3건 전부 수정.**

### E 검증 상세 (재현·근거)
- **규약 확정**: oar-ocr(GreatV) 소스 정독 — 전처리 640×640 stretch·/255·NCHW, 입력 `image`+`scale_factor`,
  출력 `[N,6]=[class,score,x1,y1,x2,y2]`(multiclass_nms 그래프 내장 → 원본좌표·NMS 완료).
- **실모델 스모크**: `pp-doclayout-m.onnx`(SHA `8e458bfc…`, 23.5MB) 를 실제 실행 —
  실문서(595×841 보도자료)에서 Header(상단)+Text(본문) 검출, 좌표 원본공간·클래스 정상.
  테스트: `ORT_DYLIB_PATH=<libonnxruntime> LAYOUT_MODEL=<layout.onnx> LAYOUT_IMAGE=<doc.png> \
  cargo test --lib ocr::layout::tests::layout_smoke -- --nocapture`

---

## 1. 남은 열린 항목 (우선순위)

### A. 발행 ★지시 대기 (유일한 미완)
- 미푸시 12커밋 push 여부 — 사용자 히스토리 관리. **CHANGELOG 초안·버전 권고(3.0.11)·발행 절차는
  scratchpad/CHANGELOG-draft.md 참조** (버전 bump 3곳: package.json·Cargo.toml·tauri.conf.json).
- kordoc 릴리스 의식(별개): 태그 v3.14.0(`8dc1fa5`)·v3.15.0(`7fb9885`), npm publish(현 3.13.0), gh release.
  README v3.14 섹션은 이번에 로컬 커밋(`c3bc1e2`, 미푸시).

### B. E(레이아웃) 실앱 검증 — opt-in 이라 안전하나 시각검증 미완
- 포팅은 스모크로 검증됐으나, **스캔 PDF 전체 파이프(OCR→레이아웃→머리글/바닥글 제외→인덱스)**
  는 앱에서 미확인. 설정 "레이아웃 분석" ON → 재시작 → 스캔 PDF 인덱싱 → 머리글/바닥글이 검색
  본문에서 빠지는지 확인. 오분류로 본문 누락되면 containment 임계(0.30)·class_to_kind 재조정.
- **§9 Q2/Q3 제품 결정**(설계문서): 머리글/바닥글 완전 폐기 vs 별도 필드 / 표 인라인 vs tables 필드.
  현재는 "본문 text 에서 제외, regions 엔 보존"으로 구현. 사용자 확정 필요.

### C. Win/Linux pdfium 런타임 테스트
- SHA·아카이브 경로는 검증 완료. **런타임 바인딩+OCR 실행은 mac-arm64 만** 실측. Win/Linux 에서
  스캔 PDF OCR 실동작 QA 필요(코드 감사상 정상). tauri 번들 포함 여부도 결정(현재 런타임 다운로드).

### D. 기존 버그(리뷰가 발견, 미수정 — surgical) ★신규
- **레이아웃 뷰 찾기 매치 하이라이트** (`LayoutView.tsx` 활성매치 effect, deps `[matchIdx, matchCount]`):
  새 검색어가 이전과 **매치 수가 같으면** idx·count 둘 다 안 변해 active 하이라이트·스크롤이
  갱신 안 됨. 이번 줌 작업과 무관한 기존 버그라 미수정. 고치려면 deps 에 `findTerm, svg` 추가
  (수집 effect 가 먼저 실행돼 matchesRef 재구성 후 이 effect 가 읽으므로 순서 안전).

### E. 이월 백로그
- E Phase 2(표구조 SLANet)·Phase 3(미리보기 region 오버레이) — 설계 `docs/OCR-LAYOUT-DESIGN.md`.
- 태그 발견성(Med), 드래그아웃 런타임 검증, 앱 시각 검증.

---

## 2. 게이트/주의
- 프론트: `npx tsc --noEmit` + `npx vite build`. Rust: `cargo fmt --all --check` +
  `cargo clippy --all-targets -- -D warnings` + `cargo test` (커밋마다 CI 강제 — lesson-docufinder-fmt-gate).
- 커밋 신원 chrisryugj / ryuseungin@naver.com. main 직접(브랜치 X). **push 는 지시받고**.
- E 레이아웃 검증 자산: 모델·이미지는 세션 scratchpad(pp-doclayout-m.onnx, doc_page.png).

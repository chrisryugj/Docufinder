# 다음 세션 — docufinder 잔여 (발행·검증 중심)

> **트리거**: "docufinder 이어서" / "발행" / "OCR 레이아웃" → 이 파일로 시작.
> **직전 세션(2026-07-04, ultracode)**: A(재인덱싱 프롬프트)·B(pdfium 리팩터+검증)·C(뷰어 줌 3종)·
> E(OCR 레이아웃 Phase 1, opt-in)·kordoc README 완료·커밋. D(발행)만 지시 대기.

---

## 0.0 검증 세션 완료 (2026-07-04 #2, ultracode) — ahead 17

직전 구현물의 **런타임/실측 검증 + 프로덕션 리뷰**(코드리뷰가 안 본 축). 커밋 `7c2414c`.

### 적용 완료 (확정결함 3건, 게이트 전부 통과)
- **LayoutView §1-D 찾기버그** — 활성매치 effect deps 에 `findTerm,svg` 추가(같은 매치수 새 검색어 갱신). ← 아래 §1-D 해소.
- **LayoutView 줌 stale 앵커** — '너비 맞춤' 리셋 분기에 `zoomAnchorRef.current=null`(클램프 한계 no-op 줌 잔여 앵커 → 리셋 스크롤 튐).
- **pdf.rs rasterize 높이상한** — `MAX_OCR_RENDER_HEIGHT=10000` + `set_maximum_height`(극단 종횡비 MediaBox 거대 비트맵 OOM 갭, 임베디드 경로 방어 대칭화).

### E 심화검증 결과 (실측 — 다양한 문서)
5개 실문서 페이지 `analyze()` 실행. Header/Footer 검출은 대개 레터헤드·출처줄(진짜 노이즈)을
정확히 잡지만, **표문서(근무변경신청서)에서 상단 중앙 문서제목을 Header(score 0.523)로 오분류
→ containment 100% → OCR 본문에서 제목 통째 드롭** 재현. 즉 `ocr/mod.rs:301` 필터의 본문손실
리스크가 실증됨(4문서 중 1건).

### ★제품결정 확정 (사용자, 구현은 이번 세션 = 다음 실행분)
- **머리글/바닥글 = 노이즈 클래스 임계 상향**: Header/Footer/PageNumber 는 `score ≥ ~0.62` 일 때만
  본문서 드롭. `ocr/layout.rs` SCORE_THRESHOLD(0.5)는 검출 유지하되, 드롭 판정용 별도 임계를
  노이즈 클래스에만 적용(또는 class_to_kind 후 필터 직전 score 게이트). 근무변경 제목(0.523)은
  유지, 확실한 레터헤드(0.63)만 제거. → 검증데이터: 근무변경 title 0.523 / doc_page header 0.541 /
  refdoc footer 0.518 / press1 header 0.629. 임계 0.62 면 앞 3개 유지·press1 만 드롭.
- 표: 현행 유지(Table kind 는 드롭 안 함 → 본문 인라인 유지, 검색 OK).

### 프로덕션 리뷰 이월 백로그 (다음 세션 처리)
CONFIRMED(미적용):
- **pdf.rs:513** — pdfium fallback 이 페이지마다 `load_pdf_from_file`로 PDF 전체 재로드(≤20회, dlopen 만 문서당 1회). 효율. 문서 1회 로드 후 페이지 인덱스 렌더로 리팩터(lifetime 주의).
- **pipeline.rs:778 / pdf.rs:799** — `looks_like_garbage_text` 가 전 파일타입 확대 적용돼 중/일(한자 지배) 문서에 "복사 시 깨짐" 오탐 배지. ★순진한 수정(한자를 readable 카운트) 금지 — 원 설계가 "한국어 CID깨짐=랜덤한자"라 일부러 제외. **올바른 수정 = garbled 저장을 pdf/hwp 파일타입으로 게이팅**.
- **PreviewPanel.tsx:1408** — '크게 보기' 팝업(role=dialog) 포커스 트랩·초기 포커스 없음(a11y). 마운트 시 focus + Tab 트랩.

PLAUSIBLE(경미, 판단):
- pdf.rs:514 `page_index as u16` 64K+ 페이지 절단(극단), model_downloader.rs:375 pdfium 비원자 쓰기+추출후 미검증(디스크풀 시), settings.rs:813 layout/pdfium 다운로드 실패 무신호(주석 "호출부 warn" 계약 위반), SearchTab.tsx:324 checkOcrCandidates 취소가드 없음.
- **레이아웃 토글 다운로드 UX**: 수식OCR 은 모델상태 패널 있는데 레이아웃 토글은 진행/실패 피드백 전무 — 추가 검토.

보안 축 = 발견 0(count 쿼리 파라미터바인딩·tar dest 고정·URL 상수·icacls 풀패스 안전).

### 앱 시각검증 (미완 — 헤드리스 불가, 사람 실측 필요)
`KORDOC_CLI_PATH=~/workspace/kordoc/dist/cli.js pnpm tauri:dev:mac` 로 띄워 C 줌·A 재인덱싱·E 레이아웃
실동작 확인. ★A: OCR 켜자마자 "지금 재인덱싱" 누르면 det/rec 모델 다운로드 완료 전이라 헛돌 수
있음(모델 온 뒤). 코드 검증은 다 됐고 화면 실측만 남음.

### rhwp 차용 후보 (edwardkim/rhwp, MIT, 활발)
kordoc: ①**HWPX LINE_SEG 합성 규칙**(Tier-2 reflow 와 동일 문제 — 인코딩단계 IR 등가화, `mydocs/eng/feedback/hwpx2ir.md`) ②render-diff 기하 게이트(bbox 구조경로 diff, TS 이식) ③금칙 테이블·표 병합셀 분할 규칙. Paint IR 은 과설계.
docufinder: native PNG 썸네일, AI Q&A용 VLM 타깃 렌더(1568px), rhwp core 크레이트 텍스트추출 교차검증. 라이선스 MIT(아이디어 자유, 코드이식 시 고지).

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

### D. ~~레이아웃 뷰 찾기 매치 하이라이트~~ ✅해소 (2026-07-04 #2, 커밋 7c2414c)
- deps 에 `findTerm, svg` 추가로 수정 완료. (같은 매치수 새 검색어에서 active·스크롤 갱신)

### E. 이월 백로그
- E Phase 2(표구조 SLANet)·Phase 3(미리보기 region 오버레이) — 설계 `docs/OCR-LAYOUT-DESIGN.md`.
- 태그 발견성(Med), 드래그아웃 런타임 검증, 앱 시각 검증.

---

## 2. 게이트/주의
- 프론트: `npx tsc --noEmit` + `npx vite build`. Rust: `cargo fmt --all --check` +
  `cargo clippy --all-targets -- -D warnings` + `cargo test` (커밋마다 CI 강제 — lesson-docufinder-fmt-gate).
- 커밋 신원 chrisryugj / ryuseungin@naver.com. main 직접(브랜치 X). **push 는 지시받고**.
- E 레이아웃 검증 자산: 모델·이미지는 세션 scratchpad(pp-doclayout-m.onnx, doc_page.png).

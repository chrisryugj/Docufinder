# Docufinder 성능 프로덕션 리뷰 (6차) — 완료 (v3.0.9) / 7차 실검증+UI/UX — 완료 (v3.0.10)

작성: 2026-07-03 (맥미니) → **2026-07-03 맥북에서 전 항목 완료, v3.0.9 릴리스.**
→ **7차(실검증+UI/UX)도 2026-07-03 맥북에서 완료, v3.0.10 릴리스** — 아래 "✅ 7차 세션 결과 로그".

---

## 🔖 다음 세션(8차: SVG 레이아웃 미리보기) 프롬프트 — 이거 붙여넣고 시작

```
docufinder 8차 세션: SVG 레이아웃 미리보기 기능 추가. HANDOFF-perf-review.md 의
"🎨 8차 설계 노트 — SVG 레이아웃 미리보기" 읽고 시작. kordoc 은 v3.10.0 으로
이미 핀·번들·호환검증 완료(7차), render CLI 인터페이스도 실측 박제돼 있음.

목표: 미리보기 패널에 "레이아웃 보기" 토글 버튼 — kordoc render 로 HWPX 를
원본 조판 그대로의 SVG 로 렌더해 표시 (마크다운 뷰 ↔ 레이아웃 뷰 전환).

진행: 설계 노트의 계획을 검토·보완해 사용자에게 확정받고 구현. 백엔드
(kordoc.rs 사이드카 배관 재사용 + tauri 커맨드 1개) → 프론트(PreviewPanel
토글) → 실 HWPX 데모 문서로 실물 검증 순.

공통: 커밋은 main 직접, 커밋 전 cargo fmt --check (CI fmt 게이트).
스크롤 로직 절대 불변. DESIGN.md 토큰 준수. 세션 끝에 HANDOFF 기록.
```

## 🎨 8차 설계 노트 — SVG 레이아웃 미리보기 (2026-07-03 실측 기반)

**동기**: 현재 미리보기는 마크다운 변환이라 조판(표 배치·들여쓰기·페이지 모양)이
소실된다. kordoc v3.10.0 신기능 "레이아웃 보존 렌더"로 원본 모양 그대로 보여준다.

**kordoc render 실측 (v3.10.0, 이 맥북)**:
- `node cli.js render <file.hwpx> -o <out.svg> --silent` — 데모 결재문서 47.8KB SVG 생성 확인.
- 출력: A4 viewBox(595.28×841.88pt) 단일 SVG, `<text>` 절대배치 + font-family 폴백 체인
  (함초롬바탕→Noto Serif CJK KR→serif) 문자열 내장 — 한컴 폰트 미설치 환경도 유사 렌더.
- **제약 1**: HWPX 전용 — HWP 입력 시 stderr `"HWPX(ZIP) 형식이 아닙니다 — 렌더는 HWPX만 지원"`.
- **제약 2**: 한컴이 저장하며 심은 **조판 캐시**(lineseg·cellAddr·hp:pos) 기반 — 캐시 없는
  HWPX(타 도구 생성)는 실패 가능. 실패 시 exit code 가 0 으로 보였음(파이프 영향 가능) —
  **stderr/출력파일 존재로 판정할 것** (8차에서 exit code 재확인).
- **제약 3**: 현재 **1페이지만** 렌더 (help 명시). UI 라벨을 "첫 페이지 레이아웃"으로 정직하게.
- 구현체: kordoc `src/render/svg-render.ts` (27KB) — 다페이지 지원 여부/계획은 kordoc 레포 확인.

**설계 스케치**:
1. **백엔드**: `commands/` 에 `render_layout_svg(path) -> ApiResult<String>` —
   `parsers/kordoc.rs` 의 사이드카 경로 해석(find_kordoc_cli·node 경로) 재사용, `render`
   서브커맨드 호출. 출력은 임시파일(`-o`) 후 읽기 or stdout 지원 여부 확인. mtime 키 캐시 고려.
2. **프론트**: PreviewPanel 툴바에 토글 버튼(HWPX 파일일 때만 노출). SVG 표시는
   `<img src="data:image/svg+xml,...">` 격리 삽입 권장(스크립트 실행 차단 — kordoc 이
   통제된 소스라도 방어적으로). 확대/축소는 width 100% + 필요시 줌.
3. **뷰 전환 규칙**: SVG 뷰에선 찾기(⌘F)·인용점프·하이라이트가 무의미 — 찾기 바 숨기고
   토글 시 마크다운 뷰로 복귀. 렌더 실패(HWP·캐시 없음) → 버튼 숨김 or 토스트 후 마크다운 유지.
4. **성능**: 렌더는 파일당 1회 후 캐시(SVG 수십 KB — 메모리 or 임시디렉토리). 사이드카
   콜드스타트(파일당 Node 기동)는 T3-2(persistent worker) 백로그와 별개로 이번엔 수용.

---

## ✅ 7차 세션 결과 로그 (맥북, 2026-07-03)

### (A) 실검증 체크리스트 결과

1. **스모크 ✅** — v3.0.6 설치본이 자동시작으로 돌고 있어 dmg 로 v3.0.9 교체 설치.
   v17 마이그레이션 로그 정상(`idx_files_last_opened`/`idx_files_size` 생성 확인),
   periodic_sync·파일와처 정상, ERROR/panic 0 (WARN 2종은 모델 미설치 안내 + benign
   `resume_with_folders called but was not paused`). 검색→미리보기 dev 실기동 확인.
2. **T3-6 Ctrl+F ✅(코드)+실물 일부** — `cssHighlightsSupported()` 가드 확인, macOS 26.6
   WKWebView 는 지원 범위. **색칠 실물 확인만 잔여** (WebView2 는 윈도우에서).
3. **T1-2 스크롤 — 실물 잔여** (자동 조작이 사용자 화면과 겹쳐 중단).
4. **임베딩 실측 ✅** — `perf_embed_real_model` 신설(커밋 `da61bcd`,
   `DOCUFINDER_BENCH_DB`+`DOCUFINDER_BENCH_MODELS`+`ORT_DYLIB_PATH` opt-in). M칩 실측:
   Embedder 로드 683ms / 쿼리 embed 1.7~2.5ms / **벡터 인덱싱 52.1 chunks/s**(803청크
   15.4s, 쓰로틀 제외 — Balanced 실사용 ≈40/s) / hybrid e2e '보고서' 2.2ms,
   '예산 집행 계획' ~117ms(벡터 전용 히트의 문장 enrich_semantic_results 비용 — 의도된
   기능, T2-2 병렬 배관은 건강. 손 안 댐).
5. **T3-1 — 실물 잔여** (더보기 수백 개 스크롤 체감. 버벅임 실측 전 손대지 않음 유지).

### 🔴 최대 성과 — macOS 배포본 시맨틱/OCR 즉사 버그 수정 (`4e7c5b0`)

- 증상: 임베더/OCR 로드(ORT dylib dlopen) 순간 dyld **SIGKILL(CODESIGNING, Invalid
  Page)** → 앱 즉사. 시맨틱·OCR 이 기본 off 라 실사용자가 아직 못 밟았을 뿐.
- 원인 체인: `setup-macos-resources.sh` 의 `install_name_tool -id` 가 MS linker-signed
  서명을 무효화 → tauri build 가 invalid dylib 품은 채 .app+dmg 동시 생성 →
  publish.yml 재서명 스텝(v2.6.10)은 bundle/macos/.app 만 고침 — **업로드되는 dmg 에
  반영된 적 없음**.
- 수정: 스크립트에서 번들 투입 전 `codesign --force --sign -` + verify (근원).
  publish.yml 은 주석 정정만. **다음 릴리스(v3.0.10)부터 유효** — 이 맥 설치본·앱데이터
  dylib 은 수동 재서명해둠.
- 재현·검증: 재서명 전 벤치 EXIT 137(SIGKILL) ↔ 재서명 후 완주 EXIT 0. 회귀 227 passed.

### (B) UI/UX 트랙 — 10건 확정·반영 (`cfb17c6`)

사용자 확정: "전부 반영, 시맨틱은 현 엔진에서 불필요"(재노출 안 함 — ca0bc2d 방침 유지).
반영: ①도움말↔실UI 불일치 정정(하이브리드/시맨틱 설명 삭제) ②연산자 발견성(placeholder
예시+0건 힌트) ③0건 제안 칩 실동작 버튼화 ④파일명 클릭 버블 수정(flat `SearchResultItem`
+grouped `GroupedSearchResultItem` — grouped 는 외곽 래퍼 selectForPreview 버블이 원인)
⑤인덱싱 중 0건 "아직 읽는 중" 분기(진행 수치) ⑥실패 리포트 StatusBar "실패 N건" 재열람
⑦검색 소요시간 배지 프로덕션 노출 ⑧시맨틱 비노출 유지 ⑨결과 클릭→미리보기 청크 점프
(citationJump 배관 재사용, 다중 오프셋 앵커, 실패 시 기존 동작) ⑩투어 6단계 문구 정정.
검증: tsc 0·vite build OK·dev 실기동 스크린샷(placeholder/0건 버튼·힌트/21ms 배지/미리보기
연동). **⑨ 점프 스크롤의 실물 확증과 ⑤⑥ 재현 확인은 잔여**(사용자 화면 사용 중이라
자동 조작 중단).

### (B-후속) 같은 날 사용자 피드백 루프 추가분 (전부 커밋·푸시)

- `f05870c` 홈 화면 최근 검색·최근 작업한 문서 **우클릭 "목록에서 삭제"**
  (최근 문서는 `remove_recently_opened_document` 커맨드 신설 — last_opened_at 만 클리어).
- `316737d` placeholder 연산자 나열이 "불친절" 피드백 → 원복하고 검색창 우측
  **? 버튼** → 도움말 '검색 모드' 탭 직행 (HelpModal `initialSection` prop 신설).
  0건 화면 연산자 나열 줄도 제거.
- `3db50ff`+`2b872e7` 사이드바 위계: 카운트 톤다운, 빈 안내문 11px·연톤 통일,
  북마크 헤더 통일, 헤더 sidebar-muted 상향, 섹션 hairline 구분선.
  추천 폴더 이모지(3D) → lucide Folder/HardDrive.

### 잔여 (다음 기회, 대부분 실물 눈 확인 2~3분)

- T1-2 카드 확장/축소 스크롤 앵커, T3-6 Ctrl+F 색칠(맥+윈도우), T3-1 더보기 수백 개 체감.
- ⑨ 점프 실물(중간 청크 카드 클릭 → 미리보기 해당 위치+cite-flash), ⑤ 인덱싱 중 0건 화면,
  ⑥ 실패 N건 버튼(실패 파일 있는 폴더 추가로 재현).
- **v3.0.10 릴리스** 하면 dylib 수정+UX 10건이 배포본에 들어감 (`./scripts/release.sh 3.0.10`).
- UI/UX 관찰만 기록(미반영): 벡터 배너↔StatusBar 이원화(시맨틱 재노출 시에만 유의미),
  Ctrl+F 발견성(툴바 버튼 존재로 낮은 우선), 설정 전용 단축키 없음.

---

## 🔖 (완료됨) 7차 세션 프롬프트 — 기록용

```
docufinder 7차 세션: v3.0.9 실검증 + 사용성(UI/UX) 리뷰. HANDOFF-perf-review.md 의
"✅ 6차 리뷰 최종 로그", "🧪 7차 실검증 체크리스트", "🎨 7차 UI/UX 트랙" 읽고 시작.
6차 성능 리뷰는 전 항목 커밋·릴리스 완료(v3.0.9).

진행: (A) 실검증 체크리스트 1~5 순서대로 — 코드 수정 아니라 실측·실사용 확인, 이상
발견 시에만 수정. (B) UI/UX 트랙 — 앱을 실사용 시나리오로 돌려보며 마찰 지점을 수집해
"관찰→개선안(공수·효과)→사용자 확정→구현" 순서로. 개선안 목록은 구현 전에 반드시
사용자에게 보여주고 우선순위 확정받을 것 (임의 대공사 금지).

공통: 커밋은 main 직접, 커밋 전 cargo fmt --check 필수(CI가 fmt 게이트).
스크롤 로직 절대 불변. 세션 끝에 HANDOFF에 결과 기록.
```

## 🧪 7차 실검증 체크리스트 (앱 구동 필요 — 어느 머신이든, 실사용 환경은 윈도우)

1. **스모크**: `pnpm tauri:dev` (또는 v3.0.9 설치본) — 인덱싱→검색→미리보기 정상.
   이 맥북은 첫 구동 시 `seed_bundled_models` 가 모델 seed → 벡터 경로도 열림.
2. **T3-6 Ctrl+F**: 미리보기 찾기 — 하이라이트 표시/활성 이동/카운트. CSS Custom Highlight
   API 라 WebView2(윈도우)·WKWebView(맥 Safari 17.2+) 지원 확인이 핵심. 미지원 증상 =
   카운트·이동은 되는데 색칠만 없음.
3. **T1-2 스크롤**(6차 이월): 검색결과 카드 확장/축소 시 스크롤 앵커 정상.
   이상하면 58a4685 의 프론트 부분만 되돌림.
4. **임베딩 실측**: ①하이브리드 검색 search_time_ms (T2-2 병렬 — 벡터 인덱싱된 폴더에서),
   ②벡터 인덱싱 처리량 (T2-4 스트림 배칭 + T3-3 병렬 토큰화 — 작은 파일 다수 폴더로,
   [VectorWorker] 로그 관찰). 비교 기준 없으면 절대치만 기록해도 됨.
5. **T3-1 판단**: "더보기" 수백 개에서 스크롤 버벅임 실측될 때만 content-visibility 검토
   (그 외 손대지 않음). T3-2(kordoc persistent worker)는 별도 레포라 범위 밖.

## 🎨 7차 UI/UX 트랙 — 사용자 편의·사용성 증대

**방식**: 실사용 시나리오(아래)를 직접 돌려보며 마찰을 수집 → 개선안을 공수(S/M/L)·효과와
함께 목록화 → **사용자 우선순위 확정 후** 구현. 기존 디자인 시스템(DESIGN.md, styles/variables.css
토큰) 준수 — 새 시각 언어 도입 금지, 기능·흐름 개선 위주.

**점검 시나리오**:
- **첫 실행 흐름**: 온보딩 투어 → 폴더 추가 → 인덱싱 대기 — 지금 뭘 해야 하는지가 화면만
  보고 자명한가. 인덱싱 중 검색 시도 시 안내가 있는가.
- **검색 핵심 루프**: 입력→결과→미리보기→파일 열기. 검색 연산자(`ext:` `-제외` `~N` 등)
  발견 가능성(힌트/예시), 0건일 때 다음 행동 안내(맞춤법·연산자·폴더 스코프 제안),
  모드(키워드/시맨틱/하이브리드) 차이가 사용자에게 이해되는가.
- **미리보기**: Ctrl+F 발견 가능성, 스니펫→본문 위치 점프 정확도, 긴 문서 탐색 편의.
- **인덱싱 피드백**: 진행률·완료·실패가 상태바만으로 충분한가, 실패 파일 목록 접근성.
- **자주 쓰는 동선 단축**: 최근 검색/즐겨찾기 폴더/스마트 폴더 — 실제로 손이 가는 위치인가,
  키보드 단축키 커버리지(팔레트 Ctrl+K 포함).
- **설정**: 검색 결과 수·시맨틱 토글 등 자주 만지는 항목이 묻혀 있지 않은가.

**산출물**: HANDOFF에 "관찰 → 개선안(공수·효과) → 결정" 표 + 확정분 구현 커밋.

---

## ✅ 6차 리뷰 최종 로그 (맥북, 2026-07-03) — 전 항목 완료

계획된 Tier 2·3 전체 + 릴리스까지 완료. 커밋(전부 main 푸시됨):

- `8fdefa2` T2-2 FTS∥임베딩 rayon 병렬 + 실DB gated 벤치(perf_bench.rs).
  실DB(803청크) FTS 질의 0.17~0.33ms 실측 — 이득 = min(t_fts, t_embed+t_vec), 코퍼스 비례.
- `5af9625` T2-3 resume 경로 스트리밍(visitor). **SQL LIKE 금지** — 이슈 #34 회귀라 normalize
  판정을 행 콜백으로 이동만.
- `6581350` T2-5 parse_file 전문 content 미적재 (NEIS 통합테스트 청크 기준으로 조정).
- `d3dcd70` T2-6 classify_documents 배치 커맨드 (단건 커맨드 제거).
- `1441a45` T2-4 vector_worker 파일경계 제거(CompletionTracker+유닛테스트4) / T1-6 embed_batch
  전환 4곳(normalize_text 동일적용 — embed()가 내부에서 하던 것). fresh-context 검증 PASS.
- `cb54055` T3-3 형태소 토큰화 파싱 풀 이동 (thread_local Lindera, 인스턴스당 dict.da ~23MB,
  런 전용 풀이라 종료 시 회수. 토크나이저 초기화 실패 시 컨슈머 인라인 폴백).
- `16c031d` T3-4 스키마 v17 부분인덱스(last_opened_at/size) / T3-5 SSD 파서 스레드 RAM 16GB↑
  min(8) (total_memory_mb 유틸 container→utils/disk_info 이동).
- `d03687a` T3-6 Ctrl+F를 CSS Custom Highlight API 로 (재파싱 0, React 재조정과 무충돌).
  ::highlight 는 border 미지원이라 밑줄로 이식.
- `014e6d0` style: cargo fmt (릴리스 게이트 선행).

**의도적 스킵**: T3-1(리스트 렌더 — 실측 후 조건부, 스크롤 로직 불변 지시),
T3-2(kordoc persistent worker — 별도 레포 큰 작업).

**남은 실측 숙제 (다음 기회에)**:
- T1-2 프론트 스크롤 실테스트 (`pnpm tauri:dev` 카드 확장/축소 앵커 확인) — 여전히 미완.
- 임베딩 실 latency (T2-2 벡터경로 / T2-4 배칭 이득) — 모델 seed 된 머신(윈도우 개발기)에서.
  이 맥북은 모델 未seed. FTS-only 벤치는 `DOCUFINDER_BENCH_DB=<실DB경로>` 로 실행 가능.
- T3-6 실사용 확인 — WebView2 에서 찾기 하이라이트/이동/카운트 동작 (지원 API 라 이론상 OK).

---

## 🆕 6차 진행 로그 (맥북, 2026-07-03) — T2-2 코드완료·미커밋

**T2-2 (FTS∥임베딩 rayon 병렬) 구현 완료.** `hybrid.rs:search_hybrid_impl` — 순차 FTS→벡터를
설계대로 `rayon::join` 두 클로저로 병렬화. 각 클로저가 풀에서 conn 따로 획득(Connection !Sync),
join 이후 RRF·enrich 용 conn 재획득. `display_query` 는 join 후 262행에서 재사용하므로 벡터
클로저가 **빌려** 씀(move 금지). `KeywordMode: Copy` 확인, 두 클로저 non-move.
- **검증 완료**: `cargo check` OK, `cargo clippy --lib -- -D warnings` **0**, 회귀 **222 passed 0 failed**
  (test 바이너리 직접 실행). rayon::join **배관은 222테스트가 이미 실행** — FTS-only 경로에서 두
  클로저 다 돌고(벡터는 embedder=None → `_ => Ok((vec![],None))`), RRF 병합까지 탐.
- **결과 동일성은 구조적 보장**: FTS·벡터 로직 그대로, 정렬은 완료순서 아닌 RRF 점수순(결정적).
  이 변경이 **새로 도입하는 임베더 스레드 위험 없음** — embed 는 검색당 1회, 이미 spawn_blocking
  워커서 호출돼 옴(Arc<Embedder> 동시검색 공유). 진짜 새 동작은 conn 2개 동시 체크아웃뿐(풀16 무해).
- **상태**: `git status` = `M hybrid.rs` **미커밋**(사용자 "멈춰"로 중단, 커밋 요청 없었음).
  /model 전환은 같은 작업트리라 이 수정 그대로 유지됨.

**⚠️ 이 맥북 실태 (핸드오프 가정과 다름 — 중요)**:
- 임베딩 모델 `model_int8.onnx` **未seed** — `~/Library/Application Support/com.anything.app/models/
  kosimcse-roberta-multitask/` 에 `libonnxruntime.dylib`(33M)만, onnx·tokenizer.json 없음.
- 벡터인덱스 **부재** — `data_dir/vectors.usearch`(+`.map`) 파일 없음(앱이 이 맥서 완전구동된 적 없음).
- 실 DB 는 있음 — `docufinder.db` 4.9M, **chunks 803개, FTS 테이블 완비**(FTS 경로는 실측 가능).
- 모델 확보법: HF `chrisryugj/kosimcse-roberta-multitask-onnx` 의 `model_int8.onnx`+`tokenizer.json`
  다운로드(URL·SHA256 은 `model_downloader.rs:41-64`). ort 는 load-dynamic → `ORT_DYLIB_PATH` 를
  위 dylib 로 set 하면 test 서 Embedder::new 로드 가능. 벡터인덱스는 803청크 embed 해서 새로 빌드 필요.
- **판단**: 실 벡터 latency 측정은 모델DL+인덱스빌드 필요(중간 비용). embed 지배 가정상 병렬이득
  ≈ t_fts(FTS 가 embed 뒤에 숨음). t_fts 실측만으로도 이득 정량화 가능 —
  `SearchService::new(db, None,None, Some(Lindera), None)` 로 embedder=None 하이브리드를 실DB에
  돌리면 실 FTS·RRF·enrich·rayon 다 타는 end-to-end 측정됨(env `DOCUFINDER_BENCH_DB` gated perf test
  로 만들면 committable). 다음 세션서 비용 대비 판단해 진행.

## ✅ 완료 (커밋·푸시됨, main)

- **Phase 0 — 계측 하니스**: `src-tauri/src/perf_bench.rs` (`#[cfg(test)]`, 릴리스 미포함).
  토큰화 마이크로벤치 3종. lib.rs에 `#[cfg(test)] mod perf_bench;` 등록.
- **Phase 1 (Tier 1)** — 커밋 `58a4685`:
  - T1-1 토큰화 dedup O(n²)→HashSet (lindera_ko.rs). **대형 tokenize 1.71→1.14ms(-33%)**, 회귀 0.
  - T1-2 프론트 onToggleExpand 안정 콜백화 (SearchResultList/Item/Grouped). tsc 0. **스크롤 실테스트 미완**.
  - T1-3 usearch SAVE_INTERVAL 적응 max(1000,size/20) (vector_worker.rs).
  - T1-4 pool MAX_POOL_SIZE stale 주석 정정 (cache_size는 근거 확실해 유지).
  - T1-5 HWPX 문단 clone 제거 3곳 (text_extraction.rs).
  - T1-6 임베딩 배치화 → **Tier 2로 이월**(4곳 흩어짐).
- **T2-1 (검색 spawn_blocking)** — 커밋 `[이 핸드오프 커밋 직전 해시, git log 참조]`:
  - search_service/{hybrid,keyword,semantic,smart}.rs: **async fn 12개 sync 강등**(.await 제거, 진짜 async 0).
  - commands/search.rs: 검색 커맨드 6개 `spawn_blocking(move|| service.search_*).await??`.
  - commands/ai.rs: RAG search_hybrid 4곳 .await만 제거(sync 인라인, 래핑 보류 — 아래 주의).
  - 검증: cargo test --lib **222 passed 0 failed**, clippy -D warnings **0**.

## 🔧 검증 방법 (중요 — harness 우회)

harness가 `cargo test` 출력을 "N passed" 요약으로 정규화해서 `--nocapture` eprintln을 삼킨다.
**test 바이너리를 직접 실행**해야 perf 수치가 보인다:
```sh
source ~/.cargo/env && cd ~/workspace/docufinder/src-tauri
cargo test --release --lib --no-run 2>/dev/null
BIN=$(ls -t target/release/deps/docufinder_lib-* | grep -v '\.d$' | head -1)
"$BIN" perf_ --nocapture --test-threads=1          # perf 벤치
"$BIN" --skip perf                                  # 전체 회귀 (222 passed 기준)
```
- macOS라 `#[cfg(windows)]` 코드는 로컬 컴파일 제외(CI가 최종 검증).
- 통합/pool 테스트 race 주의 — 이미 단일 test 순차화됨.

---

## ▶️ 다음 = T2-2 (FTS∥임베딩 rayon 병렬) — 설계 완료, 맥북서 구현·검증

**파일**: `src-tauri/src/application/services/search_service/hybrid.rs` → `search_hybrid_impl` (119행~)
**의도**: FTS(conn만)와 벡터(emb.embed[ONNX,지배적]+vi.search)가 독립인데 순차 실행(총지연 = FTS+embed+vec).
`rayon::join`으로 병렬화하면 ≈ max(FTS, embed+vec) — FTS를 임베딩 뒤에 숨긴다.

**핵심 제약**: rusqlite `Connection`은 `!Sync` → rayon 두 클로저가 `&conn` 공유 불가.
각 클로저가 풀에서 conn을 **따로 획득**한다(MAX_POOL_SIZE=16이라 여유). `&self`는 Sync(Arc 필드).

**타입**: `emb.embed()` → `Vec<f32>`, `vi.search()` → `Vec<VectorResult>`(crate::search::vector::VectorResult).
rayon 1.10 의존성 이미 있음.

**설계 (126행 `let conn=...` 제거하고 아래로)**:
```rust
// FTS ∥ 벡터 병렬 (임베딩 ONNX가 지배적이라 FTS를 뒤에 숨김).
// Connection 은 !Sync 라 각 클로저가 풀에서 conn 을 따로 빌린다.
let (fts_res, vec_bundle) = rayon::join(
    || -> AppResult<Vec<fts::FtsResult>> {
        let conn = self.get_connection()?;
        match op { /* 135-163 기존 FTS 로직 그대로, ? 대신 map_err 반환 */ }
    },
    || -> AppResult<(Vec<crate::search::vector::VectorResult>, Option<Vec<f32>>)> {
        let vector_fetch_limit = if folder_scope.is_some() { max_results*3 } else { max_results };
        match (self.embedder.as_ref(), self.vector_index.as_ref()) {
            (Some(_),Some(_)) if display_query.trim().is_empty() => Ok((vec![], None)),
            (Some(emb),Some(vi)) => match emb.embed(&display_query, true) {
                Ok(qe) => {
                    let raw = vi.search(&qe, vector_fetch_limit).unwrap_or_default();
                    let results = if folder_scope.is_some() && !raw.is_empty() {
                        let conn = self.get_connection()?;  // path_map 전용 conn
                        /* 179-190 folder_scope 필터 그대로 */
                    } else { raw };
                    Ok((results, Some(qe)))
                }
                Err(e) => { tracing::warn!("embed 실패: {}", e); Ok((vec![], None)) }
            },
            _ => Ok((vec![], None)),
        }
    },
);
let fts_results = fts_res?;
let (vector_results, query_embedding) = vec_bundle?;
let conn = self.get_connection()?;   // 이후 RRF(231 vector_only)·enrich(351/352)용 재획득
// ── 204행 이후(RRF 병합, enrich) 기존 그대로 ──
```
**주의**: rayon::join은 커맨드의 spawn_blocking 스레드 안에서 rayon 전역풀 사용 → 인덱싱 rayon과
경합 가능하나 검색이 짧아 무해. conn 최대 3개 동시(FTS/벡터/메인) — 풀 여유.

**검증(맥북, 모델 有)**:
1. FTS-only 회귀: `"$BIN" --skip perf` 222 유지.
2. 실검색 latency before/after — 앱(`pnpm tauri:dev`)에서 벡터 인덱싱된 폴더 검색, search_time_ms 로그 비교.
3. 하이브리드 결과가 병렬화 전과 동일한지(순서/개수) 실검색으로 확인.

---

## 📋 남은 항목 (계획 원본: `.claude/plans/perf-review-2026-07-03.md` — gitignore라 맥미니 로컬만)

### Tier 2 (구조 개선)
- **T2-3 resume 메모리 스트리밍** — `indexer/pipeline.rs:305`, `db/mod.rs:253`.
  `get_all_fts_indexed_paths()`가 전 DB 경로를 Vec<String> 로드 → 폴더 스코프
  `path LIKE <folder>% AND fts_indexed_at IS NOT NULL` query_map 스트리밍 HashSet 으로. (1)만으로 피크 절반. **로컬 검증 가능**.
- **T2-4 임베딩 배치 파일경계 제거** — `indexer/vector_worker.rs:340`. 프리페치 파일단위→청크 스트림, 32 채워 embed_batch. 모델 필요.
- **T2-5 content 미적재** — `parsers/mod.rs:84`, `pipeline.rs:978`. 배치 경로서 doc.content 미적재(chunks만). total_chars는 chunks 길이합. **로컬 검증 가능**(파서).
- **T2-6 useDocumentCategories 배치 커밋** — `hooks/useDocumentCategories.ts:24`. Promise.all 단일 setCategories + 백엔드 배치 커맨드. **로컬(tsc) 검증 가능**.
- **T1-6 임베딩 배치화**(이월) — `indexer/duplicate.rs:290`, `indexer/lineage.rs:324,524`, `commands/lineage.rs:278`. 단건 embed→embed_batch. 온디맨드.

### Tier 3 (대공사)
- **T3-1 리스트 렌더** — ⚠️ **JS 가상화 폐기**(사용자 지시: 스크롤 버그). `content-visibility:auto`만,
  그것도 "더보기 수백개 스크롤 버벅임" 실측 후 결정. 스크롤 로직 절대 불변.
- **T3-3 Lindera 토큰화 병렬 프로듀서** — `pipeline.rs:706`. 단일 컨슈머 병목 → 병렬 파싱단계로.
  thread_local Segmenter 필요(&mut Mutex 직렬). 모델 무관, 로컬 가능.
- **T3-4 v17 부분인덱스** — `db/mod.rs:1121`. last_opened_at/size 인덱스(홈 "최근 문서" 풀스캔). additive 마이그레이션.
- **T3-5 파서 스레드 상한** — `utils/disk_info.rs:173`. SSD min(4)→min(8), RAM 조건부(저사양 회귀 주의).
- **T3-6 Ctrl+F DOM 하이라이트** — `PreviewPanel.tsx:755`. 찾기어마다 마크다운 재파싱 → TreeWalker DOM.
- **T3-2 kordoc persistent worker** — 별도 레포(chrisryugj/kordoc). 파일당 Node 콜드스타트. 큰 작업.

### Phase 4 — 검증 + 릴리스 v3.0.9
버전 3곳 bump(package.json, Cargo.toml, tauri.conf.json) + CHANGELOG + main push + `./scripts/release.sh 3.0.9`.
release.sh가 pnpm build + cargo fmt/check/clippy(-D)/test 전체 게이트.

### Tier 4 — 건드리지 말 것 (의도적 트레이드오프)
- 청크 본문 이중저장(chunks.content + chunks_fts): 형태소 재현율/미리보기용. 재색인 고위험.
- 릴리스 프로파일(Cargo.toml codegen-units=4, lto=thin): CI 빌드 메모리 위한 양보. 주석 근거.

## ⚠️ 미해결/주의
- **프론트 T1-2 스크롤 실테스트 미완**: `pnpm tauri:dev` → 검색결과 카드 확장/축소 눌러 스크롤 앵커
  정상인지. 스크롤 로직(handleToggle*)은 안 건드렸고 prop 배선만 바꿈. 이상하면 되돌림(58a4685 프론트 부분).
- **ai.rs RAG search_hybrid**: sync 인라인(워커 블로킹 잔존). LLM 대기 지배적이라 보류. 필요시 통째 spawn_blocking(return→Result 리팩터 필요).
- 임베딩 모델(KoSimCSE model_int8.onnx)은 첫 앱 실행 시 `seed_bundled_models`가 resources→models_dir 복사.
  맥북서 앱 한 번 띄우면 seed됨(그래야 벡터 경로 벤치/검증 가능).

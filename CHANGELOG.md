# Changelog

## [2.5.13] - 2026-04-23

**네트워크(UNC) 폴더 재인덱싱 / 배치 인덱싱 예방 패치** — [이슈 #19](https://github.com/chrisryugj/Docufinder/issues/19)

### 수정
- **네트워크 폴더 재인덱싱·Resume·배치 인덱싱 실패 가능성 차단** — v2.5.0 에서 `add_folder` 는 `dunce::canonicalize` 로 `\\server\share\...` 형태를 보존하도록 바꿨는데, `reindex_folder` / `resume_indexing` / `start_indexing_batch` / `run_folder_index_job_batch` 네 경로는 여전히 `std::fs::canonicalize` 를 써서 UNC 를 `\\?\UNC\server\share\...` 로 변환하고 있었음. 이 경우 DB 에 기록된 감시 경로(`\\server\share\...`)와 불일치해 "변경분 0건" 으로 오인식되거나 status 업데이트가 엉뚱한 키로 저장되는 문제가 있었음. **네 경로 모두 `dunce::canonicalize` 로 통일.**

### 알려진 제약 (다음 라운드)
- 매핑드라이브(Z:\, X:\ 등)는 여전히 로컬로 취급됨 (PollWatcher 분기 안 탐). SMB 매핑드라이브에서 실시간 이벤트 누락 가능 — `GetDriveTypeW` 기반 분기 검토 예정.
- AI 질의응답(Gemini) 은 여전히 인터넷 필요. 망분리 환경에서는 검색/파일명/벡터 기능만 동작 (ONNX·PaddleOCR·Lindera 는 이미 MSI 에 번들되어 오프라인 OK).

---

## [2.5.12] - 2026-04-23

**안정성 / 종료 / 정렬 크래시 대응 라운드**

### 수정
- **인덱싱 중 트레이 "종료" 무반응** — 트레이 quit / X 버튼(close_to_tray=false) 핸들러가 FTS cancel 신호를 보내지 않아 파이프라인 스레드가 그대로 돌던 문제. 이제 cancel_indexing + vector worker cancel 을 즉시 broadcast 하고, cleanup 이 3초 내 끝나지 않으면 **watchdog 이 std::process::exit(0) 으로 강제 종료**. 인덱싱 중에도 트레이 우클릭 → 종료가 즉시 동작.
- **트레이 최소화 토글이 꺼지지 않던 버그** — 설정 모달의 `handleChange` 가 stale state 로 `setSettings` 를 호출해, "트레이 최소화" 토글이 `close_to_tray` + `start_minimized` 를 같은 틱에 업데이트할 때 뒤 호출이 앞 호출을 덮어썼음. functional update (`setSettings(prev => ...)`) 로 교체.
- **"모든 데이터 초기화" 첫 시도 지연 (~수 초)** — FTS 파이프라인의 잔존 WAL read lock 이 DROP TABLE 과 경쟁해 발생. 취소 신호를 **맨 먼저** broadcast + 200ms 유예 + `db::pool::drain_pool()` 후 DROP. 첫 시도도 즉시 완료.
- **smallsort "total order" 패닉** — Rust 1.81+ sort 가 엄격한 전이성을 요구하는데 `partial_cmp().unwrap_or(Equal)` 패턴은 NaN 섞이면 전이성 위반. 검색 랭킹 / 중복 유사도 / 교정 후보 / OCR 바운딩박스 정렬 5곳을 모두 `f32::total_cmp` / `f64::total_cmp` 로 교체.
- **`type1-encoding-parser` / `cff-parser` / `tao` 관련 알림 스팸** — 손상된 PDF Type1 폰트 / 앱 종료 시 Windows 이벤트 루프 race 에서 발생하는 패닉이 crash log / Telegram 으로 전송되던 문제. `BENIGN_PANIC_SOURCES` 에 추가해 알림만 억제 (해당 패닉은 이미 `catch_unwind` 또는 종료 시점이라 앱 동작에는 영향 없음).

### 개선
- **PDF 수식 OCR 설정을 "검색" 탭으로 이동** — 이전에는 "진단" 탭에 있어 발견이 어려웠음. 빨간 경고 배너로 "PDF 인덱싱 속도가 수 배 ~ 수십 배 느려질 수 있음" 을 강조.
- **크래시 로그 재전송 스팸 차단** — 앱 시작 시 미전송 crash log 를 Telegram 으로 플러시하는 경로에서, **오늘자 `crash-YYYY-MM-DD.log` 만 전송**하고 이전 날짜 로그는 조용히 `.sent` 로 마킹. 재설치 사용자의 묵은 로그가 채널로 올라오던 문제 해결.

### 내부
- `cleanup_vector_resources` 가 FTS 파이프라인도 cancel 하도록 확장 (기존엔 벡터 워커만).

---

## [2.5.11] - 2026-04-23

**kordoc v2.6.2 반영 — PDF 수식 OCR 품질 대폭 개선 (90%+ 정확도)**

### 개선
- **PDF 수식 OCR noise 제거 대폭 강화** — [kordoc v2.6.1](https://github.com/chrisryugj/kordoc/releases/tag/v2.6.1) + [v2.6.2](https://github.com/chrisryugj/kordoc/releases/tag/v2.6.2) 반영. arxiv Attention 논문 기준 순수 noise 1개만 남아 **96% 정확도** 달성. ResNet (Figure 많은 논문) 기준 **90%**. 핵심 수식 100% 유지.
  - **trivial 수식 필터 12개 규칙 추가** — 단일 글자 (`$O$`, `$a$`), 단일 `\cmd` (`$\imath$`, `$\varPi$`), 장식 `\mathrm{...}` (`$\mathrm{fcloc}$`), 반복 기호 (`$\pm\pm\pm\pm$`), substring 반복 (`\alpha_{N}=` 연쇄), `\square` placeholder, 단독 숫자, 괄호 그룹 중복, 함수 인자 반복, `\frac{X}{X}`, matrix placeholder, `\mathsf`/`\mathtt`/`\texttt` 등.
  - **MFR tokenizer 과공백 정규화** — `\mathrm { m o d d }` → `\mathrm{modd}`, `6 4` → `64`, `( Q, K, V )` → `(Q,K,V)`.
  - **`\cmd` 뒤 공백 누락 복원** — `\cdotd` → `\cdot d`, `\timesd_{k}` → `\times d_{k}` (알려진 LaTeX 명령어 사전 기반).
  - **수식 bbox y 좌표 매핑** — 이전엔 검출된 수식이 페이지 끝에 몰렸는데, 이제 pdfjs 블록 사이 **올바른 위치**에 삽입. MultiHead/FFN/PE 수식이 논문 흐름에 맞게 배치.
  - **pdfjs 중복 블록 제거** — 수식 bbox 와 60%+ 겹치는 pdfjs 텍스트 블록 자동 삭제. 동일 수식이 두 번 나타나던 현상 해결.
  - **`cleanPdfText` 수식 라인 공백 보호** — `collapseEvenSpacing` 이 수식 내부 LaTeX 공백을 "균등배분" 으로 오인식해 `\cdot d` → `\cdotd` 로 합쳐지던 숨은 버그 수정.

### 의존성
- `kordoc` 2.6.0 → 2.6.2 (번들 재빌드)

---

## [2.5.10] - 2026-04-23

**kordoc v2.6.1 (수식 OCR 품질 개선) 반영**

### 개선
- kordoc v2.6.1 초기 반영. v2.5.11 에서 더 다듬어진 상태로 배포되었으므로 상세 내역은 v2.5.11 항목 참고.

### 의존성
- `kordoc` 2.6.0 → 2.6.1

---

## [2.5.9] - 2026-04-23

**kordoc v2.6.0 (PDF 수식 OCR) + KaTeX 미리보기**

### 추가
- **PDF 이미지 기반 수식 OCR** — kordoc v2.6.0 의 [Pix2Text MFD + MFR](https://github.com/breezedeus/pix2text) ONNX 모델 연동. 스캔 PDF / 이미지 삽입 수식이 자동으로 LaTeX (`$...$`, `$$...$$`) 로 추출되어 인덱싱 + 검색 대상 포함. 기본 활성화.
- **검색 결과 수식 KaTeX 미리보기** — 결과 뷰어에서 LaTeX 수식을 KaTeX 로 즉시 렌더. 인라인 `$...$` 와 display `$$...$$` 모두 지원.
- **수식 4포맷 지원** — HWPX / DOCX / HWP5 의 수식(EQN 블록) 도 kordoc 2.5.3 이상에서 LaTeX 로 변환되어 검색 가능.

### 의존성
- `kordoc` 2.5.2 → 2.6.0 (ort, sharp, @huggingface/transformers, @hyzyla/pdfium 추가)

---

## [2.5.8] - 2026-04-23

**자동 업데이트 시스템 도입 + Telegram 오류 리포트 + 진단 탭**

### 추가
- **GitHub Releases 기반 자동 업데이트** — ed25519 서명 검증 포함. 앱이 최신 버전 감지 시 배지 노출, "지금 확인" 버튼으로 수동 체크도 가능. Tauri updater plugin.
- **진단 탭 (설정 > 진단)** — 앱 상태 / DB / 인덱스 / 모델 위치 / 로그 한눈에 확인. 사용자 문의 대응 편의.
- **Telegram 오류 리포트** — 크래시 / panic 발생 시 사용자 동의하에 Telegram 채널로 비식별화된 스택 전송 (opt-in).

### 개선
- 다양한 UX 폴리싱 (세부는 커밋 [eac8858](https://github.com/chrisryugj/Docufinder/commit/eac8858) 참고)

---

## [2.5.7] - 2026-04-22

**kordoc 2.5.2 반영 — macOS 한컴 HWPX 호환 + HWP5 배포용 COM fallback**

### 개선
- **번들 kordoc 파서 2.5.0 → 2.5.2 업그레이드** — Docufinder 가 내부적으로 쓰는 Node.js 사이드카 파서([kordoc](https://github.com/chrisryugj/kordoc)) 를 최신으로 교체. HWP/HWPX 변환 품질이 조용히 개선됨. 앱 UI 변경 없음.
  - **macOS 한컴오피스에서 "파일 깨짐" 거부되던 HWPX 생성 이슈 해결** — `markdownToHwpx` 가 만드는 HWPX 의 테이블 XML 을 최소 스켈레톤에서 **완전 스펙 형태**로 재작성. `<hp:tbl>` 필수 속성, `<hp:sz>`/`<hp:pos>`/`<hp:outMargin>`/`<hp:inMargin>` 블록, `<hp:subList>` 래퍼 + `<hp:cellAddr>`/`<hp:cellSpan>`/`<hp:cellSz>`/`<hp:cellMargin>` 추가. `Preview/PrvText.txt` 동봉. (kordoc #4)
  - **테이블 테두리 / 볼드 / 순서 있는 목록 시각 품질 개선** — 테두리 단위 공백 포함(`"0.12 mm"`), 볼드 전용 fontface(HY견고딕/Arial Black) id=2 추가, indent 레벨별 러닝 카운터로 `1. 2. 3.` 자동 번호 정상 동작. (kordoc #4 후속)
  - **HWP 5.x "배포용 문서 상위 버전" 경고 플레이스홀더 COM 재시도** — `.hwp` 바이너리에서 `"이 문서는 상위 버전의 배포용 문서입니다..."` 로만 떨어지는 케이스에서, Windows + 한컴오피스 환경이면 자동으로 `HWPFrame.HwpObject` COM API 로 재시도. 기존 HWPX DRM fallback 인프라 재활용. (kordoc #25)
- **PDF 세로선 없는 표 오인식 수정** — 세로선 없는 표를 1 열 다행 그리드로 잘못 잡아 **본문이 한 줄에 평평화(flatten) 되어 표시되던 현상** 수정. 검색 결과 스니펫 품질 체감 개선. (kordoc fix)

### 의존성
- `kordoc` 2.5.0 → 2.5.2 (번들 재빌드)

---

## [2.5.6] - 2026-04-22

**인덱싱 중 강제 종료 예방 + 관련도→최신순 전환 시 스크롤 튀는 버그 수정**

### 수정
- **관련도순 → 최신순/이름순/오래된순 전환 시 스크롤이 중간으로 튀는 버그** — 정렬을 바꾸면 [useResultSelection](src/hooks/useResultSelection.ts) 이 "선택된 파일의 새 index" 로 `selectedIndex` 를 자동 재매핑하는데, [SearchResultList](src/components/search/SearchResultList.tsx) 의 `scrollIntoView` 가 이걸 "사용자가 새 항목을 선택한 것" 으로 오해해서 해당 위치로 스크롤. 결과적으로 "정렬이 안 먹히는 것 같다" 는 체감 이슈도 같이 유발. 선택된 **파일 경로** 를 `lastScrolledPathRef` 에 기록해 두고, 경로가 그대로면 (= 같은 파일이 index 만 바뀐 재매핑이면) `scrollIntoView` 를 건너뛰도록 수정. 키보드 내비게이션 (다른 파일로 이동) 은 기존 그대로 동작.
- **이미지 기반(스캔) PDF 다수 폴더 인덱싱 중 앱 강제 종료 예방** — Downloads 같이 스캔 PDF 가 수백~수천 개 쌓인 폴더에서 kordoc(Node.js) → Rust pdf-extract 순으로 2 중 재시도가 돌아가면서 Node.js 자식 프로세스 spawn + pdf-extract 메모리 사용이 누적되어 OOM 으로 OS 가 앱을 강제 종료. [parsers/kordoc.rs](src-tauri/src/parsers/kordoc.rs) stderr 에서 "이미지 기반 PDF" 마커를 에러 메시지에 태그하고, [parsers/mod.rs](src-tauri/src/parsers/mod.rs) 에서 OCR 비활성 + 이미지 PDF 조합이면 Rust 재시도를 건너뛰고 메타데이터만 저장하도록 조기 분기. OCR 활성 상태에서는 기존대로 Rust 파서 + OCR fallback 이 돌아가 기능 손실 없음.

### 개선
- `CHANNEL_BUFFER_SIZE` 32 → 16 ([pipeline.rs](src-tauri/src/indexer/pipeline.rs)). 실제 파싱 스레드는 HDD 2 / SSD 4 개로 제한되어 있어 16 이면 충분한 여유. 저사양 PC (8GB RAM) + 부적합 인덱싱 타깃 조합에서 메모리 피크 절반 수준으로 감소.
- `panic hook` 의 `BENIGN_PANIC_SOURCES` 에 `ort`, `usearch`, `lindera` 추가 ([lib.rs](src-tauri/src/lib.rs)). ONNX Runtime / 벡터 인덱스 C++ 바인딩 / 형태소 사전 로드 중 발생하는 알려진 panic 이 사용자 `crash.log` 를 오염시키던 문제 제거.

---

## [2.5.5] - 2026-04-21

**v2.5.3 이후에도 남아있던 정렬 버그 + 드롭다운 z-index 버그 최종 해결**

### 수정
- **키워드 검색 후 "최신순/이름순" 정렬이 그룹 뷰에서 먹지 않던 문제** — 기본 뷰가 **그룹 뷰** (기존 localStorage 없으면 `"grouped"` 로 초기화) 인데 [useSearch.ts](src/hooks/useSearch.ts) 의 `groupedResults` 가 `filteredResults` (정렬 반영됨)를 받은 뒤 **마지막에 무조건 `top_confidence` 로 재정렬**하고 있어 `filters.sortBy` 가 완전히 무시되던 것. `filters.sortBy === "relevance"` 일 때만 신뢰도 재정렬을 적용하고, 그 외엔 Map 삽입 순서(= filteredResults 의 정렬된 순서) 를 유지하도록 수정. `useMemo` 의존성에 `filters.sortBy` 추가.
- **정렬/확장자/기간 드롭다운 옵션이 결과 카드에 가려져 클릭 불가능하던 z-index 버그** — 결과 카드의 `stagger-item` 애니메이션(`transform: translateY`) 이 카드마다 새 stacking context 를 생성. 처음엔 `SearchFilters` root 자체에만 `relative z-40 isolate` 를 줬지만, **SearchFilters 의 부모 wrapper (App.tsx 의 filter bar 컨테이너) 가 stacking 을 갖지 않아** 효과가 국소화되어 여전히 스크롤 영역(DOM 상 뒤 형제) 이 위로 렌더되던 것이 진짜 원인. [App.tsx](src/App.tsx) 의 filter bar wrapper 에 `relative z-40` 직접 부여. `SearchFilters` / `ResultsToolbar` 의 보조 stacking 도 유지 (내부 드롭다운 보호).

### 내부
- 정렬/신뢰도 관련 주석 보강 — `groupedResults` 가 왜 relevance 에서만 재정렬하는지, Map 삽입 순서가 어떤 의미인지 명시.

> 📝 v2.5.4 는 위 2 버그의 **부분 수정판**으로 내부 빌드만 생성되었고 외부 배포되지 않았습니다. 본 v2.5.5 에 완전 통합.

---

## [2.5.3] - 2026-04-21

**v2.5.2 부팅 CPU/메모리 피크 핫픽스 + 정렬/컨텍스트 메뉴 버그 3종**

### 수정
- **부팅 직후 3~5분 CPU 60%+ / 메모리 1.2GB 피크** — v2.5.2 에서 추가된 주기 sync(`periodic_sync`) 가 startup sync 진행 중에도 창 포커스 복귀마다 재트리거되어 같은 드라이브를 2~3중으로 병렬 파싱/FTS 하던 race condition. 두 계층으로 차단:
  - `is_busy()` 에 `WatchManager::is_paused()` 체크 추가 — startup sync 등 다른 경로가 watcher 를 pause 한 상태면 periodic_sync 는 skip.
  - `run_sync_all` 진입부에 전역 `AtomicBool SYNC_RUNNING` CAS lock + RAII guard — interval / focus 트리거끼리의 중첩 실행 자체를 차단, 패닉/early-return 시에도 자동 해제.
- **키워드 검색 결과 "관련도순 / 최신순" 정렬이 안 먹는 것처럼 보이던 문제** — 내용 매치 섹션에는 정렬이 적용되지만 상단 "파일명 매치" 섹션에는 정렬 로직이 없어 드롭다운 변경이 무반응으로 체감되던 문제. `filteredFilenameResults` 에도 내용 섹션과 동일한 `sortBy` 분기(confidence / date_desc / date_asc / name) 적용.
- **파일명 매치 결과 우클릭 → "폴더 열기" 시 파일도 함께 열리던 문제** — `ResultContextMenu` 가 `createPortal(document.body)` 로 렌더되어 DOM 은 분리되어 있지만 React synthetic event 는 여전히 원래 부모로 버블링된다. `FilenameResultItem` 의 부모 div `onClick` = `onOpenFile` 이 같이 실행되어 파일이 딸려 열림. 메뉴 버튼 4종(파일 열기 / 폴더 열기 / 경로 복사 / 유사 문서) 의 onClick 에 `e.stopPropagation()` 추가.

### 내부
- `WatchManager` 에 `is_paused() -> bool` 공개 API 추가 (기존 `pause_count` 내부 상태 노출).
- `periodic_sync.rs` 상단에 `SYNC_RUNNING` static + `SyncGuard` (Drop 구현) 추가 — 함수 중간에 panic 이 나도 lock 이 풀림.

---

## [2.5.2] - 2026-04-20

**자동 동기화 주기 — watcher 이벤트 누락 보완**

### 추가
- **백그라운드 주기 sync (기본 10분)** — Windows `ReadDirectoryChangesW` 버퍼 오버플로로 notify 이벤트가 누락되어도 최대 10분 안에 새 파일/삭제/수정이 자동 감지된다. 전체 드라이브 감시 시 특히 유용. 배치/벡터 인덱싱 중에는 skip, 실행 전 watcher pause → sync 후 resume 순서로 DB 락 경쟁 회피.
- **창 포커스 복귀 즉시 sync** — 앱을 잠시 벗어났다 돌아오면(`onFocusChanged`) 마지막 sync 로부터 2분 이상 경과했을 때 즉시 재정합. 다른 창에서 파일 복사 후 바로 검색하는 흐름이 자연스러워짐.
- **설정 > 시스템 > 성능 > "자동 동기화 주기"** — 끄기 / 5분 / 10분(기본) / 30분 선택. 0(끄기)로 두면 주기 sync 와 포커스 sync 모두 비활성.

### 변경/개선
- `AppContainer` 에 `last_sync_at`(AtomicI64) + `sync_shutdown`(AtomicBool) 추가 — 주기 task 종료 신호 공유.
- 앱 종료 시 `cleanup_vector_resources` 가 sync shutdown 을 먼저 세팅하여 task 가 최대 60초 내 탈출.
- 신규 Tauri 커맨드 `trigger_sync_if_stale(min_elapsed_secs)` — 프론트가 호출, 응답 즉시 반환(block 없음).
- 변경분 발견 시 `periodic-sync-updated` 이벤트 emit + FilenameCache 자동 재로드.

### 내부
- 신규 모듈 [src-tauri/src/indexer/periodic_sync.rs](src-tauri/src/indexer/periodic_sync.rs) — 기존 `IndexService::sync_folder` + `pause_watching`/`resume_watching` 재사용.

---

## [2.5.1] - 2026-04-20

**PDF 인코딩 깨짐 대응 + 테이블 렌더링 개선 + UX 폴리싱**

### 수정
- **PDF CID 인코딩 깨짐 감지 + OCR fallback** — Adobe InDesign 등이 Identity-H 폰트를 ToUnicode CMap 없이 임베드하면 pdf-extract/pdfjs 가 CID를 `鈀 逥鎖` 같은 쓰레기 유니코드로 반환하던 문제. 제어문자 ≥5%, PUA ≥5%, 한글+Latin+공백 <30% 중 하나 만족 시 깨진 페이지로 판단하고 OCR 로 대체, OCR 도 실패하면 해당 페이지 스킵(DB 오염 방지).
- **스캔 PDF 프리뷰 본문 누락** — kordoc 이 임베디드 텍스트만 135자 정도 반환해 프리뷰가 짧게 보이던 문제. PDF 는 kordoc 결과와 DB 청크(OCR 결과)를 비교해 긴 쪽 + 깨지지 않은 쪽을 자동 선택.
- **PDF 표가 세로 일렬로 플래튼** — 세로선 없는(행 구분선만) 표가 kordoc 에서 1열 다행 그리드로 잡혀 "목록성 데이터" 로 내려가던 문제. 1×N 그리드는 스킵하여 클러스터 기반 열 감지에 위임.
- **좁은 프리뷰 창에서 표가 글자 단위로 세로 분해** — `.doc-table` 이 `width:100%` + `.doc-th` 가 `white-space:nowrap` 이라 좁은 창에서 다른 열이 1~2글자 폭으로 쭈그러지던 문제. `width:auto`, `max-width:100%`, `word-break: keep-all`, `overflow-wrap: break-word` 로 교체.

### 개선
- **푸터 드라이브 인덱싱 진행률 smooth 표시** — 1/2 완료에서 `50%` 로 튀던 걸 사이드바와 동일하게 `(done + activeFraction) / total` 로 부드럽게. 현재 처리 중 파일명도 함께 표시.
- **검색 필터 바 스크롤 고정** — 결과 스크롤해도 `모두 포함 / 하나 이상 / 정확히 일치 / 관련도순 / 확장자 / 기간 / 파일명 제외 / 프리셋` 줄이 항상 보이도록 sticky 영역으로 분리.
- **결과 스크롤바 드래그 영역 확대** — 기본 8px, 호버 시 14px 로 커져 드래그 쉬워짐. 평소엔 미니멀.

### 빌드/번들
- kordoc 번들 재빌드됨 — `pnpm run bundle-kordoc` 로 PDF 표 감지 수정본 반영.

---

## [2.5.0] - 2026-04-20

**OneDrive·네트워크 폴더 대응 + 모델 번들 (오프라인/회사망 친화)**

### 추가
- **ONNX Runtime + PaddleOCR 모델 MSI 번들** — 첫 실행 시 인터넷 차단 환경에서도 즉시 시맨틱 검색·OCR 가능. 회사망/방화벽으로 huggingface·github 다운로드가 막혀도 동작.
- **OneDrive(클라우드 placeholder) 차단** — Files-On-Demand 로 클라우드에만 있는 파일은 본문 파싱을 자동으로 skip. 인덱서가 모르는 사이 수십 GB 를 끌어내리던 사고 방지. 파일명·크기·수정일은 정상 인덱싱(파일명 검색 가능).
- **네트워크 폴더(UNC, `\\server\share`) 정식 지원** — UNC 경로 정규화(dunce), 30초 주기 PollWatcher 분기, kordoc/HWP 파서 호환. SMB 위에서 inotify 가 동작 안 하는 한계를 폴링으로 우회.
- **`dunce` 의존성 추가** — Windows extended-length(`\\?\`) / UNC prefix 일관 정규화.

### 개선
- 폴더 등록 시 `dunce::canonicalize` 사용 — 네트워크 경로에서 표준 canonicalize 가 수십 초 block 되던 문제 해소.
- 인덱싱 결과에 `cloud_skipped_count` 추가 — 본문 skip 통계와 실패를 분리.

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB

---

## [2.4.0] - 2026-04-20

**최초 배포 (Public release)**

내 PC 문서를 통째로 검색하는 로컬 검색 엔진. 파일명 몰라도, 열어보지 않아도 문서 **내용**으로 찾습니다.

### 핵심 기능
- 문서 내용 검색 (SQLite FTS5, 1초 이내)
- 파일명 검색 (Everything 스타일, 인메모리 캐시)
- 시맨틱/하이브리드 검색 (KoSimCSE ONNX 768차원)
- AI 질의응답 + 문서 요약 (Gemini API, 선택)
- 문서 버전 자동 그룹핑 (lineage) + 버전 간 diff
- 실시간 파일 감시 (`.gitignore` 자동 존중)
- HWPX/DOCX/XLSX/PPTX/PDF/이미지(OCR)/텍스트 지원

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB

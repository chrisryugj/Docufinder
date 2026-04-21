# Changelog

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

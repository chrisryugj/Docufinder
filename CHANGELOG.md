# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/lang/ko/).

## [2.3.10] - 2026-04-20

### Fixed
- **[Critical] v2.3.9 마이그레이션 v14 치명 결함 수정 — "업데이트 후 재실행 시 앱이 안 켜진다"** — v14 가 지우려 했던 `vectors.usearch.map` 파일은 실제로는 존재하지 않는 이름이었다. `Path::with_extension("map")` 은 기존 `.usearch` 확장자를 **교체**하므로 실제 맵 파일명은 `vectors.map` 이다. 결과적으로 v14 마이그레이션은 `.usearch` 본체만 지우고 `.map` 은 그대로 두어, 다음 부팅의 `VectorIndex::new` 가 짝이 맞지 않는 mmap 위에서 usearch FFI segfault 를 낼 수 있었다. 파일명 목록을 실제 구성(`vectors.usearch` + `vectors.map`)에 맞추고, save 중간산출물(`vectors.usearch.tmp`, `vectors.map.tmp`) 까지 보수적으로 함께 회수 (`src-tauri/src/db/migration.rs:370-400`)
- **[Build] Rust 1.95 신규 clippy 린트 4건 (CI 빌드 차단 해제)** — `type_complexity` 2건(`commands/lineage.rs:234`, `db/mod.rs:709`) 은 type alias 로 분리, `needless_range_loop` 1건(`indexer/lineage.rs:357`) 은 `enumerate().take(n)` 로 전환, `needless_borrows_for_generic_args` 1건(`indexer/lineage.rs:552`) 은 불필요한 `&` 제거

### Migration
- 스키마 v14 재진입 — 이미 v2.3.9 에서 v14 가 "성공"으로 기록됐다면 마이그레이션이 재실행되지 않는다. 이 경우 사용자 PC 에 `vectors.map` 이 남아 있을 수 있으나, `VectorIndex::new` 의 이중 폴백(mmap view → full load → 빈 인덱스 회수) 로 부팅 안정성은 확보된다. 손상된 맵은 다음 벡터 재색인 주기에 정상 파일로 교체됨.

## [2.3.9] - 2026-04-19

### Fixed (코덱스 리뷰 기반 품질/보안 핫픽스 6종)
- **[High] 폴더 scope sibling 오탐 차단** — `starts_with(scope)` prefix 매칭이 `C:\docs\a` 로 `C:\docs\a-old` 까지 허용하던 문제. 파일 열기/미리보기 화이트리스트, FTS LIKE, 파일명 캐시, 벡터 후처리 필터, 파일명 캐시 삭제 경로 전부를 segment 경계(`scope/`) 기반으로 통일. 공통 헬퍼 `utils::folder_scope` 도입하여 슬래시·대소문자·`\\?\` prefix 를 단일 규칙으로 정규화 (`utils/folder_scope.rs`, `commands/file.rs:113`, `commands/preview.rs:41`, `search/fts.rs:77`, `search_service/helpers.rs:matches_folder_scope`, `search/filename_cache.rs:188`)
- **[High] 벡터 임베딩 오염 수정 + 재색인 마이그레이션 (v14)** — 벡터 워커가 `fts.content`(원문 + 형태소 토큰) 를 읽어 임베딩을 만들어 시맨틱 공간이 검색용 보강 토큰으로 오염되던 문제. `c.content`(원문) 로 변경하고, 기존 사용자 DB 의 오염된 벡터를 자동 교체하기 위해 마이그레이션 v14 에서 `vector_indexed_at` 전면 리셋 + `vectors.usearch` 삭제 → 다음 부팅 시 원문 기반으로 재임베딩 (`db/mod.rs:931 get_pending_vector_chunks_for_file`, `db/migration.rs:v14`)
- **[High] RAG vector-only 히트 200자 잘림 수정** — 하이브리드 검색의 vector-only 결과가 `full_content=""` 로 저장돼 RAG 컨텍스트 빌드 시 200자 preview 만 LLM 에 전달되던 문제. 의미 검색이 찾아낸 핵심 증거가 잘린 채 전달되지 않도록 `full_content` 에 원문 청크 저장 (`application/services/search_service/hybrid.rs:182`)
- **[High] 벡터 인덱싱 진행률/완료 상태 정확도** — `vector_index.add` 실패 청크까지 `processed` 에 포함시켜 "완료 100%"로 보이지만 재시도 대상이 남아 있던 문제, 취소 시에도 `pending_chunks=0 / is_complete=true` 최종 이벤트를 보내 "완료됨"으로 표시되던 문제 수정. 실제 성공 청크 수만 카운트하고, 최종 이벤트는 DB 에서 남은 pending 을 재조회해 `is_complete = !cancelled && pending==0` 기준으로 결정 (`indexer/vector_worker.rs:run_vector_indexing`)
- **[Medium] 단일 파일 QA 스코프** — 전역 top-25 를 뽑고 파일 필터를 거는 기존 구현은 큰 문서의 관련 청크가 전역 랭킹 밖으로 밀려날 때 "파일 QA 가 정작 질문 위치를 놓치고 앞부분 위주로 답하는" 현상을 유발. `SearchService::search_hybrid_in_file` + `fts::search_in_file` 신설로 처음부터 `f.path = ?` 스코프에서 BM25 상위 청크만 뽑도록 수정 (`application/services/search_service/hybrid.rs`, `search/fts.rs:search_in_file`, `commands/ai.rs:ask_ai_file`)
- **[Medium] Gemini 스트리밍 무증상 실패 감지** — 스트리밍 파서가 `text` 토큰만 이어붙이고 `error.message`, `promptFeedback.blockReason`, `finishReason`(SAFETY/RECITATION/BLOCKLIST/PROHIBITED_CONTENT/SPII) 을 무시해 차단/서버오류가 "빈 정상 응답" 으로 끝나던 문제. 각 SSE 청크에서 종료 사유를 검사하고, 텍스트 한 글자 없이 끝난 경우도 명시적 에러로 승격 (`llm/gemini.rs:generate_stream`)

### Changed
- **RAG 경로에도 lineage collapse 적용** — 검색 커맨드 경로(`apply_lineage_collapse`)는 `group_versions` 설정을 지켰으나 RAG(`ask_ai`) 는 `search_hybrid` 결과를 그대로 사용해 버전 문서들이 컨텍스트 예산을 잠식하던 문제. `group_versions=true` 일 때 이웃 청크 확장 직전 `collapse_by_lineage` 적용 (`commands/ai.rs:ask_ai`)

### Migration
- 스키마 v14 — 자동 적용. 기존 벡터가 오염된 상태이므로 **첫 실행 시 벡터 전면 재색인**이 트리거된다. FTS 검색은 영향 없이 즉시 사용 가능하며, 벡터/RAG 품질은 재색인 진행률에 따라 회복된다.

## [2.3.8] - 2026-04-19

### Fixed
- **MSI 설치 중 "프로그램이 예상대로 완료되지 않았습니다" 오류 핫픽스** — v2.3.0부터 WiX 커스텀 액션 `InstallVCRedist` / `InvokeBootstrapper`(WebView2)의 `Return` 속성이 `"check"`로 설정돼 있어, 이미 동일/상위 런타임이 설치된 시스템에서도 재설치 시 exit 1638(=already installed) 등이 설치 실패로 해석되어 MSI 전체가 롤백되던 문제. 두 CA의 `Return`을 `"ignore"`로 되돌리고, VC++ 감지 레지스트리를 3중화(VS 14.0 Installed / DevDiv Servicing RuntimeMinimum / VS 14.0 Version)하여 불필요한 재설치 시도 자체를 줄임 (`src-tauri/wix/main.wxs`)

## [2.3.7] - 2026-04-18

### Fixed
- **검색 결과 수정일 "1970. 1. 21" 버그** — 버전 드롭다운(LineageBadge)과 파일명 복사본 드롭다운(FilenameCopiesBadge)에서 Unix 초(Rust) → ms(JS) 변환 누락. `* 1000` 추가로 정상 날짜 표시 (`src/components/search/LineageBadge.tsx`, `FilenameCopiesBadge.tsx`)
- **버전 비교 모달 반투명/z-index 깨짐** — `--color-bg-elevated` CSS 변수 미정의로 배경 투명, 부모 stacking context에 갇혀 검색 결과가 모달 위로 비침. fallback 색상 적용 + `createPortal(document.body)`로 탈출 (`src/components/search/VersionDiffModal.tsx`)
- **날짜 표시 연도 생략** — 올해일 때 연도 빼던 로직 제거. 7일 이상 모든 날짜에 연도 포함 (`src/utils/formatRelativeTime.ts`)
- **끊임없는 "1개 파일 변경 반영" 토스트** — `C:\Users\...`를 인덱싱 폴더로 추가하면 `ntuser.dat.LOG2`(레지스트리 하이브 트랜잭션 로그)가 Windows에 의해 상시 수정되어 무한 증분 업데이트 유발. Windows 시스템 파일 블랙리스트 도입으로 차단 (`src-tauri/src/indexer/exclusions.rs`)

### Changed
- **버전 비교 모달 UX 개편** — 변경/추가/제거/동일을 섹션별로 분리 표시, A→B 방향 화살표 및 파일 경로 헤더, 수정된 청크는 2-column 나란히 비교, 동일 청크 샘플(최대 20개)도 제공해 "뭐가 비교됐는지" 시각 확인 가능, 바이트 수준 완전 동일 여부 구분 (`src-tauri/src/commands/lineage.rs`, `src/components/search/VersionDiffModal.tsx`)

### Added
- **`.gitignore` 자동 존중** — 인덱싱 폴더에 `.git`이 있으면 해당 프로젝트의 `.gitignore` + `.git/info/exclude` 규칙을 자동 적용. `node_modules`, `target`, `dist`, `.next`, `__pycache__` 등 빌드/캐시 산출물이 유발하던 반복 증분 인덱싱을 원천 차단. 중첩 git 프로젝트는 가장 깊이 매칭되는 루트 우선 (`src-tauri/src/indexer/gitignore_matcher.rs`, `ignore` 크레이트 0.4)

## [2.3.6] - 2026-04-18

### Added
- **Document Lineage Graph** — 같은 문서의 여러 버전(`최종`/`최최종`/`v2`)을 자동 그룹핑해 검색 결과에서 대표 1개로 표시. 버전 뱃지 클릭 시 포털 드롭다운으로 모든 버전 탐색. 청크 레벨 Diff 모달(added/removed/modified) 제공. Behavioral Canonical(자주 여는 파일 자동 승격) + Cross-Folder Reunion(다른 폴더 같은 문서 벡터 0.95 이상이면 병합) + 건강도 리포트(정리 대상 감지) 포함 (`commands/lineage.rs`, `indexer/lineage.rs`, `components/search/LineageBadge.tsx`, `components/search/VersionDiffModal.tsx`)
- **파일명 매치 복사본 뱃지** — 같은 파일명이 3개 이상 경로에 분산돼 있을 때 대표 1개만 표시하고 `📍 N곳` 뱃지 제공. 뱃지 클릭 시 모든 경로를 포털 드롭다운으로 나열. 2개 이하면 Everything 스타일로 전부 노출 (`components/search/FilenameCopiesBadge.tsx`)
- **없는 파일 정리 (prune)** — 디스크에서 삭제됐으나 DB에 남은 고아 레코드를 일괄 제거. 앱 시작 시 startup sync 말미에 자동 실행 + 설정 > "없는 파일 정리" 버튼으로 수동 실행 가능 (`commands/maintenance.rs`, `commands/index/init.rs`)
- v13 마이그레이션: `files.open_count`, `files.last_opened_at` 컬럼 (Behavioral Canonical 점수 계산용)

### Fixed
- **검색 결과 선택/프리뷰/스크롤 재발 버그 3종** — 리팩토링 때마다 재발하던 증상을 불변식으로 고정
  1. 인덱싱 중 결과 refresh 시 `selectedIndex`가 다른 파일을 가리키게 되어 프리뷰 자동 전환되던 문제 — `useResultSelection`을 path 기반으로 재작성, 결과 변경 시 index만 조용히 재매핑 (`src/hooks/useResultSelection.ts`)
  2. 마우스 클릭 시 `scrollIntoView({block:"nearest"})`가 viewport 하단에 걸친 카드를 아래로 끌어당기던 문제 — `pointerSelectRef` 플래그로 키보드 네비게이션에만 적용되도록 분리 (`src/components/search/SearchResultList.tsx`)
  3. 파일명 매치에 `collapse_by_lineage`가 적용되어 같은 이름의 다른 경로 복사본이 1개로 축소되던 문제 — 파일명 검색 결과는 collapse 우회, 내용 매치에만 유지 (`src-tauri/src/commands/search.rs`)
- **LineageBadge popover UI** — 배경이 반투명해 하위 콘텐츠가 비쳐 보이던 문제(명시적 불투명 배경 + shadow 강화) + 창 좁을 때 우측 잘림(viewport 경계 체크로 좌표 보정, 하단 경계 시 위로 띄움)
- **fixture 파일 인덱싱 오염** — `src-tauri/tests/fixtures/real_filenames.txt`(7388줄, 개발자 정규식 regression 전용)가 사용자 DB에 들어가던 문제. `.list` 확장자로 변경해 Docufinder 파서 대상 제외. 기존에 들어간 잔재는 startup prune 또는 설정 버튼으로 정리 (`src-tauri/src/utils/filename_normalize.rs`)

### Changed
- `DEFAULT_EXCLUDED_DIRS`에 포함된 개발 폴더(`target`, `node_modules`, `.git`, `.cache`)가 전체 드라이브 인덱싱에서 확실히 제외되도록 재확인 (기존 동작 유지)

## [2.3.5] - 2026-04-18

### Removed
- **자동 업데이터 체인 제거** — 릴리스 워크플로우가 updater 서명 단계에서 반복적으로 실패해 왔고(v2.2.0부터 `.sig`/`latest.json` 미생성), Tauri v2의 Windows self-contained updater가 미구현인 구조적 제약이 있어 유지 비용 대비 실효성이 낮다고 판단. `tauri-plugin-updater`, `tauri-plugin-process`, `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process` 의존성과 `useUpdater` 훅, `UpdateBanner` 컴포넌트를 전부 제거. 이제 새 버전은 GitHub Release 페이지에서 MSI를 수동 다운로드해 설치.

## [2.3.3] - 2026-04-17

### Fixed
- 드라이브 인덱싱 전체 진행률을 **완료 job 수 + 현재 active job 진행률 가중** 방식으로 변경. C/E 드라이브 순차 인덱싱 시 한 드라이브가 끝나기 전에도 % 가 부드럽게 증가하도록 개선
- 사이드바 인덱싱 패널의 현재 파일명 영역이 파일 전환 순간 `current_file` 이 잠깐 비어 높이가 줄어들던 문제 — running 상태면 항상 고정 높이 유지

### Fixed
- **전체 드라이브 인덱싱 복원** — v2.3.0 security fix에서 드라이브 루트(`C:\`, `D:\`)를 `validate_watch_path`에서 차단했던 건 Everything 스타일 전체 검색이라는 앱 설계 의도에 반하는 과도한 제한. 제거 후 v2.2.0과 동일하게 `add_folder`에서 drive root 감지 → 경고 emit + 벡터 자동 시작 스킵으로 복구
- **조용한 실패 제거** — `start_indexing_batch`에서 경로가 거부될 때 `continue`만 하고 에러/이벤트 없어 UI에 반응이 없던 문제. 거부 경로별로 `indexing-warning` 이벤트 emit + 모두 거부된 경우 사유 포함한 상세 에러 메시지 반환

## [2.3.1] - 2026-04-17

### Fixed
- AI API 키 마스킹 UI 개선 — input을 비우고 `●` 35자 + 마지막 4자를 placeholder로 표시 (이전 `***1JHY`처럼 7자만 박혀 "키가 잘렸다"는 오해 유발)
- `update_settings`에서 `ai_api_key`가 `None`/빈 문자열로 들어와도 기존 키 유지 — 마스킹 UI가 빈 입력을 보내는 구조 때문에 다른 설정만 저장해도 키가 삭제되던 문제
- AI 질의응답에서 키워드 2어절 이상 쿼리가 FTS AND로 0건일 때 최장 단어 하나로 단독 재검색 (`ask_ai` 폴백)
- 0건일 때 에러 메시지를 "폴더를 인덱싱해주세요"(오해 유발) → "'…' 관련 문서를 찾지 못했습니다. 더 일반적인 키워드로 시도해보세요"로 변경
- Gemini 모델 ID 오타 수정: `gemini-3.0-flash` → `gemini-3-flash-preview` (API `/v1beta/models` 응답과 일치). 기존 설정값은 `get_settings_sync` 로드 시 자동 마이그레이션

## [2.3.0] - 2026-04-17

### Security
- 감시 폴더 등록 시 시스템 폴더(`C:\Windows`, `Program Files` 등)·드라이브 루트 차단 (`constants::validate_watch_path`) — `add_folder`/`reindex_folder`/`resume_indexing`/`start_indexing_batch` 4개 진입점에 일괄 적용
- RAG 컨텍스트 문서 내용 sanitize — 프롬프트 구분자(`--- 질문 ---`, `--- 문서 ---` 등) 치환으로 Prompt Injection 방어 (`commands/ai.rs::sanitize_doc_content`)
- QA/File 시스템 프롬프트에 보안 지침 추가 — 문서 내용 안 지시문을 사용자 명령으로 해석하지 않도록 명시
- `credentials.json` Windows ACL 격리 — `icacls`로 현재 사용자 전용 접근 (멀티 프로세스 환경에서 평문 키 노출 차단)
- `get_settings` 반환값 AI API 키 마스킹 (`***` + 마지막 4자리) — 프론트엔드 메모리 잔류 차단
- PDF 이미지 디코딩 픽셀 상한 (100M 픽셀) — 악성 PDF의 비정상 width/height 유도 OOM 방어
- 데드코드 `verify_admin_code` 커맨드 제거 — 바이너리에 하드코딩 문자열(`"9812"`) 잔존 제거

### Changed
- ORT runtime feature `download-binaries` → `load-dynamic` — 빌드 타임 중복 다운로드 제거, 재현성 개선 (`ORT_DYLIB_PATH`는 lib.rs setup에서 이미 설정 중)
- `is_blocked_path` 헬퍼 공용화 + `is_drive_root`/`validate_watch_path` 추가 — 인덱싱 진입점의 경로 검증 로직 통합

### Notes
- WiX `perMachine`/HKCU 정합성 수정은 VCRedist·업그레이드 경로 QA가 필요해 이번 릴리즈에서 제외, 별도 패치에서 단독 진행 예정
- 기존 v2.1.x 사용자는 updater 서명 부재로 여전히 수동 MSI 설치 필요

## [2.2.0] - 2026-04-17

### Security
- LIKE 쿼리 ESCAPE 절 + 와일드카드 이스케이프 (`commands/duplicate.rs`)
- Gemini API 키를 URL 쿼리 → `x-goog-api-key` 헤더로 이동 (로그 유출 차단)
- `data_root` 경로 검증 (심볼릭 링크/드라이브 루트/시스템 폴더 거부)
- 파일/미리보기 화이트리스트 무조건 적용 (감시 폴더 미등록 시 거부)
- kordoc Node 시스템 PATH fallback 제거 (PATH hijacking 방지)

### Changed
- DB 커넥션 풀 6 → 16 (SQLITE_BUSY 폭주 방지)
- 벡터 prefetch 스레드 `retry_on_busy` 적용
- panic hook 필터 확장 (zip/quick-xml/calamine/lopdf crash.log 오염 방지)
- kordoc 의존성 SHA 고정 (공급망 공격 차단)
- VCRedist 설치 실패 감지 (`Return=check`)
- publish workflow `releaseDraft=false`
- `Ctrl+C` → `Ctrl+Shift+C` 재할당 (일반 복사 충돌 해결)
- localStorage 키 `docufinder_` prefix 통일 (레거시 키 자동 마이그레이션)

### Fixed
- 스플래시 스피너 브랜드 녹색 적용 (테라코타 잔존 제거)
- 누락 CSS 변수 보강 (`--color-text-tertiary`, `--color-accent-border/tertiary`, `--color-error-subtle/border`)
- `Ctrl+K` 시 질문 textarea 전체선택 방지 (입력 유실 방지)
- `useWindowFocus`가 모든 모달을 존중 (포커스 트랩 보호)

### Removed
- 미사용 `dompurify` 의존성
- 잔존 백업 파일 `wix/dialog.bmp.bak`

### Notes
- 본 릴리즈는 updater 서명(`.sig`)을 포함하지 않음 — v2.1.0에서의 자동 업데이트는 지원하지 않으며 수동으로 MSI를 받아 설치 필요

## [2.1.0] - 2026-04-11

### Added
- AI RAG 출처 문서 번호 매칭 — 근거 파일 강조 표시
- 파일 태그 시스템 (커스텀 태그 분류/검색)
- 검색 결과 CSV/JSON 내보내기
- 폴더 트리 드래그&드롭 정렬
- 시스템 트레이 최소화 + 자동 시작 옵션
- 윈도우 상태 복원 (크기/위치 기억)
- 소개 영상 (Remotion 기반)

### Changed
- Anything 브랜딩 전면 적용
- RAG 프롬프트 개선 (출처 인라인 제거 + 수치 정확 인용)
- RAG 참조 파일을 실제 컨텍스트 사용분만 표시
- AI 패널 글자 크기 증가 (가독성)
- FTS 스마트 쿼리 파싱 최적화
- README 전면 리라이트 (korean-law-mcp 스타일)

### Fixed
- 프로덕션 감사 34개 이슈 일괄 수정
- 파서 안정성 강화 (DOCX/XLSX/PDF/TXT 에러 핸들링)
- 인덱싱 중 파일 삭제 시 크래시 방지
- 대용량 폴더 추가 시 UI 멈춤 해결
- DB 동시성 이슈 전면 수정

### Security
- kordoc 번들링 (node.exe + cli.js MSI 포함, 사용자 Node.js 설치 불필요)
- 프로덕션 감사 전체 통과

## [2.0.0] - 2026-04-11

### Added
- AI 문서 분석 (Gemini RAG): 하이브리드 검색 기반 문맥 질의응답
- AI 파일 QA: 단일 파일 대상 질의응답
- AI 요약: 문서별 자동 요약 (유형 선택 가능)
- OCR 지원: PaddleOCR 기반 스캔 PDF 텍스트 추출
- HWP 파일 변환 지원 (kordoc 번들링)
- 전체 PC 인덱싱 모드 (드라이브 단위)
- 검색 범위 필터링 (폴더 스코프)
- 유사 문서 찾기 기능
- 중복 파일 탐지
- 문서 통계 대시보드
- 법령 참조 자동 링크 (law.go.kr)
- TextRank 추출적 요약 (오프라인)
- OTA 자동 업데이트 (GitHub Releases)
- 버전 동기화 스크립트 (scripts/bump-version.ps1)

### Changed
- 임베딩 모델: F32 → INT8 양자화 (용량 840MB 절감)
- 브랜딩: DocuFinder → Anything
- Clean Architecture 적용 (domain/application/infrastructure)
- DB 스키마 v11 (마이그레이션 자동)
- crash.log 날짜 기반 로테이션 (최대 3파일)
- DB retry 지수 백오프 (100ms → 200ms → 400ms)

### Security
- open_file() 감시 폴더 범위 검증 (경로 순회 방지 강화)
- AI 요청 동시 실행 제한 (Semaphore, 최대 3)
- min_confidence 백엔드 범위 검증 추가
- DB 부팅 시 무결성 검사 (PRAGMA integrity_check)

### Accessibility
- Skip-to-main-content 링크 추가
- 컨텍스트 메뉴 키보드 내비게이션 (Arrow/Enter/Escape)
- 검색 결과 수 aria-live 공지
- 폴더 트리 키보드 내비게이션 (Arrow/Enter/Space)
- 통일 Spinner 컴포넌트 (role="status", aria-label)

### Added (UX)
- AI 응답 복사 버튼 (체크마크 피드백)

## [1.0.0] - 2026-02-22

### Added
- 하이브리드 검색: 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 + Cross-Encoder 재정렬
- Everything 스타일 파일명 검색 (인메모리 캐시)
- 실시간 폴더 감시 + 증분 인덱싱 (notify 8)
- 2단계 인덱싱: FTS 즉시 완료 → 벡터 백그라운드 처리
- 인덱싱 진행률 실시간 표시 + 취소 버튼
- 즐겨찾기 폴더 핀 고정
- HDD/SSD 자동 감지 + 적응형 스레딩
- 지원 파일 형식: HWPX, DOCX, XLSX, PDF, TXT
- KoSimCSE-roberta-multitask 임베딩 (768차원)
- ms-marco-MiniLM-L6-v2 재정렬 모델
- Lindera 2.0 한국어 형태소 분석
- 다크모드 / 라이트모드 / 시스템 테마
- 색상 프리셋 커스터마이징
- CSP 보안 정책 적용
- 압축 폭탄 방어 (크기/비율/엔트리 제한)
- SHA-256 모델 무결성 검증
- HuggingFace 자동 모델 다운로드
- MSI 설치 파일 빌드 (코드 서명 지원)
- 크래시 핸들러 (panic hook → crash.log, 7일 로테이션)

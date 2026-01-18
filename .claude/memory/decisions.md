# DocuFinder 아키텍처 결정 기록

## 2026-01-17 - 토스트 시스템 분리

**상황**: useExport 내부에 토스트 로직이 내장되어 있어 다른 곳에서 재사용 불가
**결정**: 범용 useToast 훅 + Toast 컴포넌트로 분리
**이유**: 파일 열기, 경로 복사 등 다양한 곳에서 토스트 피드백 필요
**영향**:
- `src/hooks/useToast.ts` (신규)
- `src/components/ui/Toast.tsx` (신규)
- `src/hooks/useExport.ts` (showToast 옵션으로 변경)
- `src/App.tsx` (ToastContainer 통합)

## 2026-01-17 - 최근검색 타입 마이그레이션

**상황**: 기존 최근검색이 `string[]`으로 저장되어 검색시간 표시 불가
**결정**: `RecentSearch { query, timestamp }` 형식으로 변경 + 자동 마이그레이션
**이유**: 시간 배지 ("2분 전", "어제") 표시 요구사항
**영향**:
- `src/types/search.ts` (RecentSearch 타입 추가)
- `src/hooks/useLocalStorage.ts` (마이그레이션 로직)
- localStorage 키: `docufinder_recent_searches` → `docufinder_recent_searches_v2`
- 기존 데이터 자동 변환 (타임스탬프는 현재 시간으로)

## 2026-01-17 - 하위폴더 포함 설정 실제 연동

**상황**: 설정 UI에 "하위폴더 포함" 토글이 있었으나 실제 동작하지 않음
**결정**: `pipeline.rs`에 `index_folder_with_options(recursive)` 추가, `add_folder`에서 설정값 읽어서 적용
**이유**: 사용자가 설정한 값이 실제로 반영되어야 함
**영향**:
- `src-tauri/src/indexer/pipeline.rs` (collect_files_shallow 추가)
- `src-tauri/src/commands/settings.rs` (get_settings_sync 추가)
- `src-tauri/src/commands/index.rs` (설정 읽어서 전달)

## 2026-01-17 - 즐겨찾기 폴더 (DB 컬럼 방식)

**상황**: 자주 사용하는 폴더를 상단에 고정하고 싶음
**결정**: `watched_folders` 테이블에 `is_favorite` 컬럼 추가 + 자동 마이그레이션
**이유**: 별도 테이블 대신 기존 테이블 확장이 단순함, SQLite ALTER TABLE 지원
**영향**:
- `src-tauri/src/db/mod.rs` (is_favorite 컬럼 + toggle_favorite, get_watched_folders_with_info)
- `src-tauri/src/commands/index.rs` (toggle_favorite, get_folders_with_info 커맨드)
- `src/components/sidebar/FolderTree.tsx` (별 아이콘 + 즐겨찾기 상단 정렬)

## 2026-01-18 - 인덱싱 진행률 시스템 (Tauri Event)

**상황**: 대용량 폴더/드라이브 인덱싱 시 진행 상황 모름, 취소 불가
**결정**: Tauri 이벤트 시스템으로 백엔드→프론트엔드 실시간 진행률 전송 + AtomicBool 취소 플래그
**이유**:
- `invoke` 응답 대기 중 실시간 피드백 불가 → Tauri `emit` 이벤트 필요
- 병렬 파싱 중 안전한 취소 → `AtomicBool` + `Ordering::Relaxed`
**영향**:
- `src-tauri/src/indexer/pipeline.rs` (index_folder_with_progress, ProgressCallback)
- `src-tauri/src/commands/index.rs` (app_handle.emit, cancel_indexing 커맨드)
- `src-tauri/src/lib.rs` (indexing_cancel_flag in AppState)
- `src/hooks/useIndexStatus.ts` (listen, ask 다이얼로그)
- `src/components/layout/StatusBar.tsx` (진행률 바 + 취소 버튼)

## 2026-01-18 - 드라이브 루트 인덱싱 경고

**상황**: 사용자가 D:\ 등 드라이브 전체 선택 시 시간 오래 걸림 인지 필요
**결정**: 프론트엔드에서 경로 패턴 감지 후 Tauri `ask` 다이얼로그로 확인
**이유**: 백엔드에서 경고하면 이미 폴더 등록됨, 프론트에서 사전 차단이 UX에 좋음
**영향**:
- `src/hooks/useIndexStatus.ts` (isDriveRoot 함수, ask 다이얼로그)

## 2026-01-18 - Everything 스타일 파일명 검색

**상황**: 내용 검색 외에 파일명으로만 빠르게 검색하고 싶은 요구
**결정**: `files_fts` FTS5 테이블 + `search_filename` 커맨드 + 4번째 검색 모드
**이유**: 기존 내용 FTS(`chunks_fts`)와 분리하여 파일명 전용 검색 최적화
**영향**:
- `src-tauri/src/db/mod.rs` (files_fts 테이블, 자동 마이그레이션)
- `src-tauri/src/search/filename.rs` (신규)
- `src-tauri/src/commands/search.rs` (search_filename 커맨드)
- `src/types/search.ts` (SearchMode에 filename 추가)
- `src/components/search/SearchBar.tsx` (파일명 모드 버튼)
- `src/utils/cleanPath.ts` (Windows Long Path prefix 제거)

## 2026-01-18 - VectorIndex 스레드 안전성 (RwLock 선택)

**상황**: 코드 리뷰에서 `unsafe impl Send/Sync` + 보호되지 않은 `index: Index` 필드로 인한 데이터 레이스 위험 지적
**결정**: `index: RwLock<Index>` 적용, unsafe impl 제거
**대안 검토**:
- Option A: 전체 VectorIndex를 Mutex로 감싸기 → 검색도 직렬화되어 성능 저하
- Option B: index만 RwLock (선택) → 읽기 병렬, 쓰기 직렬
- Option C: Actor 패턴 → 복잡도 높음
**이유**: 데스크톱 앱 특성상 극단적 동시성 불필요, RwLock으로 충분한 성능/안전성 확보
**영향**:
- `src-tauri/src/search/vector.rs` (구조체 + 모든 메서드)

## 2026-01-18 - 모델 부재 시 벡터 인덱스 비활성화

**상황**: 모델 없을 때도 빈 벡터 인덱스 생성되어 사용자 혼동
**결정**: `get_vector_index()`에서 모델 체크 후 명확한 에러 반환
**이유**: 모델 없으면 시맨틱 검색 불가하므로 인덱스 생성 자체가 무의미
**영향**:
- `src-tauri/src/lib.rs:107-115` (모델 체크 + 에러 메시지)

## 2026-01-18 - 폰트 로컬 번들링 + CSP 강화

**상황**: 외부 CDN 폰트 + CSP null 설정은 보안 취약점
**결정**: Pretendard Variable 폰트 로컬 번들링(~2MB) + 엄격한 CSP 설정
**이유**: 오프라인 동작 보장 + XSS 공격 벡터 차단
**영향**:
- `src/assets/fonts/PretendardVariable.woff2` (신규)
- `src/assets/fonts/pretendard.css` (신규)
- `index.html` (CDN → 로컬 참조)
- `src-tauri/tauri.conf.json` (CSP 설정)

## 2026-01-18 - 설정값(max_results) 실제 반영

**상황**: Settings에 max_results가 있지만 검색에서 50으로 하드코딩
**결정**: 모든 검색 함수에서 `get_settings_sync()` 호출하여 설정값 사용
**이유**: 사용자 설정이 실제로 반영되어야 함
**영향**:
- `src-tauri/src/commands/search.rs` (search_keyword, search_filename, search_semantic, search_hybrid)

## 2026-01-18 - 2단계 인덱싱 시스템 아키텍처

**상황**: 2,000개 파일 인덱싱 시 10분+ 소요, ONNX 임베딩이 99% 병목
**결정**: 1단계(FTS) + 2단계(벡터 백그라운드) 분리 아키텍처
**대안 검토**:
- Option A: 현행 유지 + 배치 최적화 → 근본적 해결 안 됨
- Option B: 2단계 분리 (선택) → 즉시 검색 가능 + 백그라운드 완료
- Option C: 벡터 인덱싱 비활성화 옵션 → 시맨틱 검색 UX 저하
**이유**:
- 사용자는 FTS 완료 즉시 검색 시작 가능 (~30초)
- 시맨틱 검색은 백그라운드 완료 후 점진적 활성화
- 앱 재시작 시에도 이어서 진행 가능
**영향**:
- `src-tauri/src/db/mod.rs` (fts_indexed_at, vector_indexed_at 컬럼)
- `src-tauri/src/indexer/pipeline.rs` (FTS 전용 함수 분리)
- `src-tauri/src/indexer/vector_worker.rs` (신규 - 백그라운드 워커)
- `src/components/ui/VectorIndexingFAB.tsx` (신규 - 진행률 FAB)
- `src/hooks/useVectorIndexing.ts` (신규 - 상태 훅)

## 2026-01-18 - VectorWorker 설계 (별도 스레드)

**상황**: 2단계 벡터 인덱싱을 어떻게 백그라운드에서 실행할지
**결정**: `VectorWorker` 구조체 + 별도 `std::thread::spawn`
**대안 검토**:
- Option A: tokio::spawn → async 런타임 복잡도, 임베딩은 CPU 바운드
- Option B: std::thread (선택) → 단순함, CPU 바운드 작업에 적합
- Option C: rayon 스레드 풀 → 이미 파싱에 사용 중, 충돌 우려
**이유**:
- 임베딩은 CPU 집약적 작업으로 async 이점 없음
- 별도 스레드로 메인 스레드 블로킹 없음
- `Arc<AtomicBool>`로 안전한 취소 지원
**영향**:
- `src-tauri/src/indexer/vector_worker.rs` (VectorWorker, start, cancel, get_status)
- `src-tauri/src/lib.rs:44-45` (AppState에 vector_worker: RwLock<VectorWorker>)

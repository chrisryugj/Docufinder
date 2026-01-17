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

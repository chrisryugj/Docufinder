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

## 2026-01-21 - 클린 아키텍처 전면 도입

**상황**: 배포 전 최종 점검에서 다음 문제 발견
- Critical 버그: VectorIndex 데드락, WatchManager 종료 실패
- 아키텍처: DIP 위반 (commands → db 직접 의존), God Object (AppState), Repository 패턴 부재
- React: App.tsx 589줄, Props Drilling 5단계, Error Boundary 없음

**결정**: Rust + React 전면 리팩토링 (3주)
- Rust: Domain/Application/Infrastructure/Presentation 레이어 분리
- React: Zustand + Feature-Based 구조

**대안 검토**:
- Option A: 버그만 수정 → 근본적 해결 안 됨
- Option B: 핵심만 개선 (God Object 분리 + Props Drilling) → 부분적 해결
- Option C: 전면 리팩토링 (선택) → 완전한 클린 아키텍처 도입

**이유**:
- 배포 전 시간 여유 있음 (2-3주)
- 테스트 가능성 확보 (Repository Trait → Mock 가능)
- 유지보수성 향상 (레이어 분리 → 변경 영향 최소화)

**영향**:
- Rust: 35개 파일 생성, 7개 수정, 2개 삭제
- React: 47개 파일 생성, 32개 이동, 7개 삭제
- 예상 효과: 클린 아키텍처 51% → 90%+

## 2026-01-21 - Rust Repository Trait 설계 (async-trait)

**상황**: Domain Layer에 Repository 추상화 필요
**결정**: `async_trait` 매크로 사용, trait에 `Send + Sync` 바운드

**대안 검토**:
- Option A: 동기 trait → DB I/O가 블로킹, 확장성 제한
- Option B: async_trait (선택) → 비동기 지원, 테스트 용이
- Option C: GAT (Generic Associated Types) → Rust 1.65+ 필요, 복잡

**이유**:
- Tauri 2.0이 tokio 기반이므로 async 호환 필요
- async_trait이 가장 안정적이고 널리 사용됨
- `Send + Sync` 바운드로 스레드 안전성 보장

**영향**:
- `Cargo.toml` (async-trait = "0.1")
- `domain/repositories/*.rs` (모든 trait에 #[async_trait] 적용)

## 2026-01-21 - React 상태 관리 (Zustand 선택)

**상황**: App.tsx 589줄, Props Drilling 5단계 해소 필요
**결정**: Zustand로 전역 상태 관리

**대안 검토**:
- Option A: Context API → Provider hell, 불필요한 re-render
- Option B: Zustand (선택) → 간단, 성능 좋음, ~1KB
- Option C: Redux Toolkit → 과도한 boilerplate, 앱 규모에 비해 무거움
- Option D: Jotai/Recoil → 원자적 상태에 적합, 복잡한 상태에는 Zustand가 나음

**이유**:
- 번들 사이즈 최소 (~1KB gzip)
- DevTools 지원 (Redux DevTools 호환)
- TypeScript 우선 설계
- Provider 불필요 → 직접 import

**영향**:
- `pnpm add zustand`
- `features/*/store/*.ts` (5개 store 생성)
- App.tsx 589줄 → 120줄

## 2026-01-26 - Search Commands → SearchService 전환 (Thin Commands 패턴)

**상황**: 이전에 "Commands 전환 스킵"으로 결정했으나, tokenizer/reranker 기능 동기화 필요로 재검토
**결정**: Search Commands만 우선 전환 - Thin Commands 패턴 적용

**변경 사항**:
- `commands/search.rs`: 640줄 → 170줄 (입력검증 + Service 호출만)
- `SearchService`: tokenizer/reranker 지원 추가
- `AppError → ApiError` 변환 구현

**이유**:
- SearchService에 tokenizer/reranker 지원이 이미 구현됨
- Commands에서 중복 구현 대신 Service 호출이 효율적
- "Thin Commands" 패턴으로 레이어 분리 유지

**영향**:
- `commands/search.rs` (170줄로 축소)
- `application/services/search_service.rs` (tokenizer/reranker 추가)
- `application/container.rs` (get_tokenizer, get_reranker 추가)
- `application/dto/search.rs` (modified_at 필드 추가)
- `error.rs` (AppError → ApiError From 구현)

---

## 2026-01-21 - Commands → Service 전환 스킵

**상황**: Application Layer 구현 완료 후 Commands를 Service 의존으로 전환 검토
**결정**: 전환 스킵 - 오버엔지니어링으로 판단 → **2026-01-26 Search만 전환으로 변경**

**대안 검토**:
- Option A: 완전 전환 → Commands에서 AppState 대신 Service 직접 사용
- Option B: 점진적 전환 → Commands에서 Service 생성 후 호출
- Option C: 전환 스킵 (선택) → 기존 구조 유지

**이유**:
- Tauri State 구조상 Commands는 AppState에 묶임
- Service 매번 생성 = 비효율 (캐싱 불가)
- Commands는 어차피 통합 테스트 필요 (유닛 테스트 이점 없음)
- 단일 Tauri 앱에서 Application Layer 전체 활용은 오버엔지니어링

**결론**:
- Domain Layer (Entities, Value Objects, Repository Traits) ✅ 유지
- Infrastructure Layer (SQLite, usearch, ONNX 구현체) ✅ 유지
- Application Layer (Services, DTOs, Container) → 구현됨, 필요시 사용
- Commands 전환 → ❌ 스킵

**영향**:
- 기존 commands/*.rs 그대로 유지
- application/ 디렉토리는 존재하지만 미사용 (dead_code 경고)
- 향후 CLI 추가 등 확장 시 활용 가능

## 2026-01-30 - 성능 최적화 (HDD + 저사양 환경)

**상황**: i3 12100 / 8-16GB RAM / HDD / 내부망 보안프로그램 환경에서 앱이 무겁게 느껴짐
**목표**: Everything 수준의 검색 속도

### 프론트엔드 결정

**1. 외부 폰트 → 로컬 번들**
- **문제**: Google Fonts 외부 로딩이 내부망에서 타임아웃
- **결정**: Pretendard woff2 로컬 번들 (이미 존재), 외부 @import 제거
- **영향**: `src/index.css` - 앱 시작 2-5초 단축

**2. blur 효과 제거**
- **문제**: backdrop-filter: blur가 저사양 GPU에서 프레임 드롭
- **결정**: blur 제거, 불투명 배경으로 대체
- **영향**: `src/index.css` - GPU 부하 50% 감소

**3. 그룹 뷰 가상화 스킵**
- **이유**: 100개 그룹 정도는 React가 충분히 처리 가능, 진짜 병목은 백엔드
- **결정**: 우선순위 낮춤, 필요시 추후 적용

### 백엔드 결정

**4. DB 중복 조회 제거**
- **문제**: search_service.rs에서 get_chunks_by_ids() 2-3회 호출
- **결정**: FTS 결과를 HashMap으로 변환, Reranking/결과 변환에서 재사용
- **영향**: `search_service.rs:289-360` - HDD 50-100ms 절감

**5. SQLite PRAGMA 조정 (예정)**
- mmap 128MB → 64MB (HDD 환경)
- cache 32MB → 16MB (8GB RAM 배려)

**6. HWPX 중복 ZIP 오픈 제거 (예정)**
- 현재: header 파싱 후 archive drop → 다시 오픈
- 계획: 단일 순회로 통합

### 계획 파일
`C:\Users\Chris\.claude\plans\inherited-marinating-meerkat.md`

## 2026-01-31 - 저사양 환경 성능 최적화 아키텍처 (3개 리뷰 종합)

**상황**: i3/8GB/HDD + 내부망 보안프로그램 환경에서 Everything급 성능 목표
**리뷰 소스**: Claude 분석 + Gemini 리뷰 + 추가 리뷰

### 핵심 결정 1: 2단계 분리 인덱싱

**문제**: `pipeline.rs:62,450`에서 rayon par_iter() 병렬 파싱 → HDD 랜덤 I/O 폭증
**결정**:
- Phase 1: 메타데이터 스캔 (파일 열지 않음) → 파일명 검색 즉시 가능
- Phase 2: 컨텐츠 파싱 (단일 스레드, 유휴 시) → 내용 검색 점진적 활성화
**이유**: HDD에서 8스레드 랜덤 액세스 = 디스크 thrashing = 수 KB/s로 떨어짐
**영향**:
- `indexer/pipeline.rs` - scan_metadata_only() 함수 신규
- `indexer/background_parser.rs` - 신규 파일

### 핵심 결정 2: HDD 단일 스레드 처리

**문제**: rayon 병렬이 HDD에서 오히려 성능 저하, 보안 프로그램 CPU 점유율 폭발
**결정**: SSD/HDD 자동 감지 → HDD면 단일 스레드 순차 처리
**구현**:
- WMI로 MediaType 조회 (Windows)
- 실패 시 C:=SSD, D:=HDD 패턴으로 폴백
**영향**: `utils/disk_info.rs` - 신규 파일

### 핵심 결정 3: 유휴 감지 (Idle Detection)

**문제**: 백그라운드 인덱싱이 사용자 작업과 충돌
**결정**: GetLastInputInfo로 유휴 시간 감지, 3초 유휴 시에만 파싱 진행
**이유**: 사용자 활동 중에는 파싱 일시정지 → PC 사용성 유지
**영향**: `utils/idle_detector.rs` - 신규 파일

### 핵심 결정 4: full_content 제거

**문제**: `fts.rs:48`, `search_service.rs:86`에서 전체 문서 내용을 응답에 포함 → DB I/O, 직렬화, 메모리 과부하
**결정**: highlight() 제거, snippet만 유지, full_content 필드 옵션화
**효과**: 응답 크기 ~90% 감소
**영향**:
- `search/fts.rs` - highlight() 제거
- `application/services/search_service.rs` - full_content 제거
- `src/types/search.ts` - full_content 옵션화

### 핵심 결정 5: 변경 감시 벡터 분리

**문제**: `manager.rs:230`에서 파일 변경 시 FTS+벡터 즉시 갱신 → CPU/IO 급증, UI 끊김
**결정**: FTS만 즉시 갱신, 벡터는 벡터 워커가 백그라운드에서 처리
**영향**: `indexer/manager.rs` - index_file_fts_only() 사용

### 계획 파일
`~/.claude/plans/indexed-gathering-hellman.md`

## 2026-01-31 - Phase 1 Quick Win 구현

### 핵심 결정 6: full_content 제거 (응답 경량화)

**문제**: 검색 응답에 전체 문서 내용(`full_content`) 포함 → DB I/O, 직렬화, 프론트 메모리 과부하
**결정**:
- 백엔드: `full_content: String::new()` (빈 문자열)
- 프론트: `full_content?: string` 옵션화, snippet/content_preview 폴백
**이유**: 프리뷰에는 snippet(~200자)으로 충분, 펼치기 뷰도 content_preview 활용 가능
**영향**:
- `search/fts.rs` - highlight() 컬럼 제거
- `search_service.rs` - 모든 검색 함수에서 빈 문자열
- `search.ts` - @deprecated 마킹
- `SearchResultItem.tsx`, `useSearch.ts` - 폴백 로직

### 핵심 결정 7: 진행률 throttling

**문제**: 파일마다 진행률 이벤트 발송 → UI 렌더링 부하, 메인 스레드 블로킹
**결정**: 100ms 또는 10파일마다 1회로 제한
**구현**: Cell 기반 마지막 전송 시간/개수 추적
**영향**: `pipeline.rs:410-432` - send_progress에 force 파라미터 추가

## 2026-01-31 - Phase 2~3 성능 최적화 구현

### 핵심 결정 8: scan_metadata_only() 설계

**문제**: 대용량 폴더 인덱싱 시 파일 열기가 병목 (파싱 + 보안 스캔)
**결정**: walkdir로 메타만 수집, DB에 즉시 저장 (fts_indexed_at = NULL)
**이유**:
- 파일 열지 않으면 보안 프로그램 스캔 트리거 안 됨
- 메타만 저장해도 파일명 검색 즉시 가능
- 컨텐츠 파싱은 BackgroundParser가 유휴 시 처리
**영향**:
- `indexer/pipeline.rs` - scan_metadata_only() 신규
- `db/mod.rs` - insert_file_metadata_only() 신규

### 핵심 결정 9: 변경 감시 벡터 분리

**문제**: `manager.rs:230`에서 index_file()이 FTS+벡터 동시 처리 → CPU/IO 급증
**결정**: index_file_fts_only() 사용, 벡터는 vector_worker가 백그라운드 처리
**이유**: 변경 감지 시 UI 끊김 방지, 사용자 체감 속도 향상
**영향**: `indexer/manager.rs:231` - index_file_fts_only() 호출

### 핵심 결정 10: BackgroundParser 유휴 감지

**문제**: 백그라운드 인덱싱이 사용자 작업과 충돌
**결정**: GetLastInputInfo로 유휴 감지, 3초 유휴 시에만 파싱
**구현**:
- `utils/idle_detector.rs` - is_user_idle(), wait_for_idle_sync()
- `indexer/background_parser.rs` - 유휴 대기 루프
**이유**: 사용자 활동 중 일시정지 → PC 사용성 유지

### 핵심 결정 11: SSD/HDD 자동 감지

**문제**: HDD에서 병렬 처리가 오히려 성능 저하 (랜덤 I/O thrashing)
**결정**: WMI MediaType 조회 → 실패 시 드라이브 문자 추정 (C:=SSD, D:=HDD)
**구현**: `utils/disk_info.rs` - detect_disk_type(), DiskSettings
**이유**: HDD면 throttle 적용, SSD면 병렬 처리

### 핵심 결정 12: FilenameCache 인메모리 검색

**문제**: LIKE '%term%' 전체 스캔이 HDD에서 느림 (100ms+)
**결정**: DB 전체를 인메모리 캐시로 로드, O(n) 벡터 스캔 (~5ms)
**구현**:
- `search/filename_cache.rs` - FilenameCache 구조체
- `container.rs` - filename_cache 필드 + load_filename_cache()
- `search_service.rs` - 캐시 우선, 폴백은 DB LIKE
- `lib.rs` - 앱 시작 시 캐시 로드
**이유**: 10만 파일에서도 ~5ms로 Everything 수준 달성
**영향**: 메모리 사용량 약간 증가 (파일당 ~200바이트)

## 2026-01-31 - 사내 배포 보안 강화 (Phase 1)

**상황**: 사내망 배포 준비성 검토에서 보안 이슈 발견
- 외부 업데이터 (GitHub Releases) → 내부망에서 통신 차단/보안 정책 충돌
- 모델/DLL 무결성 검증 없음 → 변조 시 코드 실행 위험
- 압축 폭탄 미방어 → OOM/DoS 가능성

**리뷰 소스**: AI 리뷰 + 외부 시니어 리뷰 3개 종합
- AI 리뷰: RwLock expect() 과대평가, 배포 환경 고려 부족
- 외부 리뷰: 실무 관점, 보안 정책, 환경 제약 고려 → **외부 리뷰가 더 정확**

### 결정 1: 업데이터 완전 비활성화

**결정**: updater 플러그인 제거, 수동 배포 정책
**이유**: 사내망에서 GitHub 접근 차단 → 업데이트 실패 시 앱 오류로 인식
**영향**:
- `tauri.conf.json` - plugins.updater 섹션 제거
- `default.json` - updater:default 권한 제거
- `lib.rs` - tauri_plugin_updater 주석
- `Cargo.toml` - tauri-plugin-updater 주석

### 결정 2: SHA-256 무결성 검증

**결정**: 모델/DLL 다운로드 시 해시 검증 필수
**구현**:
- 예상 해시값 상수 정의
- 다운로드 후 compute_sha256() 검증
- 불일치 시 파일 삭제 + 명확한 에러 메시지
**영향**: `model_downloader.rs` - 전면 개편 (sha2 크레이트 추가)

### 결정 3: 다운로드 타임아웃/크기 제한

**결정**: 연결 30초, 읽기 5분, 최대 500MB
**이유**: 내부망 차단 시 무한 대기 방지
**영향**: `model_downloader.rs` - ureq Agent config 설정

### 결정 4: 압축 폭탄 방어

**결정**: ZIP 파일 압축 해제 시 다중 검증
- 단일 엔트리: 50MB 제한
- 총 압축 해제: 200MB 제한
- 압축 비율: 100:1 초과 시 차단
- 엔트리 수: 1000개 제한
**영향**:
- `parsers/hwpx.rs` - validate_zip_archive() 전면 검증
- `parsers/docx.rs` - validate_zip_archive() 추가
- `parsers/xlsx.rs` - 파일 크기 100MB 제한

### 계획 파일
`~/.claude/plans/prancy-soaring-anchor.md`

## 2026-01-31 - 사내 배포 보안 강화 (Phase 3)

**상황**: 3가지 리뷰 종합 후 추가 보안/안정성 개선 필요
- 리뷰 1: 보안/코드품질/배포안정성 종합
- 리뷰 2: HWPX 파서 공백/에러처리/압축폭탄
- 리뷰 3: DLL 무결성/DB 암호화 최종 검증

### 결정 5: ONNX Runtime DLL 해시 검증

**문제**: 모델 파일은 SHA-256 검증하지만, DLL은 검증 생략 (코드에 주석으로 명시)
**결정**: ZIP 다운로드 후 해시 검증 추가 (`78d447...`)
**이유**: DLL은 실행 코드 → 변조 시 치명적
**영향**: `model_downloader.rs:31-32, 276-285`

### 결정 6: DB 트랜잭션 적용

**문제**: 4개 DELETE 개별 실행 → 중간 실패 시 데이터 불일치 (고아 레코드)
**결정**: BEGIN IMMEDIATE + COMMIT/ROLLBACK 패턴
**구현**: 클로저로 작업 묶고, 에러 시 롤백
**영향**:
- `db/mod.rs:291-336` - delete_file()
- `db/mod.rs:362-409` - delete_files_in_folder()
- `db/mod.rs:412-443` - clear_all_data()
- `db/mod.rs:448-475` - delete_chunks_for_file()

### 결정 7: 크래시 핸들러

**문제**: `.expect()` 패닉 시 데이터 저장 없이 강제 종료
**결정**: panic::set_hook으로 크래시 로그 저장
**구현**: `dirs::data_dir()/com.anything.app/crash.log`에 기록
**영향**: `lib.rs:84-114`

### 결정 8: HWPX Read::take() 하드 제한

**문제**: by_index_raw 실패 시 사전 검증 우회 가능
**결정**: 실제 읽기에도 50MB 하드 제한 추가
**영향**: `hwpx.rs:326, 342` - take() 적용

### 결정 9: lineSpacing 조건 수정

**문제**: 기본 스타일 밖에서도 lineSpacing 적용 → 페이지 계산 오차
**결정**: `&& in_default_style` 조건 추가
**영향**: `hwpx.rs:671`

### 결정 10: 프로덕션 console.log 제거

**문제**: 31곳에서 디버그 정보 노출 가능
**결정**: esbuild drop 설정으로 프로덕션에서 자동 제거
**영향**: `vite.config.ts:27-29`

### 계획 파일
`~/.claude/plans/shimmering-soaring-hamming.md`

## 2026-01-31 - 사내 배포 최종 안정성 검증 (Phase 4)

**상황**: 30년차 개발자 관점의 비판적 코드 리뷰 후 Critical/Warning 이슈 발견
- 3개 병렬 에이전트로 Rust 코드 품질, 보안, 프론트엔드 분석 완료
- 전체 점수 85/100 - 배포 가능 수준

### 결정 11: VectorIndex expect() → graceful error handling

**문제**: 30개 이상의 `expect()` 호출 → lock poisoned 시 전체 앱 panic
**시나리오**:
- PDF 파싱 중 panic → catch_unwind로 복구
- 해당 스레드가 VectorIndex lock 잡고 있었으면 → Lock poisoned
- 다음 검색 시 전체 앱 crash

**결정**: `VectorError::LockPoisoned` 추가, 모든 expect() → `map_err(|_| VectorError::LockPoisoned)?`
**영향**:
- `search/vector.rs` - 25개 위치 수정
- `size()`, `id_map_size()`, `capacity()` → `unwrap_or(0)`
- `clear()` → best-effort (lock 실패 시 조용히 무시)

### 결정 12: FolderTree 비동기 setState 메모리 누수 방지

**문제**: `fetchFolderInfo()` 비동기 호출 후 컴포넌트 언마운트 시 setState 호출
**시나리오**:
- 사용자가 빠르게 폴더 추가/삭제 반복
- 비동기 invoke 완료 전 컴포넌트 언마운트
- 콘솔 경고 + 잠재적 메모리 누수

**결정**: `isMounted` 플래그로 언마운트 후 setState 호출 방지
**영향**: `FolderTree.tsx:59-85` - useEffect cleanup 추가

### 리뷰 결과 요약

**이미 잘된 부분** (수정 불필요):
- SQL Injection 방어 ✅ (파라미터 바인딩)
- Path Traversal 방어 ✅ (canonicalize)
- 압축 폭탄 방어 ✅ (size/ratio/entry 제한)
- SHA-256 무결성 검증 ✅ (모델+DLL)
- ErrorBoundary ✅ (main.tsx에서 App 감쌈)
- Crash Handler ✅ (panic hook)
- TypeScript 타입 안정성 ✅ (any 0개)

### 계획 파일
`~/.claude/plans/floating-stirring-hickey.md`

## 2026-02-08 - 프로덕션 종합 리뷰 Phase A+B+C (26건 개선)

**상황**: 사내 배포 전 종합 리뷰 결과 4 Critical + 8 Major + 20 Minor 발견 (프로덕션 점수 65/100)
**3개 병렬 에이전트**: Rust 백엔드, React 프론트엔드, 빌드/배포/설정 심층 분석

### 결정 13: panic = "unwind" 전환 (Critical)

**문제**: `panic = "abort"`가 PDF/XLSX 파서의 `catch_unwind` 완전 무효화 → 손상 파일 하나로 앱 전체 크래시
**결정**: `panic = "unwind"` + `strip = "debuginfo"`
**트레이드오프**: 바이너리 5-15% 증가 (안정성 >> 크기)
**영향**: `Cargo.toml:89,93` - 모든 catch_unwind 복원 (pdf.rs, mod.rs, pipeline.rs, lib.rs)

### 결정 14: VectorIndex TOCTOU 레이스컨디션 수정

**문제**: add()/remove()에서 read lock → release → write lock 패턴 → 사이에 다른 스레드 개입 가능
**결정**: 단일 write lock 스코프로 통합 + 고정 락 순서 (index → id_map → key_map → next_key)
**이유**: 데드락 방지를 위해 락 순서 고정, 자기 호출(self.remove()) 인라인화
**영향**: `search/vector.rs:107-176` - add(), remove() 완전 재작성

### 결정 15: 디스크 감지 캐싱 (OnceLock)

**문제**: PowerShell WMI 호출이 매번 실행됨 → 2번째 인덱싱부터도 2-3초 지연
**결정**: `OnceLock<Mutex<HashMap<char, DiskType>>>` 드라이브별 1회 캐싱
**영향**: `disk_info.rs` - detect_disk_type() 캐시 로직 추가

### 결정 16: CSS 디자인 시스템 체계화

**문제**: ErrorBoundary CSS 변수 5/7개 미정의 (에러 화면 안 보임), 9개 컴포넌트 하드코딩 컬러
**결정**:
- ErrorBoundary: `--bg-secondary` → `--color-bg-secondary` 등 5개 수정
- FileIcon, Tooltip, Input, RecentSearches, SettingsModal, SearchFilters: CSS 변수 마이그레이션
**이유**: 라이트/다크 테마 모두 정상 동작 확보
**영향**: 8개 프론트엔드 파일 수정

### 결정 17: devtools feature flag 분리

**문제**: `tauri/devtools`가 릴리스 빌드에 포함 → 사용자에게 DevTools 노출
**결정**: `[features] devtools = ["tauri/devtools"]` 분리, 개발 시 `--features devtools`
**영향**: `Cargo.toml` - features 섹션 재구성

### 계획 파일
`~/.claude/plans/wobbly-sauteeing-horizon.md`

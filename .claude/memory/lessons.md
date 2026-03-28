# DocuFinder 교훈 기록

> 세션에서 발생한 실수 패턴 + 방지 규칙

| 상황 | 실수 | 교훈 | 방지 규칙 |
|------|------|------|---------|
| Rust enum 변형 사용 | `ApiError::Internal` → 존재하지 않는 variant 사용 | enum variant는 사전에 정의 확인 필수 | Edit 전 Read로 해당 파일의 enum/struct 정의 먼저 확인 |
| Tauri trait 메서드 | `app.path()` 호출 시 `tauri::Manager` import 누락 | Tauri 매니저 메서드 호출 시 trait import 필수 | `app.path()`, `app.emit()` 등 사용 시 `use tauri::Manager` 확인 |
| 고차 함수 변환 | `.map()` → `.and_then()` 변경 시 괄호 불일치 | 클로저 시그니처 변경 시 반환값 타입+괄호 재검토 | 변경 전 원본 형태 확인 → 변환 후 괄호 짝 점검 |
| 새 기능 설계 | `auto_index_all_drives` Rust 명령 계획 → 기존 API 재활용이 더 적절 | 구현 전 기존 코드 탐색 우선 | 체크: (1) 관련 함수 grep (2) DTO 재사용 가능성 (3) FE에서 조합 가능한지 |
| 다중 파일 수정 | 5개 Rust 파일 + 4개 TS 파일 동시 수정 → 일관성 검증 어려움 | 의존성 순서 중요: 도메인 → 서비스 → 명령 → 프론트엔드 | 단계별 cargo check + 각 레이어 검증 후 다음 진행 |
| async fn에서 blocking I/O | `std::process::Command::output()`을 async fn에서 직접 사용 | Tokio 런타임 스레드 블로킹 → UI 프리징 | async fn 내 프로세스 실행은 `tokio::process::Command` 사용 + Cargo.toml에 `process` feature 추가 |
| Windows 경로 비교 | `starts_with(scope)` → case-sensitive 비교 | Windows 경로는 case-insensitive | `to_lowercase().starts_with()` 또는 헬퍼 함수로 통일 |
| 경로 정규화 | `canonicalize()`가 `\\?\` prefix 추가 | JS/TS에서 경로 비교 시 prefix 불일치 | `strip` 헬퍼: `p.replace(/^\\\\\?\\/, "")` 적용 후 비교 |
| React 모달 state | 모달 닫았다 재오픈 시 이전 state 잔류 | props 변경과 내부 state가 독립적 | key prop 변경 사용 또는 `useEffect([props])` → state 초기화 |
| React 훅 타입 정의 | updateToast 파라미터를 `type: string`으로 정의 → ToastType union 필요 | 공유 훅의 콜백 타입은 호출처 타입과 정확히 일치해야 함 | 훅 작성 시 호출처의 실제 타입 grep → import하여 재사용 |
| 리팩토링 후 import 정리 | 공통 함수 추출 후 기존 파일에 unused import 잔존 | 코드 이동 직후 원본 파일 import 정리 필수 | 함수 추출 → cargo check → warnings 전부 정리 → 다음 단계 |
| 미리보기 패널 연동 | 마우스 클릭으로 결과 선택 시 미리보기 안 열림 (키보드만 동작) | 새 UI 패널은 반드시 마우스+키보드 양쪽 트리거 확인 | 결과 선택 = onSelectResult prop + selectedIndex 연동 필수 |
| JS regex.test() + g 플래그 | `regex.test()` with `g` flag → lastIndex 변경으로 교대 매칭 실패 | split(/(pattern)/)의 홀수 인덱스가 매칭 부분 | regex.test() 대신 인덱스 기반 판단 (`i % 2 === 1`) |
| AppContainer 필드 접근 | `container.db_path()` 메서드 호출 → 실제로는 `pub db_path: PathBuf` 필드 | 서비스 접근 패턴 확인: 메서드 vs 필드 | `container.db_path.to_string_lossy()` 사용 |
| get_connection 시그니처 | `db::get_connection(&str)` 호출 → 실제로는 `&Path` 인자 | DB 함수 시그니처 반드시 확인 | `std::path::Path::new(&db_path)` 래핑 필요 |
| Rust borrow in loop | `Vec<&str>`로 split 결과 빌려서 같은 루프에서 `*remaining =` 재할당 → borrow 에러 | loop 내에서 원본 수정 필요하면 `Vec<String>`으로 소유해야 함 | remaining을 수정하는 루프에서는 반드시 `split().map(String::from).collect()` 사용 |
| chrono NaiveDate API | `date_naive().with_month(1)` → Datelike trait 미 import + and_hms_opt 체인 타입 추론 실패 | chrono의 NaiveDate 메서드는 trait import + 명확한 체인 필요 | `use chrono::Datelike;` + `NaiveDate::from_ymd_opt()` → `and_hms_opt()` → `and_utc()` 순서 |
| 계획 품질 피드백 | 첫 계획이 너무 표면적 → "대충 쓴 거 같다" 피드백 | 계획 작성 시 비판적 검토 섹션 포함, 설계 결정 근거+대안+리스크 명시 필수 | "왜 이 방식인가" + "다른 방식은 왜 안 되나" + "FTS5 NOT 제약 같은 기술적 한계" 반드시 포함 |
| std::sync::OnceLock 마이그레이션 | once_cell → OnceLock 전환 시도 → `get_or_try_init` unstable (feature gate `once_cell_try`) | Rust 표준 라이브러리 API 안정화 상태 확인 필수 | OnceLock은 `get_or_init`만 stable, `get_or_try_init`은 nightly. fallible init 필요 시 once_cell 유지 |
| 기능 상태 판단 | Explore 에이전트가 "스텁"이라 보고 → 실제로는 전부 완전 구현체 | 에이전트 보고서를 맹신하면 안 됨, 핵심 파일은 직접 Read로 확인 | 기능 존재/스텁 판단 시 반드시 (1) 프론트 hook (2) 백엔드 command (3) DB 스키마 세 곳 직접 확인 |

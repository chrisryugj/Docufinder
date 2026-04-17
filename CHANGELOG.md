# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/lang/ko/).

## [2.3.2] - 2026-04-17

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

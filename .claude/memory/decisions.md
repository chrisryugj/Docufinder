# DocuFinder 아키텍처 결정 기록

> 최근 주요 결정만 기록. 이전 결정은 git 히스토리 참고.

## 2026-02-22 - 수익화 전략 v3 수립

**배경**: 22개 GitHub 프로젝트 포트폴리오 분석 → 월 30만원 구독 수익 목표
**법적 검토 결과**:
- 법제처 API: 공공누리 제1유형 → 상업적 이용 OK (출처표시만)
- 공무원 겸직: 국가공무원법 64조 리스크 → 겸직허가 신청 필요
- 부모 법인: 실질 운영 참여 어려움 → 형식적 명의대여 리스크

**결정: 3경로 하이브리드 전략**
1. 즉시: 오픈소스 + 후원 모델 (GitHub Sponsors, BMC)
2. 병행: 겸직허가 비공식 문의
3. 허가 결과 따라: 정식 상용화 or 법인+자동화

**제품 우선순위**: korean-law-mcp Pro > Docufinder Pro > hwp2html API
**상세 계획**: `.claude/plans/crispy-doodling-aurora.md`

## 2026-02-22 - 프로덕션 리뷰 4차 (88/100)

**P1 수정 5건**: DB VACUUM, unsafe set_var SAFETY, aria-activedescendant, SettingsModal 에러, ort RC 확인
**P2 수정 3건**: SearchResultItem 분리 (629→475줄), CI/CD 워크플로우, 결과 페이지네이션 + 설정 연동
**남은 P2 3건**: 이중 FS 순회 통합, data_root 설정, PDF timeout 조정

## 2026-02-21 - 프로덕션 리뷰 2차 운영 안정성 6건

- 벡터 인덱싱 부분실패 시 완료 마킹 방지 (`file_failed_chunks` 카운터)
- 프리패치 버퍼 축소 (4→2)
- WatchManager 파일 크기 제한 (`max_file_size_mb`)
- FilenameCache truncated 시 DB fallback
- 인덱싱 실패 시 상태 복구 ("failed" 설정)

## 2026-02-08 - 프로덕션 종합 리뷰 Phase A+B+C (26건)

**Critical**: `panic = "unwind"` 전환, VectorIndex TOCTOU 수정 (단일 write lock)
**Major**: PDF 스레드 카운터, 디스크 감지 캐싱(OnceLock), CSS 디자인 시스템 체계화 등
**Minor**: 테마 플래시 방지, TXT 사이즈 리밋, devtools feature flag 등

## 2026-01-31 - 저사양 환경 성능 최적화

- **2단계 분리 인덱싱**: 메타스캔 → 컨텐츠 파싱 (HDD 대응)
- **HDD 단일 스레드**: SSD/HDD 자동 감지 → 적응형 스레딩
- **유휴 감지**: GetLastInputInfo, 3초 유휴 시 파싱
- **full_content 제거**: 응답 ~90% 경량화
- **FilenameCache**: 인메모리 캐시 (~5ms, Everything 수준)

## 2026-01-31 - 사내 배포 보안 강화

- 업데이터 완전 비활성화 (사내망 차단 대응)
- SHA-256 무결성 검증 (모델 + DLL)
- 다운로드 타임아웃/크기 제한 (30초/500MB)
- 압축 폭탄 방어 (50MB/200MB/100:1/1000엔트리)
- DB 트랜잭션 원자성 보장 (DELETE 작업)
- 크래시 핸들러 (panic hook → crash.log)

## 2026-01-26 - Search Commands → SearchService 전환

- Thin Commands 패턴 적용 (640→170줄)
- tokenizer/reranker 지원 추가
- AppError → ApiError 변환 구현

## 2026-01-21 - 클린 아키텍처 도입 + AppContainer 마이그레이션

- Rust: Domain/Application/Infrastructure 레이어 분리
- AppState → AppContainer 완전 대체 (DI 패턴)
- Repository Trait (async_trait + Send + Sync)
- Commands 전환은 Search만 적용 (나머지는 오버엔지니어링)

## 2026-01-18 - 2단계 인덱싱 + VectorWorker

- FTS 즉시 완료 → 벡터 백그라운드 (std::thread)
- VectorIndex RwLock 적용 (unsafe impl 제거)
- 파일명 FTS5 테이블 + 4번째 검색 모드

## 2026-01-18 - 폰트 로컬 번들링 + CSP 강화

- Pretendard Variable woff2 로컬 번들 (~2MB)
- 엄격한 CSP: `script-src 'self'`

## 2026-01-17 - UI/UX 기반 결정

- 범용 useToast 훅 + Toast 컴포넌트 분리
- 최근검색 `string[]` → `RecentSearch { query, timestamp }` 마이그레이션
- 즐겨찾기 폴더 (DB 컬럼 방식)
- 하위폴더 포함 설정 실제 연동

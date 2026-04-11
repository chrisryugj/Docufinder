# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/lang/ko/).

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

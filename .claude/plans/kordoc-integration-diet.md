# kordoc 이식 + 다이어트 + 문서 미리보기 계획

## Phase 1: 다이어트 (기능 제거/단순화)

### 1A. 프론트엔드 제거
- [ ] AI Answer Panel 제거 (AiAnswerPanel.tsx + useAiSearch hook + 관련 타입)
- [ ] 만료일 스캔 제거 (ExpiryAlertModal.tsx + useExpiryAlert hook)
- [ ] 필터 프리셋 제거 (useFilterPresets hook + UI 연동)
- [ ] 컬러 프리셋 피커 제거 (ColorPresetPicker.tsx + 설정 연동)
- [ ] 면책 모달 제거 (DisclaimerModal.tsx + 첫 실행 체크)
- [ ] 내보내기 단순화: XLSX/JSON/ZIP 제거, CSV만 유지

### 1B. 백엔드 제거
- [ ] Reranker 제거 (reranker/ 디렉토리 + hybrid.rs 연동 + 모델 다운로드)
- [ ] AI/Gemini RAG 제거 (ai/ + commands/ai.rs + reqwest 의존성 검토)
- [ ] 만료일 스캔 백엔드 제거 (commands/expiry.rs)
- [ ] TextRank 요약 제거 → 첫 500자 미리보기로 대체
- [ ] 내보내기 백엔드 단순화 (XLSX/JSON/ZIP 커맨드 + rust_xlsxwriter 의존성)
- [ ] Cargo.toml 불필요 의존성 정리

### 1C. 정리
- [ ] 사용하지 않는 타입/인터페이스 정리
- [ ] 빌드 검증 (pnpm build + cargo check)

## Phase 2: kordoc 사이드카 통합

### 2A. 사이드카 인프라
- [ ] Node.js 최소 런타임 번들링 전략 결정
- [ ] kordoc CLI 래퍼 스크립트 작성 (stdin → JSON stdout)
- [ ] Tauri sidecar 설정 (tauri.conf.json)
- [ ] Rust에서 sidecar 호출 유틸리티 (spawn + JSON 파싱)

### 2B. 파서 교체
- [ ] 기존 Rust HWPX 파서 → kordoc 위임으로 교체
- [ ] HWP5 바이너리 파싱 지원 추가 (kordoc 경유)
- [ ] 배포용(Distribution-locked) HWP 자동 복호화 연동
- [ ] 손상된 HWP 파일 복구 파싱 (lenient CFB)
- [ ] kordoc 마크다운 출력 → ParsedDocument 변환 로직
- [ ] 파일 타입 라우팅 수정 (mod.rs: .hwp → HWP5, .hwpx → HWPX)

### 2C. 마크다운 품질 향상
- [ ] 헤딩 계층 감지 (H1-H3)
- [ ] 테이블 구조 보존 (colspan/rowspan → 마크다운 테이블)
- [ ] 각주/미주 인라인 삽입
- [ ] 하이퍼링크 보존
- [ ] OLE alt-text 노이즈 필터링
- [ ] 균등배분 텍스트 정규화

## Phase 3: 문서같은 미리보기

### 3A. 마크다운 렌더러
- [ ] 마크다운 → HTML 렌더링 라이브러리 선택/통합
- [ ] 헤딩 스타일링 (크기/굵기 계층)
- [ ] 테이블 렌더링 (border, padding, header 구분)
- [ ] 코드 블록, 리스트, 인용문 스타일링
- [ ] 링크 클릭 처리 (외부 브라우저 열기)

### 3B. 미리보기 패널 리디자인
- [ ] PreviewPanel → 마크다운 렌더러 통합
- [ ] 문서 스타일 CSS (여백, 줄간격, 폰트 — 실제 문서 느낌)
- [ ] 페이지 구분선 표시
- [ ] 각주 하단 표시
- [ ] 다크/라이트 테마 대응

### 3C. PDF 미리보기 개선
- [ ] PDF도 kordoc 경유 파싱 검토
- [ ] 또는 기존 pdf-extract 출력의 마크다운 변환 개선

## Phase 4: 통합 검증
- [ ] pnpm build 성공
- [ ] cargo check + clippy
- [ ] pnpm tauri:dev 통합 테스트
- [ ] HWP5 배포용 문서 인덱싱 테스트
- [ ] 미리보기 렌더링 검증

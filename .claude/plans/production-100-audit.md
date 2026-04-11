# 프로덕션 100점 감사 — 종합 리포트 v2

## 현재 평가: 85/100점

**분석 범위**: 프론트엔드 UI/UX, Rust 백엔드, 빌드/설치, 검색 UX
**분석 일시**: 2026-04-11
**기준**: 새 PC 설치 → 실행 → 일상 사용까지 전 과정

---

## 이전 감사(78점) 대비 수정 완료 확인 ✅

| 항목 | 상태 |
|------|------|
| Skip-to-main-content | ✅ App.tsx:299-306 |
| open_file() 감시 폴더 검증 | ✅ file.rs:94-119 |
| OCR SHA-256 해시 | ✅ model_downloader.rs:54-56 |
| DB 무결성 검사 | ✅ lib.rs:466-484 |
| crash.log 날짜 로테이션 | ✅ lib.rs:322-370 |
| DB retry 지수 백오프 | ✅ db/mod.rs:14-51 (100→200→400ms) |
| AI 답변 복사 버튼 | ✅ AiAnswerPanel.tsx CopyableActionBar |
| Spinner 컴포넌트 통일 | ✅ ui/Spinner.tsx |
| 버전 동기화 스크립트 | ✅ scripts/bump-version.ps1 |
| prefers-reduced-motion | ✅ animations.css:309-323 |

---

## 🔴 CRITICAL (3건) — 릴리스 차단

### C-1. 코드 서명 미설정
- **파일**: tauri.conf.json:46 → `certificateThumbprint: null`
- **영향**: Windows SmartScreen이 설치 차단, 기업 환경 배포 불가
- **대응**: 내부용은 self-signed (create-cert.ps1), 공개 배포는 CA 인증서 필요
- **난이도**: 중 (인증서 구매/설정)

### C-2. download-model.ps1 ONNX Runtime SHA-256 빈값
- **파일**: scripts/download-model.ps1:19 → `$ONNX_RUNTIME_SHA256 = ""`
- **영향**: ONNX Runtime DLL 무결성 미검증 → 공급망 공격 벡터
- **대응**: 한 번 다운로드 후 Get-FileHash로 계산, 값 채우기
- **참고**: Rust 코드(model_downloader.rs:44-45)에는 이미 해시 있음 → 스크립트만 누락
- **난이도**: 하 (5분)

### C-3. WebView2 런타임 부트스트래퍼 누락
- **영향**: Windows 10 구형 빌드에서 WebView2 없으면 앱 실행 불가, 흰 화면
- **현재**: WiX에서 WebView2 설치 체크 없음
- **대응**: 
  - 옵션 A: WiX에 WebView2 부트스트랩 추가 (tauri가 지원)
  - 옵션 B: 최소 요구사항 문서화 (Windows 10 21H2+)
- **난이도**: 중

---

## 🟠 HIGH (6건) — 릴리스 전 수정 권장

### H-1. 모델 다운로드 실패 시 사용자 피드백 없음
- **파일**: lib.rs:136-143
- **현재**: `model-download-status: "failed"` 이벤트만 발생, UI에서 처리 안 함
- **영향**: 새 PC에서 시맨틱 모델 다운로드 실패 → 왜 시맨틱 검색 안 되는지 모름
- **대응**: 프론트엔드에서 `model-download-status` 이벤트 리스닝 → 실패 시 토스트/배너
- **난이도**: 하

### H-2. 폴더 0개 상태에서 경로 검증 바이패스
- **파일**: file.rs:103-104
- **현재**: 감시 폴더 미등록 시 모든 파일 열기 허용
- **영향**: 첫 실행 시 `open_file` IPC로 시스템 파일 접근 가능
- **대응**: 폴더 0개여도 BLOCKED_PATH_PATTERNS 검증은 이미 적용됨(line 88-91), 추가 필요한지 재검토
- **실제 위험도**: 낮음 (이미 시스템 폴더 차단 + 확장자 제한 있음, 사용자가 직접 IPC 호출해야 함)
- **난이도**: 하

### H-3. 검색 0건 결과 시 UX 부재
- **현재**: 검색어 입력 후 결과 0건이면 빈 화면
- **대응**: "결과 없음" 메시지 + 검색어 수정 제안 + 다른 검색 모드 제안
- **난이도**: 중

### H-4. 한글 텍스트 하이라이트 바이트 인덱스 불일치
- **파일**: HighlightedText.tsx:157 — "Rust 바이트 인덱스 버그 회피" 주석
- **현재**: Rust UTF-8 바이트 인덱스 vs JS UTF-16 코드 유닛 불일치
- **완화**: snippet `[[HL]]` 마커 우선 사용으로 대부분 회피
- **영향**: snippet 파싱 실패 시 한글 하이라이트 2-3글자 어긋남
- **난이도**: 중 (Rust 측에서 char index로 변환 필요)

### H-5. 설정값 저장 시 범위 검증 누락
- **파일**: settings.rs:279-289
- **현재**: 로드 시에만 clamp, 저장 시 무검증
- **영향**: JSON 직접 편집 시 비정상 값 저장 가능
- **대응**: `update_settings` 커맨드에서도 clamp 적용
- **난이도**: 하

### H-6. .tmp 모델 파일 잔여물 정리 누락
- **파일**: model_downloader.rs
- **현재**: 다운로드 중 크래시 시 `.tmp` 파일 잔류
- **대응**: 앱 시작 시 `models/` 내 `.tmp` 파일 삭제 로직 추가
- **난이도**: 하

---

## 🟡 MEDIUM (12건) — 품질 향상

### UX 개선

| ID | 이슈 | 파일 | 설명 |
|----|------|------|------|
| M-1 | 패러다임 전환 시 쿼리 소멸 | useSearch.ts:341-355 | instant↔natural↔question 전환 시 입력 유지 |
| M-2 | 파일명 검색 실패 무시 | useSearch.ts:286-300 | allSettled 후 에러 토스트 없이 빈 결과 |
| M-3 | 결과 내 검색(refine) UX 모호 | SearchFilters.tsx | snippet vs full_content 어디 검색하는지 불명확 |
| M-4 | SmartQueryInfo 필터 로직 미설명 | SmartQueryInfo.tsx | AND/OR 어떻게 적용되는지 사용자 모름 |
| M-5 | 필터 초기화 버튼 미렌더링 | SearchFilters.tsx:67-69 | handleReset 함수 있으나 JSX에 없음 |
| M-6 | 빈 북마크/최근검색 안내 부재 | BookmarkList, RecentSearches | "여기에 표시됩니다" 같은 빈 상태 메시지 없음 |

### 백엔드 안정성

| ID | 이슈 | 파일 | 설명 |
|----|------|------|------|
| M-7 | 크래시 로그 fsync 누락 | lib.rs:363-369 | write 후 sync_all() 미호출 → 전원 차단 시 유실 |
| M-8 | 벡터 인덱스 불일치 자동 복구 없음 | vector.rs:230-250 | usearch size > id_map → 로그만, 재빌드 없음 |
| M-9 | Preview 요청 디바운스 부족 | PreviewPanel.tsx:294 | 150ms로 빠른 화살표 키 이동 시 요청 쌓임 |
| M-10 | AI 요약 스트리밍 정리 불완전 | PreviewPanel.tsx:247-271 | 이전 파일 요약이 새 파일에 잠깐 표시 가능 |
| M-11 | OCR 다운로드 실패 무통보 | lib.rs:163-189 | 이벤트만 발생, UI에서 안 들음 |
| M-12 | filteredResults sort 비효율 | useSearch.ts:415-517 | 매 필터 변경 시 전체 배열 재정렬 |

---

## 🟢 LOW (8건) — 개선 가능

| ID | 이슈 | 파일 | 설명 |
|----|------|------|------|
| L-1 | 쿼리 길이 프론트 검증 없음 | SearchBar.tsx | 1000자 초과 → 백엔드 에러만 |
| L-2 | 날짜 필터 "30일" ≠ 달력 월 | useSearch.ts:451-461 | 정확히 30×86400초, 달력 기준 아님 |
| L-3 | Long path >260자 Windows 제한 | file.rs:122-126 | `\\?\` 접두사 제거 → 260자 초과 시 실패 |
| L-4 | metadata-only 확장자 하드코딩 | pipeline.rs:228-239 | SUPPORTED_EXTENSIONS 상수 미사용 |
| L-5 | WCAG AA 대비 미검증 | 전역 | 자동화 테스트 없음 |
| L-6 | question textarea 시각적 잘림 | SearchBar.tsx:117-122 | 96px 제한, 긴 텍스트 잘려 보임 |
| L-7 | Hybrid 모드 UI 미노출 | types/search.ts:66-69 | 백엔드 존재하나 SEARCH_MODES에 없음 |
| L-8 | ORT RC 버전 고정 | Cargo.toml:48 | `ort = "=2.0.0-rc.11"` → stable 미출시 |

---

## 빌드/배포 이슈

| 심각도 | 이슈 | 현황 |
|--------|------|------|
| 🟠 HIGH | 프론트엔드 테스트 없음 | CI에 React 테스트 없음 (cargo test만) |
| 🟡 MED | bundle-kordoc.ps1 하드코딩 경로 | `c:\github_project\kordoc` 기본값 |
| 🟡 MED | CI에서 실제 모델 포함 MSI 테스트 안 함 | 플레이스홀더로 대체 |
| 🟡 MED | 업데이트 서명 키 로테이션 정책 없음 | 키 분실 시 복구 불가 |
| 🟢 LOW | 시스템 요구사항 문서 없음 | 디스크/RAM 요구사항 미기재 |

---

## 점수 산출 근거

| 영역 | 점수 | 비고 |
|------|------|------|
| 설치/첫 실행 | 7/10 | 코드 서명 없음, WebView2 미체크 |
| 에러 핸들링 | 9/10 | ErrorBoundary + 토스트 + 크래시 로그 우수 |
| 로딩 상태 | 9/10 | 스켈레톤, 진행률 바, 스트리밍 커서 완비 |
| 검색 UX | 8/10 | 0건 결과 UX 부재, 하이라이트 바이트 이슈 |
| 접근성 | 8.5/10 | skip link, 키보드 네비, ARIA 우수 |
| 테마 | 9/10 | 플래시 방지, CSS 변수 완전 분리 |
| 보안 | 8/10 | CSP, 경로 검증, SQL 바인딩 양호 |
| 백엔드 안정성 | 8.5/10 | PDF 타임아웃, DB retry, 벡터 검증 양호 |
| 배포 파이프라인 | 7/10 | 서명 미설정, 프론트 테스트 없음 |
| **종합** | **85/100** | |

---

## 100점 달성 로드맵

### Phase 1: 필수 수정 (85→92점)
1. C-2: ONNX Runtime SHA-256 채우기 (5분)
2. H-1: 모델 다운로드 실패 UI 피드백 (30분)
3. H-3: 검색 0건 결과 메시지 (30분)
4. H-5: 설정 저장 시 범위 검증 (15분)
5. H-6: .tmp 파일 시작 시 정리 (15분)
6. M-5: 필터 초기화 버튼 렌더링 (10분)
7. M-6: 빈 상태 안내 메시지 (20분)
8. M-7: crash log fsync (5분)

### Phase 2: 품질 강화 (92→96점)
1. C-1: 코드 서명 설정 (외부 의존)
2. C-3: WebView2 체크 또는 요구사항 문서화
3. H-4: 하이라이트 바이트 인덱스 수정
4. M-1: 패러다임 전환 시 쿼리 보존
5. M-9: Preview 디바운스 300ms로 증가
6. M-12: filteredResults 정렬 최적화

### Phase 3: 최종 다듬기 (96→100점)
1. 프론트엔드 테스트 추가 (Vitest)
2. 시스템 요구사항 문서화
3. WCAG AA 대비 검증
4. 업데이트 키 로테이션 정책 수립

---

## 검증 방법

```bash
# TypeScript 빌드
pnpm build

# Rust 검증
cd src-tauri && cargo check && cargo clippy -- -D warnings

# 통합 테스트
pnpm tauri:dev
# → 새 폴더 추가 → 검색 → AI 질문 → 설정 변경 → 다크모드 → 키보드 전용 사용
```

---

**태스크 수**: Phase 1: 8건 / Phase 2: 6건 / Phase 3: 4건 = **총 18건**

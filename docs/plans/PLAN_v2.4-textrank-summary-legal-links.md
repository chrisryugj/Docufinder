# Implementation Plan: v2.4 TextRank 추출적 요약 + 법령 참조 링크

**Status**: 🔄 In Progress
**Started**: 2026-03-20 | **Last Updated**: 2026-03-20
**Plan Size**: Medium (5 phases, ~10 hours)

---

**⚠️ CRITICAL**: Phase 완료 후 Quality Gate 전 항목 통과 필수. 실패 시 다음 Phase 진행 금지.

---

## 📋 Overview

### Feature Description
1. **TextRank 추출적 요약**: 문서 전체 텍스트에서 핵심 문장 3~5개를 자동 추출하여 미리보기 패널 상단에 표시. Rust 구현, 오프라인, 추가 모델 불필요.
2. **법령 참조 링크**: 문서 내 "제N조", "법률 제N호" 등 법령 참조를 정규식으로 감지하여 law.go.kr 링크로 변환. 미리보기 패널에서 인터랙티브 링크 렌더링.

### Success Criteria
- [ ] "요약 보기" 버튼 클릭 → 3~5문장 요약이 미리보기 패널 상단에 표시
- [ ] 요약 생성 시간 < 3초 (일반 문서 기준)
- [ ] 법령 참조 패턴 자동 감지 + 클릭 시 law.go.kr 링크 오픈
- [ ] 시맨틱 검색 OFF 시에도 TF-IDF 기반 요약 동작 (fallback)
- [ ] 기존 UI/UX와 유기적 통합 (별도 화면 X)

### User Impact
- 긴 보고서의 핵심 내용을 빠르게 파악 (검토 시간 대폭 절감)
- 법령 참조 확인 시 법제처 수동 검색 불필요

---

## 🏗️ Architecture Decisions

| Decision | Rationale | Trade-offs |
|----------|-----------|------------|
| **TF-IDF 기반 TextRank (기본)** | 임베딩 모델 없이도 동작, 빠름 | 시맨틱 유사도보다 정확도 약간 낮음 |
| **임베딩 기반 TextRank (옵션)** | KoSimCSE 모델 로드 시 더 높은 품질 | 모델 로드 필요, 느림 |
| **별도 IPC 커맨드** (`generate_summary`) | Preview 로딩과 분리하여 UX 개선 (요약 = 온디맨드) | IPC 호출 1회 추가 |
| **법령 링크는 프론트엔드 처리** | 정규식 매칭은 가벼움, Rust 왕복 불필요 | 복잡한 패턴은 한계 있음 |
| **요약 캐싱 없음 (v2.4)** | 간결한 구현, 인덱싱 파이프라인 변경 불필요 | 같은 문서 재요약 시 재계산 |

---

## 🚀 Implementation Phases

### Phase 1: TextRank 알고리즘 (Rust 핵심 로직)
**Goal**: TF-IDF 기반 TextRank 알고리즘 구현 + 단위 테스트 | **Time**: 2.5h | **Status**: ⏳ Pending

#### Tasks

**🔴 RED: Write Failing Tests First**
- [ ] **Test 1.1**: TextRank 핵심 함수 테스트 → `src-tauri/src/search/textrank.rs`
  - `build_tfidf_vectors()`: 문장 목록 → TF-IDF 벡터
  - `build_similarity_matrix()`: 벡터 목록 → 유사도 매트릭스
  - `rank_sentences()`: 매트릭스 → 랭크 스코어 (PageRank 수렴)
  - `summarize()`: 전체 파이프라인 (텍스트 → 상위 N 문장)
  - Edge cases: 빈 텍스트, 문장 1개, 동일 문장 반복, 매우 긴 텍스트

**🟢 GREEN: Implement to Make Tests Pass**
- [ ] **Task 1.2**: `src-tauri/src/search/textrank.rs` 신규 모듈 생성
  - `split_sentences_unlimited()`: sentence.rs 활용하되 MAX 제한 해제
  - `build_tfidf_vectors()`: 문장별 TF-IDF 벡터 생성 (형태소 분석 없이 음절 n-gram)
  - `build_similarity_matrix()`: 코사인 유사도 NxN 매트릭스
  - `power_iteration()`: PageRank 반복 계산 (damping=0.85, max_iter=100, epsilon=1e-6)
  - `summarize(text: &str, num_sentences: usize) -> Vec<RankedSentence>`
- [ ] **Task 1.3**: `src-tauri/src/search/mod.rs`에 `pub mod textrank;` 추가

**🔵 REFACTOR: Clean Up Code**
- [ ] **Task 1.4**: sentence.rs의 split_sentences를 내부적으로 재활용 (공통 로직 추출)

#### Quality Gate ✋
- [ ] `cargo test --lib search::textrank` 전 테스트 통과
- [ ] `cargo check` 성공
- [ ] 한국어/영문 문서 모두 요약 품질 확인 (테스트 내)

---

### Phase 2: IPC 커맨드 + 서비스 레이어
**Goal**: Tauri IPC 커맨드로 요약 기능 노출 | **Time**: 2h | **Status**: ⏳ Pending

#### Tasks

**🔴 RED: Write Failing Tests First**
- [ ] **Test 2.1**: 서비스 레이어 테스트 (가능한 범위)
  - 빈 파일 경로 → Validation 에러
  - 존재하지 않는 파일 → 빈 요약 반환
  - 정상 청크 → 요약 반환

**🟢 GREEN: Implement to Make Tests Pass**
- [ ] **Task 2.2**: `commands/preview.rs`에 `generate_summary` 커맨드 추가
  ```rust
  #[tauri::command]
  pub async fn generate_summary(
      file_path: String,
      num_sentences: Option<usize>,  // 기본 3
      state: State<'_, RwLock<AppContainer>>,
  ) -> ApiResult<SummaryResponse>
  ```
- [ ] **Task 2.3**: `SummaryResponse` DTO 정의
  ```rust
  pub struct SummaryResponse {
      pub sentences: Vec<SummarySentence>,
      pub total_sentences: usize,
      pub generation_time_ms: u64,
  }
  pub struct SummarySentence {
      pub text: String,
      pub score: f32,
      pub page_number: Option<i64>,
      pub location_hint: Option<String>,
  }
  ```
- [ ] **Task 2.4**: `lib.rs` generate_handler에 `commands::preview::generate_summary` 등록

**🔵 REFACTOR: Clean Up Code**
- [ ] **Task 2.5**: 에러 핸들링 패턴 기존 커맨드와 일관성 맞추기

#### Quality Gate ✋
- [ ] `cargo check` 성공
- [ ] `cargo test` 전체 통과
- [ ] IPC 시그니처 정상 등록 확인

---

### Phase 3: 프론트엔드 요약 UI
**Goal**: 미리보기 패널에 요약 섹션 통합 | **Time**: 2h | **Status**: ⏳ Pending

#### Tasks

**🔴 RED: 타입 정의 먼저**
- [ ] **Task 3.1**: `src/types/api.ts`에 SummaryResponse, SummarySentence 타입 추가

**🟢 GREEN: UI 구현**
- [ ] **Task 3.2**: PreviewPanel에 요약 상태 + 버튼 추가
  - "요약 보기" 버튼 (미리보기 헤더 영역)
  - 로딩 스피너 (요약 생성 중)
  - 요약 결과 표시 영역 (패널 상단, 접을 수 있는 섹션)
  - 각 요약 문장 클릭 → 해당 위치로 스크롤
- [ ] **Task 3.3**: `invoke<SummaryResponse>("generate_summary", { filePath })` 호출 로직
- [ ] **Task 3.4**: 요약 문장에 페이지/위치 뱃지 표시

**🔵 REFACTOR: Clean Up Code**
- [ ] **Task 3.5**: 스타일 기존 PreviewPanel 디자인과 일관성

#### Quality Gate ✋
- [ ] `pnpm build` (tsc) 성공
- [ ] PreviewPanel에서 요약 버튼 → 요약 표시 E2E 동작
- [ ] 다크 모드/라이트 모드 모두 정상 표시

---

### Phase 4: 법령 참조 링크
**Goal**: 미리보기 텍스트 내 법령 참조를 인터랙티브 링크로 변환 | **Time**: 2h | **Status**: ⏳ Pending

#### Tasks

**🔴 RED: Write Failing Tests First**
- [ ] **Test 4.1**: 법령 패턴 추출 유틸리티 테스트 (프론트엔드)
  - "제15조" → `{ type: "article", number: "15", url: "..." }`
  - "법률 제12345호" → `{ type: "law", number: "12345", url: "..." }`
  - "○○법 시행령" → `{ type: "enforcement_decree", name: "○○법", url: "..." }`
  - "행정절차법 제21조" → 법령명 + 조항 추출
  - 일반 텍스트 (매칭 없음) → 빈 배열

**🟢 GREEN: Implement to Make Tests Pass**
- [ ] **Task 4.2**: `src/utils/legalReference.ts` 신규 유틸리티
  - `extractLegalReferences(text: string): LegalReference[]`
  - 정규식 패턴:
    - `([\uAC00-\uD7A3]+법)\s*제(\d+)조` — 법률명 + 조항
    - `법률\s*제(\d+)호` — 법률 번호
    - `([\uAC00-\uD7A3]+법)\s*시행령` — 시행령
    - `([\uAC00-\uD7A3]+법)\s*시행규칙` — 시행규칙
    - `제(\d+)조(?:의(\d+))?` — 독립 조항 (법령명 없음)
  - URL 생성: `https://law.go.kr/법령/검색?query={법령명+조항}`
- [ ] **Task 4.3**: `LegalReference` 타입 정의
  ```typescript
  interface LegalReference {
    text: string;       // 원본 매칭 텍스트
    lawName?: string;   // 법령명
    article?: string;   // 조항 번호
    type: 'article' | 'law_number' | 'enforcement_decree' | 'enforcement_rule';
    url: string;        // law.go.kr 링크
    start: number;      // 원본 텍스트 내 위치
    end: number;
  }
  ```

**🟢 GREEN: PreviewPanel 통합**
- [ ] **Task 4.4**: PreviewPanel의 `highlightText` 함수 확장
  - 기존 검색어 하이라이트 + 법령 참조 링크 병합 렌더링
  - 법령 링크는 밑줄 + 아이콘 + 클릭 시 `shell.open(url)` (Tauri)
- [ ] **Task 4.5**: 법령 참조 토글 설정 (Settings → 법령 링크 표시 ON/OFF)

**🔵 REFACTOR: Clean Up Code**
- [ ] **Task 4.6**: 정규식 패턴 상수화, 유틸리티 분리

#### Quality Gate ✋
- [ ] `pnpm build` 성공
- [ ] 법령 포함 문서에서 링크 정상 표시
- [ ] 일반 문서(법령 없음)에서 부작용 없음

---

### Phase 5: 통합 검증 + 마무리
**Goal**: 전체 기능 E2E 검증 + 빌드 확인 | **Time**: 1.5h | **Status**: ⏳ Pending

#### Tasks

- [ ] **Task 5.1**: `cargo check` + `cargo test` + `cargo clippy` 통과
- [ ] **Task 5.2**: `pnpm build` (tsc) 통과
- [ ] **Task 5.3**: E2E 시나리오 검증
  - 미리보기 열기 → 요약 버튼 → 요약 표시
  - 법령 포함 문서 미리보기 → 링크 렌더링
  - 시맨틱 OFF 상태에서 요약 동작 확인
  - 빈 문서 / 짧은 문서 (문장 3개 미만) 처리
- [ ] **Task 5.4**: CLAUDE.md 업데이트 (v2.4 완료 표기)
- [ ] **Task 5.5**: activeContext.md 업데이트

#### Quality Gate ✋
- [ ] 전체 빌드 파이프라인 통과
- [ ] 기존 기능 회귀 없음
- [ ] 커밋 준비 완료

```bash
cd c:/github_project/Docufinder && cargo check 2>&1 | tail -5
cd c:/github_project/Docufinder && cargo test 2>&1 | tail -10
cd c:/github_project/Docufinder && pnpm build 2>&1 | tail -5
```

---

## ⚠️ Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TextRank 한국어 품질 낮음 | Medium | Medium | TF-IDF 대신 음절 n-gram 사용, 형태소 분석 선택적 적용 |
| 임베딩 모델 미로드 시 fallback | Low | High | TF-IDF 기반을 기본으로, 임베딩은 옵션 부스트 |
| 긴 문서 요약 시간 초과 | Low | Medium | 문장 수 제한 (최대 200문장), 타임아웃 설정 |
| 법령 정규식 오탐 (false positive) | Medium | Low | 패턴 보수적 설정, "제N조" 단독은 문맥 확인 |
| law.go.kr URL 구조 변경 | Low | Low | URL 생성 로직 상수화하여 유지보수 용이 |

---

## 📊 Progress Tracking

| Phase | Estimated | Actual | Status |
|-------|-----------|--------|--------|
| Phase 1: TextRank 알고리즘 | 2.5h | - | ⏳ |
| Phase 2: IPC + 서비스 | 2h | - | ⏳ |
| Phase 3: 프론트엔드 요약 UI | 2h | - | ⏳ |
| Phase 4: 법령 참조 링크 | 2h | - | ⏳ |
| Phase 5: 통합 검증 | 1.5h | - | ⏳ |
| **Total** | **10h** | - | 0% |

---

## 📝 Notes & Learnings
- sentence.rs의 `MAX_SENTENCES_PER_CHUNK = 5` 제한을 TextRank에서는 해제 필요
- 법령 링크 URL은 `shell.open()` (Tauri API)으로 기본 브라우저에서 오픈
- 요약은 온디맨드 (버튼 클릭 시 생성) — 자동 생성은 UX 부담

---
**Next Action**: Phase 1 시작 (TextRank 알고리즘 구현) | **Blocked By**: None

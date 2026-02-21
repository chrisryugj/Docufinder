# 프로덕션 리뷰 수정 계획

## Context

다음주 사내 배포 전 프로덕션 리뷰 수행. 타겟 HW: i3-12100 (4C/8T), 8~16GB RAM, C: 용량 작음, D: HDD에 자료 저장.
두 차례 코드 리뷰(내부 + 외부) 결과를 교차 검증하여 실제 영향도 기준으로 우선순위를 확정함.
종합 점수: 83/100 - 아키텍처 탄탄하고 HDD 최적화 우수. 아래 항목 수정 후 배포 가능.

---

## P0: 배포 전 필수

### P0-1. FTS 검색 OR→AND 변경 (검색 핵심 로직)

**문제**: 단어 추가 시 결과가 늘어남 (OR 동작). "고용보험료 부과"=309건 → "고용보험료 부과 고용"=327건↑
**방식**: 어절 AND + 형태소 OR (사용자 결정)

**수정 파일**:
- `src-tauri/src/search/fts.rs` (~line 106): `terms.join(" OR ")` 변경
- `src-tauri/src/tokenizer/lindera_ko.rs` (~line 159): `term_queries.join(" OR ")` 변경

**구현 방식**: "고용보험 부과" → `("고용"* OR "보험"*) AND "부과"*`
- 공백으로 분리된 어절 간에는 AND
- 같은 어절에서 형태소 분석된 토큰 간에는 OR
- lindera_ko.rs에서 어절별 그룹핑 로직 추가 필요

**테스트**: 기존 검색 결과가 줄어드는 방향인지 확인. 빈 결과 없는지도 체크.

---

### P0-2. IPC 타임아웃 래퍼

**문제**: Rust 백엔드 hang 시 프론트엔드 무한 대기
**수정 파일**: `src/hooks/useSearch.ts` 또는 공통 유틸
**구현**: `Promise.race([invoke(...), timeout(ms)])` 유틸 함수
- 검색: 30s, 파일 작업: 5s, 인덱싱 명령: 10s

---

### P0-3. 쿼리 길이 제한

**문제**: max query length 없음
**수정 파일**: `src-tauri/src/commands/search.rs` (4개 검색 커맨드 모두)
**구현**: `const MAX_QUERY_LEN: usize = 1000;` + 초과 시 에러 반환

---

### P0-4. 에러 UI 노출 보강

**문제**: useIndexStatus, useVectorIndexing 에러가 console.warn만 찍힘
**수정 파일**: 해당 hooks + `src/App.tsx` ErrorBanner 연결
**구현**: 에러 state를 UI에 노출, 스택 트레이스 sanitize

---

### P0-5. 검색결과 파일명 호버 시 최종수정일 표시

**문제**: 파일 수정일 정보가 DTO에 있지만 UI에 미표시
**기존 인프라** (이미 있음):
- `result.modified_at`: Unix timestamp (초), SearchResult DTO에 이미 포함
- `formatRelativeTime()`: `src/utils/formatRelativeTime.ts` (상대시간 포맷팅)
- `Tooltip` 컴포넌트: `src/components/ui/Tooltip.tsx`

**수정 파일**: `src/components/search/SearchResultItem.tsx`
**구현**: 파일명 영역을 `<Tooltip>` 으로 감싸고 `formatRelativeTime(result.modified_at)` 표시. 절대 날짜도 함께 표시 (예: "3일 전 (2026.02.18 14:30)")

---

## P1: 배포 1주 내

### P1-1. 시맨틱 결과 "왜 나왔는지" 시각적 설명

**문제**: 시맨틱 매칭 결과에 왜 매칭됐는지 시각적 설명 없음
**기존 인프라** (이미 있음):
- `enrich_semantic_results()` (`search_service.rs:497-578`): 이미 가장 유사한 문장을 추출해서 snippet에 `[[HL]]` 마커로 추가
- `match_type: "semantic" | "hybrid" | "keyword"`: 매치 타입 구분 이미 됨
- 문제는 시맨틱 하이라이트가 키워드 하이라이트와 **시각적으로 구분 안 됨**

**구현 방안**:
1. 시맨틱 결과(`match_type === "semantic"`)의 하이라이트를 다른 스타일로 표시 (예: 점선 밑줄 + 주황색 배경)
2. 하이라이트 위에 "의미 유사 문장" 라벨 추가
3. 하이브리드 결과는 키워드 하이라이트(노란 배경) + 시맨틱 문장 하이라이트(주황 점선) 이중 표시

**수정 파일**:
- `src/components/search/SearchResultItem.tsx`: match_type에 따른 하이라이트 스타일 분기
- `src/components/search/HighlightedText.tsx`: 시맨틱 전용 하이라이트 스타일 추가
- `src/index.css`: `--color-highlight-semantic-bg`, `--color-highlight-semantic-text` CSS 변수 추가

---

### P1-2. 시맨틱 OFF 시 모델 미로드

**문제**: `search_service()` 호출 시 `get_embedder().ok()`가 OnceCell 트리거 → 키워드 검색만 해도 ~300MB 모델 로드
**수정 파일**: `src-tauri/src/application/container.rs` (search_service, index_service 메서드)
**구현**: `semantic_search_enabled` 설정 확인 → false면 embedder/vector_index를 None으로 전달

---

### P1-3. 시작 시 sync 조건부 실행

**문제**: 매 시작 시 모든 완료 폴더를 full FS walk. HDD에서 10-30초 디스크 점유.
**수정 파일**: `src-tauri/src/lib.rs` (~line 342-443)
**구현 옵션**:
- 설정에 `sync_on_startup: bool` 추가 (기본 ON)
- 또는: notify watcher가 이미 돌고 있으므로, "앱이 정상 종료되었으면" 스킵

---

### P1-4. clear_all 후 VACUUM

**문제**: DELETE 후 DB 파일 크기 안 줄어듦. C: 용량 미복구.
**수정 파일**: `src-tauri/src/db/mod.rs` (clear_all_data 함수)
**구현**: COMMIT 후 `conn.execute("VACUUM", [])?;` 추가

---

### P1-5. 에러 메시지 sanitize

**문제**: Rust 에러 문자열이 그대로 프론트엔드에 노출될 수 있음
**수정 파일**: `src-tauri/src/error.rs` (ApiError → 사용자 친화적 메시지 변환)

---

## P2: 1개월 내

| # | 항목 | 파일 | 설명 |
|---|------|------|------|
| 1 | 이중 FS 순회 통합 | index.rs, pipeline.rs | 1차 스캔 파일리스트를 2차에 전달 |
| 2 | data_root 설정 | container.rs, settings.rs, lib.rs | 인덱스/DB 저장 경로 변경 기능 |
| 3 | PDF timeout 조정 | parsers/pdf.rs | 5s → 3s (i3-12100 기준) |
| 4 | 대량 결과 페이지네이션 | SearchResultList.tsx | 5000+ 결과 시 CPU 스파이크 방지 |
| 5 | Vector mapping atomic write | search/vector.rs | temp→rename 패턴 |
| 6 | 모달 포커스 트랩 | SettingsModal 등 | 접근성 개선 |

---

## 기각 항목

- ❌ chunk pagination/streaming: 메모리 영향 무시 수준 (500 chunks ≈ 1MB)
- ❌ 연결 풀링: Tauri 단일 프로세스에서 per-command 연결로 충분

---

## 아키텍처 강점 (유지)

- HDD 감지 → mmap_size=0: D드라이브 안정성 확보 탁월
- 데드락 완전 방지: 락 순서 고정 (index → id_map → key_map → next_key)
- 파서 panic 격리: catch_unwind + timeout thread + detached cleanup
- 2단계 인덱싱: FTS 먼저 → 벡터 백그라운드. 즉시 검색 가능
- 보안: SQL injection/path traversal 완전 방어, CSP 엄격

---

## 검증 방법

1. `cargo test --lib` (46 passed 확인)
2. `pnpm build` (프론트엔드 빌드 성공)
3. `cargo check` (Rust 컴파일 확인)
4. 수동 테스트:
   - FTS AND 동작: 단어 추가 시 결과 줄어드는지
   - 시맨틱 OFF 상태에서 메모리 사용량 확인 (Task Manager)
   - D: HDD 폴더 인덱싱 → 검색 → 결과 확인
   - 데이터 초기화 → C: 공간 복구 확인
   - 파일명 호버 → 수정일 표시 확인

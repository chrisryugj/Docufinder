# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1~5**: 완료
- **Phase 6**: 완료 (3대 기능 + 프로덕션 리뷰 + 리팩토링)
- **v2.1~v2.8**: 모두 커밋+푸시 완료
- **고도화 Phase A+B**: 완료 (이번 세션)

## 마지막 업데이트
2026-03-28 (고도화 Phase A+B — DB 안정성 + UI/UX 폴리시)

---

## 현재 상태 요약

**빌드 상태**: cargo check 통과 + tsc --noEmit 통과
**커밋 상태**: Phase A+B 미커밋 (커밋 예정)
**통합 테스트**: 미실행 (`pnpm tauri:dev` 실제 동작 확인 필요)

### ✅ Phase A: 다이어트 + 안정화 (이번 세션)

| # | 항목 | 변경 내용 |
|---|------|----------|
| 1 | 앱 종료 DB 최적화 | `cleanup_database()` 추가 — WAL checkpoint(TRUNCATE) + PRAGMA optimize |
| 2 | 북마크 고아 레코드 정리 | `get_bookmarks()` 호출 시 파일 미존재 북마크 자동 삭제 |
| 3 | once_cell 마이그레이션 취소 | `OnceLock::get_or_try_init` 아직 unstable → once_cell 유지 |

### ✅ Phase B: UI/UX 폴리시 (이번 세션)

| # | 항목 | 변경 내용 |
|---|------|----------|
| 1 | 필터 드롭다운 통일 | `CustomSelect` 컴포넌트 생성, SearchFilters + FilterDropdown 교체 |
| 2 | 결과 확장 애니메이션 | grid `0fr→1fr` 트랜지션 + duration-200 |
| 3 | 접근성: 신뢰도 점수 | `aria-label="신뢰도 X% (높음/보통/낮음)"` + title 추가 |
| 4 | 접근성: 파일 타입 배지 | Badge에 aria-label prop 추가, 파일 형식 라벨 전달 |
| 5 | 접근성: 패러다임 토글 | `role="radiogroup"` + `role="radio"` + `aria-checked` |
| 6 | 접근성: 확장 버튼 | `aria-expanded` + `role="button"` + aria-label |

**수정 파일:**
- `src-tauri/src/lib.rs` — cleanup_database() 추가
- `src-tauri/src/commands/preview.rs` — 북마크 고아 레코드 정리
- `src/components/ui/CustomSelect.tsx` — 새 컴포넌트
- `src/components/ui/Badge.tsx` — aria-label prop 추가
- `src/components/ui/FilterDropdown.tsx` — CustomSelect 사용
- `src/components/search/SearchFilters.tsx` — CustomSelect 사용
- `src/components/search/SearchResultItem.tsx` — 확장 애니메이션 + 접근성
- `src/components/search/SearchParadigmToggle.tsx` — 접근성 개선

### 🔍 보류 항목 (이전 세션부터)

- **H-2**: OCR 모델 SHA-256 해시 채우기
- **SEC-H2**: 하드코딩 admin code 9812
- **SEC-M4**: API 키 plaintext 저장

### 📋 다음 할 일

**다음 세션 (Phase C: 성능 고도화):**
1. DB 커넥션 풀 최적화 (현재도 6개 풀 있지만 retry_on_busy 개선)
2. 벡터 검색 스코프 프리필터 (전체 결과 후처리 → 폴더별 프리필터)
3. 증분 인덱싱 (mtime 기반 변경 파일만 재인덱싱)
4. 리랭커 배치 제한 (Top-100만 리랭킹)
5. Lindera 토크나이저 캐싱

**후속:**
- Phase D: 킬러 피처 (문서 미리보기 패널, 내보내기 고도화, 오타교정)
- `pnpm tauri:dev` 통합 테스트
- MSI 빌드 테스트

## 핵심 설계 결정

### CustomSelect 설계
- 네이티브 `<select>` 대체: 다크모드 옵션 목록 완전 스타일링
- 키보드 네비게이션: ArrowUp/Down, Enter, Escape, Home/End
- CSS grid `0fr→1fr` 트랜지션: line-clamp 대신 부드러운 확장

### once_cell 유지 결정
- `std::sync::OnceLock::get_or_try_init`이 Rust nightly에서만 사용 가능 (feature `once_cell_try`)
- stable Rust에서는 `once_cell` 크레이트 계속 사용

---

## 다음 세션 이어가기 프롬프트

```
Docufinder(Anything) 프로젝트 — 로컬 문서 검색 앱 (Tauri 2 + React).

고도화 Phase A+B 완료, Phase C 시작 예정.
- Phase A: DB 안정성 (WAL checkpoint on exit, 북마크 고아 정리)
- Phase B: UI/UX (커스텀 드롭다운, 확장 애니메이션, 접근성)

다음 할 일 (Phase C: 성능 고도화):
1. 벡터 검색 스코프 프리필터 — 현재 전체 결과 로드 후 후처리 → 폴더별 필터링 선적용
2. 증분 인덱싱 — mtime 기반 변경 파일만 재인덱싱 (현재 전체 재처리)
3. 리랭커 배치 제한 — 전체가 아닌 Top-100만 리랭킹
4. Lindera 토크나이저 캐싱 — 매 검색마다 재생성 → 싱글턴

컨텍스트: .claude/memory/activeContext.md
고도화 전략: .claude/plans/distributed-swimming-willow.md
```

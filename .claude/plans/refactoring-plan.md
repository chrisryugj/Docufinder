# DocuFinder 대대적 리팩토링 계획

**작성일**: 2026-01-17
**버전**: 1.0

---

## 개요

5가지 주요 개선사항을 3개 Sprint로 구현

| Sprint | 기간 | 주요 작업 |
|--------|------|-----------|
| 1 | 1주차 | 토스트 시스템 + 최근검색 개선 |
| 2 | 2주차 | 사이드바 UI/UX 강화 |
| 3 | 3-4주차 | 전체 PC 검색 기반 |

---

## Sprint 1: Quick Wins (8-12시간)

### 1.1 토스트 시스템 분리 및 파일 열기 피드백

**목표**: 파일 열기 시 "파일 여는 중..." → 성공/실패 피드백

**변경 파일**:
- `src/components/ui/Toast.tsx` (신규)
- `src/hooks/useToast.ts` (신규)
- `src/App.tsx` - handleOpenFile 수정
- `src/hooks/useExport.ts` - 내장 toast → useToast로 교체

**구현 단계**:
1. Toast 컴포넌트 생성 (success/error/loading/info 타입)
2. useToast 훅 생성 (showToast, updateToast, dismissToast)
3. ToastContainer로 다중 토스트 스택 지원
4. handleOpenFile에 적용:
   ```typescript
   const toastId = showToast("파일 여는 중...", "loading");
   try {
     await invoke("open_file", { path, page });
     updateToast(toastId, { message: "파일 열림", type: "success" });
   } catch {
     updateToast(toastId, { message: "파일 열기 실패", type: "error" });
   }
   ```

**난이도**: 하 | **소요**: 2-3시간

---

### 1.2 최근검색 시간 배지

**목표**: 검색시간 표시 ("2분 전", "어제") + 섹션 접기/펼치기

**변경 파일**:
- `src/types/search.ts` - RecentSearch 타입 추가
- `src/hooks/useLocalStorage.ts` - 타입 마이그레이션
- `src/components/sidebar/RecentSearches.tsx` - 시간 배지 UI
- `src/components/sidebar/Sidebar.tsx` - 접기/펼치기 상태
- `src/utils/formatRelativeTime.ts` (신규)

**타입 변경**:
```typescript
// 기존: string[]
// 변경: RecentSearch[]
interface RecentSearch {
  query: string;
  timestamp: number;
}
```

**마이그레이션 로직**:
```typescript
// useLocalStorage에서 자동 변환
const migrateSearches = (data: unknown): RecentSearch[] => {
  if (Array.isArray(data) && typeof data[0] === "string") {
    return data.map(q => ({ query: q, timestamp: Date.now() }));
  }
  return data as RecentSearch[];
};
```

**접기/펼치기**:
- Sidebar에 `isSearchesExpanded` 상태 추가
- Chevron 아이콘 회전 애니메이션
- localStorage에 상태 저장 (선택)

**난이도**: 하 | **소요**: 3-4시간

---

## Sprint 2: 사이드바 강화 (8-12시간)

### 2.1 폴더별 인덱싱 통계

**목표**: 각 폴더의 파일 수, 마지막 인덱싱 시간 표시

**변경 파일**:
- `src-tauri/src/db/mod.rs` - get_folder_stats 함수
- `src-tauri/src/commands/index.rs` - get_folder_stats 커맨드
- `src/types/index.ts` - FolderStats 타입
- `src/components/sidebar/FolderTree.tsx` - 통계 UI

**백엔드 쿼리**:
```sql
SELECT COUNT(*) as file_count, MAX(indexed_at) as last_indexed
FROM files WHERE path LIKE '{folder}/%'
```

**UI 표시**:
- 폴더명 옆에 작은 배지 (파일 수)
- 확장 시 마지막 인덱싱 시간

**난이도**: 중 | **소요**: 4-6시간

---

### 2.2 추가 사이드바 개선안

| 기능 | 설명 | 우선순위 |
|------|------|----------|
| 즐겨찾기 폴더 | 자주 검색하는 폴더 핀 고정 | P2 |
| 폴더 필터링 | 특정 폴더 범위 내 검색 | P2 |
| 검색 기록 그룹화 | 오늘/어제/이번 주 | P3 |
| 키보드 네비게이션 | Tab/Arrow 이동 | P3 |

---

### 2.3 하위폴더 검색 옵션 (이미 구현됨!)

**현재 상태**: `pipeline.rs`의 `collect_files_recursive()` 함수가 이미 재귀 탐색 구현

**추가 개선**:
- 설정에서 "하위폴더 포함" 토글 추가 (기본: true)
- `add_folder` 커맨드에 `recursive: bool` 파라미터

**난이도**: 하 | **소요**: 1-2시간

---

## Sprint 3: Everything 스타일 전체 PC 검색 (40-60시간)

### 3.1 기술 분석

**Everything의 핵심**: NTFS MFT 직접 읽기 (파일명만)
**DocuFinder의 차별점**: 파일 **내용** 검색 (파싱 필요)

### 3.2 권장 접근법: 드라이브 루트 인덱싱

**Phase 1: 기본 지원** (현재 가능)
- UI에서 드라이브 선택 허용 (C:\, D:\)
- 경고 메시지: "전체 드라이브 인덱싱은 시간이 오래 걸릴 수 있습니다"
- 인덱싱 진행률 표시

**Phase 2: 점진적 인덱싱**
- 초기엔 메타데이터만 빠르게
- 백그라운드에서 콘텐츠 파싱

**변경 파일**:
- `src-tauri/src/indexer/pipeline.rs` - 진행률 콜백
- `src-tauri/src/commands/index.rs` - 진행 상태 이벤트
- `src/hooks/useIndexStatus.ts` - 진행률 상태
- `src/components/layout/StatusBar.tsx` - 진행률 UI

### 3.3 성능 고려사항

| 항목 | 예상치 | 대응 |
|------|--------|------|
| C:\ 전체 스캔 | 10-30분 | 백그라운드 + 취소 버튼 |
| DB 크기 | 500MB-2GB | 청크 압축 |
| 메모리 | 200-500MB | 스트리밍 파싱 |

---

## 리스크 및 대응

| 리스크 | 확률 | 대응 |
|--------|------|------|
| localStorage 마이그레이션 실패 | 낮음 | 버전 키 + 자동 감지 |
| 시스템 폴더 접근 권한 | 높음 | 에러 핸들링 + 스킵 |
| 대용량 인덱싱 UX | 중간 | 진행률 필수 |

---

## 핵심 파일 목록

| 우선순위 | 파일 | 역할 |
|----------|------|------|
| ★★★ | `src/hooks/useLocalStorage.ts` | 최근검색 마이그레이션 |
| ★★★ | `src/App.tsx` | 토스트 통합, handleOpenFile |
| ★★☆ | `src/components/sidebar/RecentSearches.tsx` | 시간 배지 UI |
| ★★☆ | `src/components/sidebar/Sidebar.tsx` | 접기/펼치기 |
| ★☆☆ | `src-tauri/src/indexer/pipeline.rs` | 진행률 콜백 |

---

## 검증 방법

### Sprint 1 완료 기준
1. 파일 클릭 → "파일 여는 중..." 토스트 표시
2. 최근검색에 "2분 전" 형태 시간 배지 표시
3. 최근검색 섹션 접기/펼치기 동작

### Sprint 2 완료 기준
1. 폴더 확장 시 파일 수/마지막 인덱싱 시간 표시
2. 설정에서 하위폴더 포함 토글 동작

### Sprint 3 완료 기준
1. D:\ 드라이브 전체 인덱싱 가능
2. 인덱싱 진행률 실시간 표시
3. 인덱싱 취소 버튼 동작

---

## 실행 순서

```
Week 1
├── Day 1-2: Toast 시스템 분리 + 파일 열기 피드백
├── Day 3-4: 최근검색 시간 배지 + 마이그레이션
└── Day 5: 사이드바 접기/펼치기

Week 2
├── Day 1-3: 폴더별 통계 (백엔드 + 프론트)
└── Day 4-5: 하위폴더 옵션 + 테스트

Week 3-4
├── 인덱싱 진행률 시스템
├── 드라이브 루트 인덱싱 지원
└── 백그라운드 작업 취소
```

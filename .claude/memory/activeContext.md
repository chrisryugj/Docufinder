# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1**: 완료 (기반 구축)
- **Phase 2**: 완료 (파일 파서)
- **Phase 3**: 완료 (시맨틱 검색)
- **Phase 4**: 완료 (고급 기능)

## 완료된 작업

### Phase 1 (기반 구축)
- Tauri 프로젝트 셋업
- AppState (DB 경로 공유)
- DB CRUD 함수 (파일, 청크, 폴더)
- 인덱싱 파이프라인 (폴더 순회 → 파싱 → DB 저장)
- FTS5 검색 + 파일 정보 조인
- 프론트엔드 UI (폴더 선택, 검색, 결과 표시)

### Phase 2 (파일 파서)
| 파서 | 파일 | 라이브러리 |
|------|------|------------|
| HWPX | `parsers/hwpx.rs` | zip + quick-xml |
| DOCX | `parsers/docx.rs` | zip + quick-xml |
| XLSX | `parsers/xlsx.rs` | calamine |
| PDF | `parsers/pdf.rs` | pdf-extract |
| TXT/MD | `parsers/txt.rs` | 내장 |

### Phase 3 (시맨틱 검색)
| 컴포넌트 | 파일 | 라이브러리 |
|----------|------|------------|
| 임베더 | `embedder/mod.rs` | ort 2.0.0-rc.11 + tokenizers |
| 벡터 인덱스 | `search/vector.rs` | usearch 2.23 |
| 하이브리드 검색 | `search/hybrid.rs` | RRF 알고리즘 |

**주요 기능**:
- multilingual-e5-small 모델 (384차원, 118MB)
- 청크 임베딩 → usearch HNSW 인덱스
- 시맨틱 검색 (벡터 유사도)
- 하이브리드 검색 (FTS + 벡터 + RRF 병합)

### Phase 4 (고급 기능)
| 기능 | 파일 | 설명 |
|------|------|------|
| 파일 감시 | `indexer/manager.rs` | notify 기반, 디바운스 처리 |
| 증분 인덱싱 | `indexer/manager.rs` | 백그라운드 스레드에서 자동 처리 |
| 하이라이트 | `App.tsx` | highlight_ranges → mark 태그 |
| 검색 모드 | `App.tsx` | keyword/semantic/hybrid 선택 |
| 파일 열기 | `App.tsx` | shell:open 연동 (기존) |

## 다음 작업 (Phase 5: 배포)

| 작업 | 설명 | 우선순위 |
|------|------|----------|
| MSI 설치파일 | `pnpm tauri:build` → MSI 생성 | P1 |
| 자동 업데이트 | tauri-plugin-updater 설정 | P2 |
| 사용자 테스트 | 파일명 검색 등 전체 기능 검증 | P1 |

## 핵심 파일
| 파일 | 역할 |
|------|------|
| `src-tauri/src/lib.rs` | AppState (embedder, vector_index, watch_manager) |
| `src-tauri/src/embedder/mod.rs` | ONNX 임베딩 생성 |
| `src-tauri/src/search/vector.rs` | usearch 벡터 인덱스 |
| `src-tauri/src/search/hybrid.rs` | RRF 병합 |
| `src-tauri/src/indexer/pipeline.rs` | 인덱싱 (FTS + 벡터) |
| `src-tauri/src/indexer/manager.rs` | 파일 감시 + 증분 인덱싱 |
| `src-tauri/src/commands/search.rs` | 검색 커맨드 (keyword, semantic, hybrid) |

## 모델 다운로드
시맨틱 검색을 사용하려면 모델 파일이 필요합니다:
```
{앱 데이터 폴더}/models/multilingual-e5-small/
├── model.onnx       (118MB)
└── tokenizer.json   (17MB)
```

HuggingFace에서 다운로드:
- https://huggingface.co/intfloat/multilingual-e5-small

## 실행 방법
```bash
pnpm tauri:dev
```

## 최근 세션 작업 (2026-01-17)

**계획 문서**: `.claude/plans/refactoring-plan.md`

### ✅ Sprint 1 완료

| 작업 | 파일 | 상태 |
|------|------|------|
| 토스트 시스템 분리 | `src/components/ui/Toast.tsx`, `src/hooks/useToast.ts` | ✅ |
| 파일 열기 피드백 | `src/App.tsx:148-161` | ✅ |
| 최근검색 시간 배지 | `src/components/sidebar/RecentSearches.tsx` | ✅ |
| RecentSearch 타입 마이그레이션 | `src/hooks/useLocalStorage.ts`, `src/types/search.ts` | ✅ |
| formatRelativeTime 유틸 | `src/utils/formatRelativeTime.ts` | ✅ |
| 사이드바 접기/펼치기 | `src/components/sidebar/Sidebar.tsx` | ✅ |

### ✅ Sprint 2 완료

| 작업 | 파일 | 상태 |
|------|------|------|
| 폴더별 인덱싱 통계 | `db/mod.rs:372-398`, `FolderTree.tsx` | ✅ |
| 하위폴더 포함 토글 (설정) | `settings.rs:19-25`, `SettingsModal.tsx:228-258` | ✅ |
| 하위폴더 설정 실제 연동 | `pipeline.rs:42-173`, `index.rs:63-82` | ✅ |
| 즐겨찾기 폴더 (DB) | `db/mod.rs:118-160` | ✅ |
| 즐겨찾기 폴더 (백엔드) | `index.rs:221-260` | ✅ |
| 즐겨찾기 폴더 (프론트) | `FolderTree.tsx` (별 아이콘 + 상단 정렬) | ✅ |

### ✅ Sprint 3 완료

| 작업 | 파일 | 상태 |
|------|------|------|
| 인덱싱 진행률 시스템 | `pipeline.rs`, `index.rs`, `lib.rs` | ✅ |
| 진행률 UI + 취소 버튼 | `StatusBar.tsx`, `useIndexStatus.ts` | ✅ |
| 드라이브 루트 경고 | `useIndexStatus.ts` (ask dialog) | ✅ |
| 이벤트 타입 | `types/index.ts` (IndexingProgress) | ✅ |

### ✅ Sprint 4 완료 (파일명 검색)

| 작업 | 파일 | 상태 |
|------|------|------|
| files_fts FTS5 테이블 | `db/mod.rs` | ✅ |
| search_filename 커맨드 | `search/filename.rs`, `commands/search.rs` | ✅ |
| 파일명 모드 버튼 | `types/search.ts`, `SearchBar.tsx` | ✅ |
| 통합 모드 (파일명+내용) | `useSearch.ts`, `SearchResultList.tsx` | ✅ |
| "파일명만" 필터 | `SearchFilters.tsx` | ✅ |
| cleanPath 유틸 | `utils/cleanPath.ts` | ✅ |
| 설정 모드 연동 | `settings.rs`, `App.tsx`, `SettingsModal.tsx` | ✅ |

**기능 요약**:
- Everything 스타일 파일명 검색
- 4가지 검색 모드: 하이브리드/키워드/시맨틱/파일명
- 내용 검색 시 파일명 매치도 함께 표시 (상단)
- Windows Long Path prefix (`\\?\`) 자동 제거

### 이전 세션 작업
- **보기 밀도 설정**: 기본/컴팩트 모드 전환 기능
- **컴팩트 모드**: 세로 ~50% 축소 (line-clamp 2줄, 경로 숨김, 패딩/간격 축소)
- **검색바**: 그라디언트 제거, 미니멀 디자인으로 간소화
- **스크롤 맨 위로 FAB**: 300px 이상 스크롤 시 표시
- 다크모드 UI 전면 개선
- FTS5 한국어 키워드 검색 버그 수정
- RAG 스타일 신뢰도 표시 및 파일 그룹핑

## 핵심 파일 (Sprint 2 신규)
| 파일 | 역할 |
|------|------|
| `src-tauri/src/db/mod.rs:118-160` | 즐겨찾기 토글, 폴더 상세정보 조회 |
| `src-tauri/src/indexer/pipeline.rs:42-173` | recursive 옵션 지원 인덱싱 |
| `src-tauri/src/commands/settings.rs:101-113` | 설정 동기 조회 (get_settings_sync) |
| `src/components/sidebar/FolderTree.tsx` | 폴더 통계, 즐겨찾기 UI |
| `src/types/index.ts` | FolderStats, WatchedFolderInfo 타입 |

## 핵심 파일 (Sprint 3 신규)
| 파일 | 역할 |
|------|------|
| `src-tauri/src/indexer/pipeline.rs:128-288` | 진행률 콜백 + 취소 지원 인덱싱 |
| `src-tauri/src/commands/index.rs:41-162` | Tauri 이벤트 emit + cancel_indexing 커맨드 |
| `src-tauri/src/lib.rs:36-70` | indexing_cancel_flag (AtomicBool) |
| `src/hooks/useIndexStatus.ts` | 진행률 이벤트 리스너 + 드라이브 경고 |
| `src/components/layout/StatusBar.tsx` | 진행률 바 + 취소 버튼 UI |
| `src/types/index.ts:31-45` | IndexingProgress 타입 |

## 🔧 코드 리팩토링 (2026-01-18)

코드 리뷰 기반 Critical/High 이슈 수정 완료:

| Phase | 이슈 | 파일 | 상태 |
|-------|------|------|------|
| 1 | VectorIndex 스레드 안전성 | `search/vector.rs:35` - RwLock 적용 | ✅ |
| 2 | 파일 삭제 이벤트 미처리 | `indexer/manager.rs:148` - Remove 분기 | ✅ |
| 3 | foreign_keys PRAGMA | `db/mod.rs:19` + chunks 명시적 삭제 | ✅ |
| 4 | 설정값(max_results) 미반영 | `commands/search.rs:71,144,217,327` | ✅ |
| 5 | CSP + 폰트 로컬화 | `tauri.conf.json`, `index.html`, fonts/ | ✅ |
| 6 | 모델 부재 시 에러 | `lib.rs:109-115` | ✅ |

**계획 문서**: `.claude/plans/drifting-sauteeing-goblet.md`

### 핵심 변경사항

1. **VectorIndex 스레드 안전성**
   - `index: Index` → `index: RwLock<Index>`
   - `unsafe impl Send/Sync` 제거
   - 검색은 read lock, 추가/삭제/저장은 write lock

2. **데이터 무결성**
   - foreign_keys PRAGMA ON
   - 파일 삭제 시 chunks 명시적 삭제

3. **보안 강화**
   - CSP: `default-src 'self'; style-src 'self' 'unsafe-inline'...`
   - Pretendard 폰트 로컬 번들링 (~2MB)

## 🔍 결과내검색 기능 (2026-01-18)

| 기능 | 파일 | 설명 |
|------|------|------|
| 결과내검색 필터 | `useSearch.ts:178-186` | 2글자 이상 키워드만 필터링 |
| 필터 UI | `SearchFilters.tsx` | refineQuery 입력 + 클리어 버튼 |
| 필터바 유지 | `App.tsx:381` | 원본 결과 기준으로 필터바 표시 |
| IME 초기화 | `App.tsx:102-117` | 앱 시작 시 blur-focus 사이클 |
| CSS 수정 | `index.css:1-5` | @import 순서 수정 (HMR 오류 방지) |

**Known Issue**: 앱 최초 시작 시 IME 팝업창 뜸 (다른 창 갔다오면 해결)

## 🚀 시스템 트레이 + 자동 시작 (2026-01-18)

| 기능 | 파일 | 설명 |
|------|------|------|
| 시스템 트레이 | `lib.rs:273-311` | X버튼→트레이 최소화, 메뉴(열기/종료) |
| 자동 시작 | `lib.rs:208`, `settings.rs:149-158` | tauri-plugin-autostart |
| 시작 시 최소화 | `lib.rs:315-325` | --minimized 인자 또는 설정 |
| 재인덱싱 | `index.rs:348-463` | 폴더 우클릭 메뉴 |
| 컨텍스트 메뉴 | `FolderTree.tsx:90-133` | 우클릭 재인덱싱/폴더제거 |

**설정 옵션**:
- `auto_start`: Windows 시작 시 자동 실행
- `start_minimized`: 시작 시 트레이로 최소화

## 마지막 업데이트
2026-01-18 (시스템 트레이 + 자동 시작 + 재인덱싱)

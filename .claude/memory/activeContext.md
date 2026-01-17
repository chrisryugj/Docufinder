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

## 다음 작업 (Phase 5)
1. MSI 설치파일 생성
2. 자동 업데이트 설정
3. 사용자 가이드 문서

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

### 📋 다음 할 일

- [ ] 실제 테스트 (D:\ 드라이브 인덱싱)

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

## 마지막 업데이트
2026-01-18 (Sprint 3 완료)

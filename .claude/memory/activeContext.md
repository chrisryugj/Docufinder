# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1~5**: 완료
- **Phase 6**: 3대 기능 + 프로덕션 리뷰 수정 + 리팩토링 4건 완료 (빌드 통과, 실행 테스트 미완)

## 마지막 업데이트
2026-03-12 (프로덕션 리뷰 전체 수정 C5+H11+M19+L14 + 리팩토링 4건)

---

## 현재 상태 요약

**빌드 상태**: cargo check + pnpm build 모두 통과
**커밋 상태**: 미커밋 (30개+ 파일, 3대 기능 + 리뷰 수정 + 리팩토링)

### ✅ 프로덕션 리뷰 수정 (Critical 5 + High 11 + Medium 19 + Low 14 = 49건)

Phase 1 (Critical C1~C5), Phase 2 (High H1~H6), Phase 3 (Medium/Low) 모두 완료.

주요 수정 사항:
| 등급 | 수정 | 파일 |
|------|------|------|
| C | tokio::process 전환, 시스템 폴더 차단 등 | `index.rs, file.rs, constants.rs` |
| H | case-insensitive 경로, LIKE escape, SystemTime unwrap 등 | `filename_cache.rs, background_parser.rs, file_repository.rs` |
| M/L | IPC_TIMEOUT 타입, 검색 쿼리 검증, modal state 등 | `SettingsModal.tsx, CompactSearchBar.tsx` |

### ✅ 리팩토링 4건 (전부 완료)

| 항목 | 변경 | 핵심 파일 |
|------|------|-----------|
| validate_zip_archive 공통화 | docx/hwpx 중복 → mod.rs | `parsers/mod.rs, docx.rs, hwpx.rs` |
| lib.rs setup() 분리 | 400줄+ → 6개 헬퍼 함수 | `lib.rs` |
| SearchBar 중복 추출 | 공통 dropdown + hook | `SearchModeDropdown.tsx, useSearchInput.ts` |
| App.tsx 이벤트 분리 | 이벤트 리스너 → hook | `useAppEvents.ts` |

### 📋 다음 할 일

- [ ] **커밋** (30개+ 수정 파일)
- [ ] `pnpm tauri:dev`로 전체 기능 동작 확인
  - [ ] 폴더 추가 → 인덱싱 → 검색 기본 동작
  - [ ] 검색 범위 필터 (드롭다운 선택 → 결과 필터링)
  - [ ] CompactSearchBar scope 칩 표시/제거
  - [ ] IndexingReportModal 열기/닫기/재오픈 시 state 초기화
- [ ] HWP 변환 테스트 (한글 설치된 PC)
- [ ] MSI 빌드 → VC++ 미설치 VM에서 설치 테스트

### 남은 P2 작업 (3건)
- [ ] 이중 FS 순회 통합 (index.rs, pipeline.rs)
- [ ] data_root 설정 기능 (container.rs, settings.rs)
- [ ] PDF timeout 5s→3s (parsers/pdf.rs)

---

## 핵심 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/parsers/mod.rs` | 공통 ZIP 검증 + chunk_text (리팩토링됨) |
| `src-tauri/src/lib.rs` | setup() 6개 헬퍼로 분리됨 |
| `src/components/search/SearchModeDropdown.tsx` | 검색 모드 드롭다운 (신규) |
| `src/hooks/useSearchInput.ts` | 검색 입력 공통 로직 (신규) |
| `src/hooks/useAppEvents.ts` | App-level 이벤트 리스너 (신규) |
| `src/App.tsx` | 이벤트 분리로 경량화 |
| `src/components/search/SearchBar.tsx` | 217→99줄 경량화 |
| `src/components/search/CompactSearchBar.tsx` | 중복 제거 경량화 |

## 실행 방법
```bash
pnpm tauri:dev      # 개발 모드
pnpm tauri:build    # MSI 빌드
```

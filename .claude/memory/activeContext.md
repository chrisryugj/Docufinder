# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1~5**: 완료
- **Phase 6**: 완료 (3대 기능 + 프로덕션 리뷰 + 리팩토링)
- **Phase 7 리팩토링**: 완료 (Context 분리 + God File 해체 + framer-motion 제거)
- **고도화 Phase A~D**: 완료
- **다이어트 (Phase 1)**: 완료
- **kordoc 사이드카 통합 + 마크다운 미리보기**: 완료
- **인덱싱 시스템 전체 개선**: 완료 (2026-04-07)
- **프로덕션 100점 감사 + 34개 이슈 수정**: 완료 (2026-04-11)
- **kordoc + Node.js 번들링**: 완료 (2026-04-11)

## 마지막 업데이트
2026-04-11 (프로덕션 감사 34개 이슈 수정 + kordoc 번들링 구조 완성)

---

## 현재 상태 요약

**빌드 상태**: cargo check 0 warning, tsc 0 error
**커밋 상태**: 미커밋 (대규모 변경사항 34개 + kordoc 번들링)
**통합 테스트**: 미실행 (`pnpm tauri:dev` 필요)
**검증 상태**: verify-work 서브에이전트 25/25 PASS

---

### ✅ 이번 세션 완료 (프로덕션 100점 감사 + 수정)

#### P0 — 데이터 무결성 (4개)
| 수정 | 파일 | 효과 |
|------|------|------|
| 취소 감지 bool 필드 | `pipeline.rs`, `sync.rs`, `folder.rs` | 문자열 검색("Cancelled") → `was_cancelled: bool` |
| clear_all_data 완전 초기화 | `db/mod.rs:419` | bookmarks, file_tags 삭제 추가 |
| N+1 쿼리 제거 | `db/mod.rs:311` | 단일 JOIN 쿼리 + dead code 삭제 |
| MAX_FILE_SIZE 동기화 | `parsers/mod.rs:173` | 200MB→500MB (설정 최대값과 일치) |

#### P1 — UX 핵심 (6개)
| 수정 | 파일 | 효과 |
|------|------|------|
| PasswordProtected 에러 | `docx.rs`, `xlsx.rs`, `pdf.rs` | 암호 파일 감지 + 사용자 안내 |
| 한글 1~2자 LIKE 폴백 | `search/fts.rs:220` | FTS 빈 결과 시 LIKE 검색 |
| Explorer \\?\ strip | `commands/file.rs:91,293` | 파일 열기 무음 실패 방지 |
| HWP 변환 120초 타임아웃 | `commands/index/data.rs:160` | 모달 hang 방지 |
| EUC-KR charset 폴백 | `parsers/txt.rs` + Cargo.toml | encoding_rs로 관공서 문서 지원 |
| 진행률 바 | 이미 올바름 확인 | skip_indexed 후 total 계산 |

#### P2 — 접근성 (8개)
| 수정 | 파일 | 효과 |
|------|------|------|
| hover 전용 버튼 | `BookmarkList.tsx`, `RecentSearches.tsx` | group-focus-within 추가 |
| AI 스트리밍 aria-live | `AiAnswerPanel.tsx:196` | 스크린리더 공지 |
| 접근성 일관성 (6곳) | Header, PreviewPanel, SmartQueryInfo, VectorIndexingBanner, UpdateBanner, Input | aria-expanded, aria-label, aria-describedby, role="status" |
| TagInput 키보드 | `TagInput.tsx:54` | ArrowUp/Down/Enter/Escape |

#### P3 — 안정성 (4개)
| 수정 | 파일 | 효과 |
|------|------|------|
| 에러 타입 매핑 | `error.rs:118,134` | Domain→IndexingFailed, IO→IndexingFailed |
| raw 에러 정제 | `useFileTags.ts`, `useExport.ts` | 한국어 메시지로 대체 |
| 로깅 초기화 순서 | `lib.rs:392` | 콘솔 폴백 먼저 |
| ~$ 임시파일 필터 | 이미 구현 확인 | manager.rs:376 |

#### P4 — 완성도 (8개)
| 수정 | 파일 | 효과 |
|------|------|------|
| Button loadingText | `Button.tsx:10` | 커스텀 로딩 텍스트 |
| Toast 30초 타임아웃 | `useToast.ts:72` | loading 토스트 안전망 |
| useFavorites 삭제 | `useLocalStorage.ts` | 데드코드 제거 |
| XLSX MAX_TOTAL_CHARS | `xlsx.rs:44` | 5M 문자 제한 가드 |
| auto_vacuum | `migration.rs:36`, `lib.rs:285` | INCREMENTAL + cleanup |
| startup sync 최적화 | `init.rs:149` | 루프 밖 단일 DB 연결 |
| upsert RETURNING | `db/mod.rs:218,694,729` | 2쿼리→1쿼리 |
| ResultContextMenu | `ResultContextMenu.tsx` | 유사문서 찾기 메뉴 추가 |

#### KR — kordoc 번들링
| 수정 | 파일 | 효과 |
|------|------|------|
| 번들 스크립트 | `scripts/bundle-kordoc.ps1` | node.exe + kordoc dist + node_modules 준비 |
| 리소스 등록 | `tauri.conf.json:40-42` | node.exe, kordoc/* 번들 |
| 번들 node.exe 우선 탐색 | `kordoc.rs:184` | 시스템 Node.js 불필요 |
| 번들 cli.js 우선 탐색 | `kordoc.rs:74` | 리소스 디렉토리 우선 |
| .gitignore | `.gitignore:56-57` | 바이너리 제외 |

---

### 📋 잔여 이슈 (다음 세션)

#### 구조적 변경 필요 (High)
- [ ] `sync.rs: db_files HashMap` (~200MB) → DB-side 임시 테이블 diff로 추가 절감
- [ ] `collector.rs collect_files()` → channel-based 스트리밍

#### kordoc 번들링 마무리 (High)
- [ ] `bundle-kordoc.ps1` 실행 테스트 → 실제 번들 크기 확인
- [ ] kordoc `resources/kordoc/*` glob → 서브디렉토리(node_modules) 포함 여부 테스트
- [ ] `pnpm tauri:build` 프로덕션 빌드 + MSI 설치 테스트
- [ ] kordoc standalone 번들 검토 (tsup noExternal로 node_modules 인라인 → node.exe만 필요)

#### 기능 개선 (Medium)
- [ ] 벡터 인덱스 RAM 관리: 500만 청크 시 ~7.5GB
- [ ] `idle_detector` 통합
- [ ] startup sync 전체 드라이브 최적화

---

## 핵심 파일
| 파일 | 역할 |
|------|------|
| `src-tauri/src/parsers/kordoc.rs` | kordoc 사이드카 (번들 node.exe + cli.js 탐색) |
| `src-tauri/src/parsers/mod.rs` | 파서 라우팅 (kordoc 우선 → Rust 폴백) |
| `src-tauri/src/error.rs` | 에러 타입 매핑 (Domain→IndexingFailed) |
| `src-tauri/src/db/mod.rs` | DB 함수 (JOIN 쿼리, RETURNING id, clear_all) |
| `src-tauri/src/search/fts.rs` | FTS5 검색 (LIKE 폴백 추가) |
| `src-tauri/src/indexer/pipeline.rs` | 인덱싱 파이프라인 (was_cancelled 필드) |
| `scripts/bundle-kordoc.ps1` | kordoc 번들 준비 스크립트 |
| `src-tauri/tauri.conf.json` | Tauri 번들 리소스 설정 |

---

## 다음 세션 이어가기 프롬프트

```
Docufinder(Anything) 프로젝트 — 로컬 문서 검색 앱 (Tauri 2 + React).

이전 세션: 프로덕션 100점 감사 → 34개 이슈 수정 + kordoc 번들링 구조 완성.
빌드: cargo check 0 warn, tsc 0 err. verify-work 25/25 PASS. 미커밋.

주요 수정:
- was_cancelled bool 필드 (문자열 검색 제거)
- clear_all_data 완전 초기화 (bookmarks/file_tags 추가)
- N+1 쿼리 → 단일 JOIN, upsert RETURNING id
- PasswordProtected 에러 (DOCX CFB 감지, XLSX, PDF)
- 한글 1~2자 LIKE 폴백 검색
- EUC-KR charset 감지 (encoding_rs)
- Explorer \\?\ strip, HWP 변환 타임아웃
- 접근성 8개소 (aria-live, aria-expanded, focus-within, TagInput 키보드)
- kordoc + node.exe 번들링 (bundle-kordoc.ps1, tauri.conf.json)

다음 할 일:
1. 미커밋 변경사항 커밋
2. bundle-kordoc.ps1 실행 테스트 + 프로덕션 빌드 검증
3. kordoc standalone 번들 검토 (node_modules 인라인)
4. sync.rs DB 임시 테이블 diff 최적화

컨텍스트: .claude/memory/activeContext.md
```

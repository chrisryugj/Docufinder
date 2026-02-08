# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1**: 완료 (기반 구축)
- **Phase 2**: 완료 (파일 파서)
- **Phase 3**: 완료 (시맨틱 검색)
- **Phase 4**: 완료 (고급 기능)
- **Clean Architecture**: Phase 2 완료 (AppContainer 마이그레이션)
- **성능 최적화**: Phase 1~5 완료
- **Phase 5 배포**: ✅ **보안 강화 Phase 1~2 완료, 베타 배포 준비 완료**

## 마지막 업데이트
2026-02-08 (프로덕션 리뷰 최종 완료 - 잔여 4건 수정, Match Rate 100%, 배포 준비 완료)

---

## 🚀 Phase 5: 사내 배포 준비 (완료)

### ✅ Phase 1 완료 (보안 필수)
| 작업 | 파일 | 설명 |
|------|------|------|
| **업데이터 비활성화** | `tauri.conf.json`, `default.json`, `lib.rs` | 외부 통신 차단 |
| **SHA-256 무결성 검증** | `model_downloader.rs` | 모델/DLL 다운로드 시 해시 검증 |
| **타임아웃 추가** | `model_downloader.rs` | 30초 연결, 5분 읽기 |
| **압축 폭탄 방어** | `hwpx.rs`, `docx.rs`, `xlsx.rs` | uncompressed size, 엔트리 수, 압축비 제한 |

### ✅ Phase 2 완료 (안정성 강화)
| 작업 | 파일 | 설명 |
|------|------|------|
| **파싱 오류 알림** | `App.tsx:141-162` | 토스트로 실패 수 표시 |
| **FTS SSD 모드 강제** | `pipeline.rs:409-421` | HDD에서도 병렬 처리 |
| **PDF 타임아웃 5초** | `pdf.rs:9` | 10초 → 5초 |
| **경로 파싱 수정** | `disk_info.rs:26-36` | `\\?\` 접두사 처리 |

### ✅ Phase 3 완료 (안정성/보안 강화)
| 작업 | 파일 | 설명 |
|------|------|------|
| **DLL 해시 검증** | `model_downloader.rs:31-32, 276-285` | ONNX Runtime ZIP SHA-256 검증 |
| **DB 트랜잭션** | `db/mod.rs:291-336, 362-409, 412-443, 448-475` | delete_file/folder/clear_all 원자성 보장 |
| **크래시 핸들러** | `lib.rs:84-114` | panic hook + crash.log 저장 |
| **HWPX 하드 제한** | `hwpx.rs:326, 342` | Read::take(50MB) 압축 폭탄 완전 방어 |
| **lineSpacing 조건** | `hwpx.rs:671` | in_default_style 조건 추가 |
| **console.log 제거** | `vite.config.ts:27-29` | esbuild drop 설정 |

### ✅ Phase 4 완료 (안정성 최종 검증)
| 작업 | 파일 | 설명 |
|------|------|------|
| **VectorIndex LockPoisoned** | `search/vector.rs` | 25개 expect() → graceful error handling |
| **FolderTree 메모리 누수** | `FolderTree.tsx:59-85` | isMounted 플래그로 언마운트 후 setState 방지 |

### ✅ Phase 6 완료 (프로덕션 종합 리뷰 - 2026-02-08)

**프로덕션 점수: 65 → ~85/100** (잔여 4건 추가 수정 완료)

#### Phase A (Critical - 배포 차단) - 5건 완료
| 작업 | 파일 | 설명 |
|------|------|------|
| **panic=unwind 전환** | `Cargo.toml:89` | abort→unwind, catch_unwind 복원 |
| **strip=debuginfo** | `Cargo.toml:93` | 백트레이스 함수명 유지 |
| **.expect() 3곳 제거** | `lib.rs:564,445`, `pipeline.rs:529` | graceful crash handling |
| **set_var unsafe 래핑** | `lib.rs:117`, `container.rs:144` | 스레드 안전성 |
| **ErrorBoundary CSS 수정** | `ErrorBoundary.tsx:49-77` | CSS 변수 5개 정정 |
| **배포 문서화** | `DEPLOYMENT.md`, `tauri.conf.json` | 코드 서명 가이드 + WiX 서명 placeholder |

#### Phase B (Major) - 7건 완료
| 작업 | 파일 | 설명 |
|------|------|------|
| **PDF 스레드 카운터 복구** | `pdf.rs:52-68` | cleanup 스레드 추가 |
| **디스크 감지 캐싱** | `disk_info.rs` | OnceLock 캐시 |
| **Input CSS 변수화** | `Input.tsx` | 하드코딩 → CSS 변수 |
| **8개 컴포넌트 컬러 정리** | FileIcon, Tooltip, SettingsModal 등 | 하드코딩 → CSS 변수 (Tooltip arrow, SettingsModal InfoTooltip 포함) |
| **VectorIndex TOCTOU 수정** | `vector.rs:108-176` | 단일 write lock 통합 |
| **컨텍스트 메뉴 뷰포트** | SearchResultItem, Grouped | 경계 체크 추가 |
| **뮤텍스 패턴 정리** | `index.rs` | 불필요한 재잠금 제거 + clear_all_data() lock 통합 |

#### Phase C (Minor) - 8건 완료
| 작업 | 파일 | 설명 |
|------|------|------|
| **테마 플래시 방지** | `index.html` | 인라인 스크립트 |
| **크래시 로그 개선** | `lib.rs` | append 모드, create_dir_all |
| **TXT 사이즈 리밋** | `txt.rs:6` | 50MB 제한 |
| **VectorWorker Drop** | `vector_worker.rs:173-179` | 자동 정리 |
| **Dead deps 제거** | `package.json`, `Cargo.toml` | plugin-updater, plugin-fs |
| **devtools feature flag** | `Cargo.toml` | 릴리스 빌드 제외 |
| **MSI 타겟 지정** | `tauri.conf.json:30` | "all" → ["msi"] |
| **tokio features** | `Cargo.toml` | "full" → 선택적 features |

### 📋 배포 후 점진 개선
- [ ] C1: SearchBar/CompactSearchBar IME 중복 → 커스텀 훅
- [ ] C2: App.tsx(708줄), SettingsModal(706줄) 분리
- [ ] C3: React.memo 확대 (현재 3/25)
- [ ] C10: Settings 파일 캐싱
- [ ] C14: DB 스키마 버전 트래킹
- [ ] DB 암호화 검토 (SQLCipher) - 정책 요구 시
- [ ] CSP 강화 - 보안팀 요구 시

---

## 📊 인덱싱 테스트 결과 (1695개 파일)

| 항목 | 결과 |
|------|------|
| 성공 | 1558개 (92%) |
| 실패 | 137개 (8%) |
| 실패 원인 | ZIP 손상, PDF 타임아웃, 압축 폭탄 방어 |

---

## 핵심 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/lib.rs` | AppContainer 초기화 + 크래시 핸들러 |
| `src-tauri/src/indexer/pipeline.rs` | FTS 인덱싱 파이프라인 |
| `src-tauri/src/indexer/vector_worker.rs` | 벡터 인덱싱 워커 (Drop 구현) |
| `src-tauri/src/model_downloader.rs` | 모델 다운로드 + SHA-256 검증 |
| `src-tauri/src/utils/disk_info.rs` | SSD/HDD 감지 (OnceLock 캐싱) |
| `src-tauri/src/parsers/` | 문서 파서 (압축 폭탄 방어 + 사이즈 리밋) |
| `src-tauri/src/search/vector.rs` | 벡터 인덱스 (TOCTOU 수정, 단일 write lock) |
| `src/components/ErrorBoundary.tsx` | 에러 바운더리 (CSS 변수 수정) |
| `src/App.tsx` | 프론트엔드 앱 |
| `DEPLOYMENT.md` | 코드 서명 + 배포 가이드 |

## 실행 방법
```bash
pnpm tauri:dev      # 개발 모드
pnpm tauri:build    # MSI 빌드
```

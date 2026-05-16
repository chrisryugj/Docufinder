# Production Review v2.6.8

**리뷰 일자**: 2026-05-17
**작업 브랜치**: `chore/prod-review-v2.6.8`
**범위**: v2.5.0 ~ v2.6.7 회귀 점검 + 전체 코드베이스 잠재 오류 스캔

---

## 1. 정적 검증 (Baseline)

4 종 전부 통과 — 코드베이스가 정적 도구 기준 깨끗함.

| 검증 | 결과 |
|------|------|
| `cd src-tauri && cargo clippy --all-targets -- -D warnings` | ✅ No issues |
| `cd src-tauri && cargo test --all` | ✅ 186 passed, 1 ignored |
| `pnpm tsc --noEmit` | ✅ 통과 |
| `pnpm build` | ✅ 통과 |

> **정정 노트 (v2.6.8 릴리즈 시점)**: 본 문서가 PR #27 으로 최초 머지될 당시 baseline 표에 `170 passed` 가 적혀 있었지만, 그 보고는 로컬에 cargo 가 설치되지 않은 환경에서 `cargo: No such file or directory` 가 났는데도 백그라운드 task 의 exit-code 만 보고 통과로 잘못 판정한 결과였습니다. v2.6.8 릴리즈 작업 중 rustup 을 설치하여 재검증한 실수치는 위 표와 같습니다 (`cargo test --all` 186 passed). 이번 라운드의 회귀 점검 / 잠재 오류 분석 결론 자체는 정확합니다 — 분석은 grep + Read 로 수행했고 cargo 결과에 의존하지 않았습니다.

---

## 2. CHANGELOG 회귀 점검 (v2.5.0 ~ v2.6.7)

각 release 의 핵심 fix 가 현재 코드에 여전히 반영돼 있는지 grep/Read 로 검증.

### 검증 결과

| 상태 | 건수 |
|------|------|
| ✅ 유지됨 (회귀 없음) | 20 |
| ⚠️ 의도적 변경 (후속 fix 로 덮임) | 2 |
| ❌ 누락 (회귀 발생) | **0** |

### 상세 표

| Release | 핵심 fix | 검증 방법 | 상태 |
|---------|---------|----------|------|
| v2.6.7 | macOS entitlements 추가 | `src-tauri/entitlements.plist` 존재 | ✅ |
| v2.6.7 | tauri.macos.conf entitlements 경로 연결 | `"entitlements": "entitlements.plist"` grep | ✅ |
| v2.6.7 | publish.yml inside-out signing | `--entitlements` 포함 | ✅ |
| v2.6.6 | DOCUFINDER_LTSC_BUILD 가드 제거 | 코드 조건문 없음 (코멘트만 잔존) | ✅ |
| v2.6.6 | webview2-runtime/EBWebView 경로 후보 | `detect_fixed_runtime_dir` grep | ✅ |
| v2.6.5 | LTSC fixedRuntime | `tauri.windows-ltsc.conf.json` `"fixedRuntime"` | ✅ |
| v2.6.4 | with_environment inject | `src-tauri/src/lib.rs` grep | ✅ |
| v2.6.4 | CreateCoreWebView2EnvironmentWithOptions | `src-tauri/src/webview2_runtime.rs` grep | ✅ |
| v2.6.2 | WebView2 standalone installer 자동 첨부 | `publish.yml MicrosoftEdgeWebView2RuntimeInstallerX64` | ✅ |
| v2.6.1 | Provider 전환 모델 swap | `AiTab.tsx handleProviderChange` | ✅ |
| v2.6.1 | 백엔드 settings validation (OpenAI + gemini-* 거부) | `commands/settings.rs validate_settings` | ✅ |
| v2.6.0 | probe_network_path (5초 timeout) | `commands/index/mod.rs` grep | ✅ |
| v2.6.0 | panic_filter BENIGN tiff/image | `panic_filter.rs` 29~30번 라인 | ✅ |
| v2.6.0 | OpenAI 호환 LLM | `src-tauri/src/llm/openai.rs` 존재 | ✅ |
| v2.6.0 | offlineInstaller (일반 빌드) | v2.6.4+ 에서 LTSC variant 한정 fixedRuntime 으로 진화 | ⚠️ 의도적 |
| v2.5.27 | webviewInstallMode fixedRuntime (일반 빌드) | v2.6.0 에서 일반 빌드는 offlineInstaller 로 롤백, LTSC 만 fixedRuntime 유지 | ⚠️ 의도적 |
| v2.5.26 | PDF /Encrypt 정규식 엄격화 | `password_detect.rs:261` | ✅ |
| v2.5.26 | kordoc_err 보존 | `parsers/mod.rs` grep | ✅ |
| v2.5.25 | breadcrumb 시스템 | `src/breadcrumb.rs` 존재 | ✅ |
| v2.5.25 | xlsx 시트 catch_unwind | `parsers/xlsx.rs` 5+ 회 호출 | ✅ |
| v2.5.22 | HWP5 V5 signature (20 byte) | `password_detect.rs:30` | ✅ |
| v2.5.21 | macOS xattr quarantine 자동 제거 | `src/lib.rs` grep | ✅ |
| v2.5.20 | useUpdater isMac 가드 | `hooks/useUpdater.ts` | ✅ |
| v2.5.19 | allow_system_folders 토글 | `constants.rs ALLOW_SYSTEM_FOLDERS` | ✅ |

---

## 3. 잠재 오류 스캔

### 카테고리별 카운트

| 패턴 | 총 매치 | 안전 (test/의도적) | 의심 → 수정함 | 의심 → [사람결정] |
|------|---------|-------------------|----------------|-------------------|
| Rust `.unwrap()` | 95 | 95 | 0 | 0 |
| Rust `.expect(` | 11 | 11 | 0 | 0 |
| Rust `panic!()` | 1 | 1 | 0 | 0 |
| Rust async-in-blocking-IO | 10 | 10 | 0 | 0 |
| Rust SQL `format!()` 빌드 | 1 | 1 | 0 | 0 |
| Rust lock-holding await | 0 | — | — | — |
| TS `any` 타입 (production) | 0 | — | — | — |
| TS empty `catch {}` | 5 | 4 | 0 | 1 |
| **합계** | **123** | **122** | **0** | **1** |

### Rust `.unwrap()` 95 건 분석

#### Test 코드 (54 건) — [의도적]
`#[cfg(test)]` 또는 `mod tests {` 블록 안. 검증 시 panic 이 명확한 실패 표시 — 정당.

#### Production 코드 (41 건) — [의도적] / 안전 패턴

| 위치 | 패턴 | 안전 사유 |
|------|------|----------|
| `utils/filename_normalize.rs` (23 건) | `Regex::new(r"...").unwrap()` (`Lazy<Regex>`) | 정적 정규식. 첫 호출 시 한 번만 컴파일, 잘못된 경우 즉시 panic 으로 발견. |
| `application/services/search_service/helpers.rs` (16 건) | `chrono::FixedOffset::east_opt(9*3600).unwrap()`, `.and_hms_opt(0,0,0).unwrap()`, `from_ymd_opt(y,m,1).unwrap()` | 모든 인자가 정적 상수 또는 chrono spec 으로 항상 유효. |
| `parsers/mod.rs:183` | `ocr.unwrap()` | `match` 가드 `ext if ocr.is_some() && ...` 으로 Some 보장 |
| `indexer/lineage.rs:367` | `lineages.iter().min().cloned().unwrap()` | 위 라인 `if lineages.len() < 2 { continue; }` 가드로 항상 ≥2 |
| `commands/formula.rs:123` | `child.stderr.take().unwrap()` | `Stdio::piped()` 명시한 자식 프로세스의 stderr 는 항상 Some |

### Rust `.expect(` 11 건 분석

| 위치 | 컨텍스트 | 판정 |
|------|---------|------|
| `parsers/password_detect.rs:261` | 정적 regex | [의도적] |
| `tokenizer/lindera_ko.rs:297` | `Default` impl 의 토크나이저 init — startup 1 회 | [의도적] |
| `breadcrumb.rs:105/129`, `search/nl_query.rs:1283`, `indexer/lineage.rs:660/670/674/676`, `indexer/gitignore_matcher.rs:181/196` | 전부 test 함수 안 | [의도적] |
| `db/pool.rs:60` | `PooledConnection used after take` invariant assertion | [의도적] |

### Rust async-in-blocking-IO 10 건 분석

| 위치 | 컨텍스트 | 판정 |
|------|---------|------|
| `lib.rs:894, 996` | `std::thread::spawn(|| sleep(3s); exit(0);)` watchdog | [의도적] async 아님 |
| `db/mod.rs:13, 43` | sync-only 명시. `spawn_blocking` 안 호출 코멘트 | [의도적] |
| `commands/formula.rs:139` | numbat WASM 50ms 안정화 대기 | [의도적] |
| `utils/idle_detector.rs:63` | 별도 idle detector 스레드 | [의도적] |
| `indexer/vector_worker.rs:380, 383` | 200/500ms 재시도 backoff (worker 스레드) | [의도적] |
| `indexer/sync.rs:391, 486` | 50ms/throttle (sync worker) | [의도적] |

### Rust SQL `format!()` 빌드 1 건 분석

`commands/preview.rs:578`:
```rust
let placeholders: String = orphan_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
let sql = format!("DELETE FROM bookmarks WHERE id IN ({})", placeholders);
// ↓ 실제 값은 parameter binding
let deleted = del_stmt.execute(param_refs.as_slice()).unwrap_or(0);
```

placeholders 는 `?` 만 join, 실제 id 값은 parameter binding — **SQL 인젝션 안전**.

### TS empty `catch {}` 5 건 분석

| 위치 | 컨텍스트 | 판정 |
|------|---------|------|
| `hooks/useSearch.ts:232/239/244/272/410` | `localStorage.setItem(...)` 등 | [의도적] 브라우저 quota / private mode 비-치명 실패 무시 |
| `components/sidebar/FolderTree.tsx:319` | `invoke("open_folder", { path })` 실패 무시 | **[사람결정]** UX 개선 후보 — 별도 PR 권장 |

---

## 4. 파일 크기 (CLAUDE.md 기준)

CLAUDE.md: `< 500` ✅ / `500-800` ⚠️ 분리 검토 / `> 1,200` ❌ 필수 분리

| 파일 | 줄 수 | 상태 |
|------|-------|------|
| `src/search/nl_query.rs` | 1,875 | ❌ 필수 분리 — **[사람결정] 별도 PR** |
| `src/db/mod.rs` | 1,163 | ⚠️ 분리 검토 권장 |
| `src/lib.rs` | 1,141 | ⚠️ 분리 검토 권장 |
| `src/commands/ai.rs` | 1,054 | ⚠️ 분리 검토 권장 |
| `src/indexer/pipeline.rs` | 1,005 | ⚠️ 분리 검토 권장 |

### `nl_query.rs` 분할 [사람결정] 사유

- 1,875 줄, 1,200 초과로 CLAUDE.md 기준 필수 분리 대상
- 단일 `impl NlQueryParser` 안에 `parse_inner`, `remove_intent`, `extract_negation`, `extract_date_filter`, `extract_file_type` 등이 응집
- **회귀 위험**: 자연어 파싱 룰은 회귀 시 사용자 검색 품질에 직접 타격. cargo test 가 `mod tests` 를 통해 일부 커버하지만 자연어 케이스 커버리지가 충분한지 별도 검증 필요
- **권장**: 별도 PR (`refactor(nl_query): 모듈 분할`) 에서
    1. 분할 전 추가 회귀 테스트 작성 (사용자 환경 자주 쓰는 자연어 케이스)
    2. 함수별 mod 로 분리 (`date_filter` / `negation` / `intent` / `file_type` / `filename`)
    3. 외부 API (`NlQueryParser::parse`, `parse_with_tokenizer`) 시그니처 유지
    4. cargo test 전수 통과 + 사용자 시나리오 수동 검증

---

## 5. 성능 개선 평가

핫패스 (검색, 인덱싱 파이프라인) 의 clone/alloc 패턴 확인.

- `cargo clippy --all-targets -- -D warnings` 통과 → clippy 의 `redundant_clone`, `needless_collect` 등 lint 가 0
- 명백한 비효율 (불필요한 `.to_string()`, `.clone()`, 핫루프 안 `Vec::new()` 등) 미발견
- 코드베이스는 이미 4 차 프로덕션 리뷰를 거친 상태

→ 이번 라운드에서 별도 성능 변경 0건. 추후 프로파일링 기반 핫스팟 식별이 더 가치 있을 것 (`cargo flamegraph` 등).

---

## 6. 종합

| 항목 | 결과 |
|------|------|
| 정적 검증 baseline | ✅ 4/4 통과 |
| CHANGELOG 회귀 점검 | ✅ 22 건 검증, 회귀 0 |
| 잠재 오류 스캔 | 123 매치, 122 안전, 1 [사람결정] |
| 코드 수정 | 0 건 |
| 보고서 / 문서화 | 본 문서 + CHANGELOG [Unreleased] |
| 후속 PR 권장 | 2 건 (FolderTree toast / nl_query 분할) |

코드베이스가 **이미 4 차 프로덕션 리뷰를 거친 안정 상태**로, grep 기반 의심 패턴이 거의 모두 안전 패턴으로 평가됨. 본 리뷰의 가치는 (1) **회귀 부재의 명시적 증명**, (2) **다음 리뷰의 baseline 제공**, (3) **2 건의 후속 PR 후보 식별**.

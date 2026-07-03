# Docufinder 성능 프로덕션 리뷰 (6차) — 맥북프로 이관 핸드오프

작성: 2026-07-03 (맥미니). **맥북프로에서 이어가기 위한 컨텍스트.** T2-2부터는 임베딩 모델이
로컬에 있어야 검증되므로 모델 seed된 맥북에서 진행한다.

---

## 🔖 맥북에서 이어가기 프롬프트 (이거 붙여넣고 시작)

```
docufinder 성능 프로덕션 리뷰 6차 이어가기. HANDOFF-perf-review.md 읽고 시작.
맥미니에서 Phase 0(계측)·Phase 1(Tier1)·T2-1(검색 spawn_blocking)까지 커밋·푸시 완료.
지금 맥북(임베딩 모델 있음)이라 T2-2(FTS∥임베딩 rayon 병렬)부터 실검증하며 진행.
먼저 git pull 하고, HANDOFF의 "다음=T2-2" 설계대로 hybrid.rs 구현 → FTS-only 회귀 +
실검색 latency before/after 측정. 이후 T2-3~6, T1-6, T3, Phase4(릴리스 v3.0.9) 순.
```

---

## ✅ 완료 (커밋·푸시됨, main)

- **Phase 0 — 계측 하니스**: `src-tauri/src/perf_bench.rs` (`#[cfg(test)]`, 릴리스 미포함).
  토큰화 마이크로벤치 3종. lib.rs에 `#[cfg(test)] mod perf_bench;` 등록.
- **Phase 1 (Tier 1)** — 커밋 `58a4685`:
  - T1-1 토큰화 dedup O(n²)→HashSet (lindera_ko.rs). **대형 tokenize 1.71→1.14ms(-33%)**, 회귀 0.
  - T1-2 프론트 onToggleExpand 안정 콜백화 (SearchResultList/Item/Grouped). tsc 0. **스크롤 실테스트 미완**.
  - T1-3 usearch SAVE_INTERVAL 적응 max(1000,size/20) (vector_worker.rs).
  - T1-4 pool MAX_POOL_SIZE stale 주석 정정 (cache_size는 근거 확실해 유지).
  - T1-5 HWPX 문단 clone 제거 3곳 (text_extraction.rs).
  - T1-6 임베딩 배치화 → **Tier 2로 이월**(4곳 흩어짐).
- **T2-1 (검색 spawn_blocking)** — 커밋 `[이 핸드오프 커밋 직전 해시, git log 참조]`:
  - search_service/{hybrid,keyword,semantic,smart}.rs: **async fn 12개 sync 강등**(.await 제거, 진짜 async 0).
  - commands/search.rs: 검색 커맨드 6개 `spawn_blocking(move|| service.search_*).await??`.
  - commands/ai.rs: RAG search_hybrid 4곳 .await만 제거(sync 인라인, 래핑 보류 — 아래 주의).
  - 검증: cargo test --lib **222 passed 0 failed**, clippy -D warnings **0**.

## 🔧 검증 방법 (중요 — harness 우회)

harness가 `cargo test` 출력을 "N passed" 요약으로 정규화해서 `--nocapture` eprintln을 삼킨다.
**test 바이너리를 직접 실행**해야 perf 수치가 보인다:
```sh
source ~/.cargo/env && cd ~/workspace/docufinder/src-tauri
cargo test --release --lib --no-run 2>/dev/null
BIN=$(ls -t target/release/deps/docufinder_lib-* | grep -v '\.d$' | head -1)
"$BIN" perf_ --nocapture --test-threads=1          # perf 벤치
"$BIN" --skip perf                                  # 전체 회귀 (222 passed 기준)
```
- macOS라 `#[cfg(windows)]` 코드는 로컬 컴파일 제외(CI가 최종 검증).
- 통합/pool 테스트 race 주의 — 이미 단일 test 순차화됨.

---

## ▶️ 다음 = T2-2 (FTS∥임베딩 rayon 병렬) — 설계 완료, 맥북서 구현·검증

**파일**: `src-tauri/src/application/services/search_service/hybrid.rs` → `search_hybrid_impl` (119행~)
**의도**: FTS(conn만)와 벡터(emb.embed[ONNX,지배적]+vi.search)가 독립인데 순차 실행(총지연 = FTS+embed+vec).
`rayon::join`으로 병렬화하면 ≈ max(FTS, embed+vec) — FTS를 임베딩 뒤에 숨긴다.

**핵심 제약**: rusqlite `Connection`은 `!Sync` → rayon 두 클로저가 `&conn` 공유 불가.
각 클로저가 풀에서 conn을 **따로 획득**한다(MAX_POOL_SIZE=16이라 여유). `&self`는 Sync(Arc 필드).

**타입**: `emb.embed()` → `Vec<f32>`, `vi.search()` → `Vec<VectorResult>`(crate::search::vector::VectorResult).
rayon 1.10 의존성 이미 있음.

**설계 (126행 `let conn=...` 제거하고 아래로)**:
```rust
// FTS ∥ 벡터 병렬 (임베딩 ONNX가 지배적이라 FTS를 뒤에 숨김).
// Connection 은 !Sync 라 각 클로저가 풀에서 conn 을 따로 빌린다.
let (fts_res, vec_bundle) = rayon::join(
    || -> AppResult<Vec<fts::FtsResult>> {
        let conn = self.get_connection()?;
        match op { /* 135-163 기존 FTS 로직 그대로, ? 대신 map_err 반환 */ }
    },
    || -> AppResult<(Vec<crate::search::vector::VectorResult>, Option<Vec<f32>>)> {
        let vector_fetch_limit = if folder_scope.is_some() { max_results*3 } else { max_results };
        match (self.embedder.as_ref(), self.vector_index.as_ref()) {
            (Some(_),Some(_)) if display_query.trim().is_empty() => Ok((vec![], None)),
            (Some(emb),Some(vi)) => match emb.embed(&display_query, true) {
                Ok(qe) => {
                    let raw = vi.search(&qe, vector_fetch_limit).unwrap_or_default();
                    let results = if folder_scope.is_some() && !raw.is_empty() {
                        let conn = self.get_connection()?;  // path_map 전용 conn
                        /* 179-190 folder_scope 필터 그대로 */
                    } else { raw };
                    Ok((results, Some(qe)))
                }
                Err(e) => { tracing::warn!("embed 실패: {}", e); Ok((vec![], None)) }
            },
            _ => Ok((vec![], None)),
        }
    },
);
let fts_results = fts_res?;
let (vector_results, query_embedding) = vec_bundle?;
let conn = self.get_connection()?;   // 이후 RRF(231 vector_only)·enrich(351/352)용 재획득
// ── 204행 이후(RRF 병합, enrich) 기존 그대로 ──
```
**주의**: rayon::join은 커맨드의 spawn_blocking 스레드 안에서 rayon 전역풀 사용 → 인덱싱 rayon과
경합 가능하나 검색이 짧아 무해. conn 최대 3개 동시(FTS/벡터/메인) — 풀 여유.

**검증(맥북, 모델 有)**:
1. FTS-only 회귀: `"$BIN" --skip perf` 222 유지.
2. 실검색 latency before/after — 앱(`pnpm tauri:dev`)에서 벡터 인덱싱된 폴더 검색, search_time_ms 로그 비교.
3. 하이브리드 결과가 병렬화 전과 동일한지(순서/개수) 실검색으로 확인.

---

## 📋 남은 항목 (계획 원본: `.claude/plans/perf-review-2026-07-03.md` — gitignore라 맥미니 로컬만)

### Tier 2 (구조 개선)
- **T2-3 resume 메모리 스트리밍** — `indexer/pipeline.rs:305`, `db/mod.rs:253`.
  `get_all_fts_indexed_paths()`가 전 DB 경로를 Vec<String> 로드 → 폴더 스코프
  `path LIKE <folder>% AND fts_indexed_at IS NOT NULL` query_map 스트리밍 HashSet 으로. (1)만으로 피크 절반. **로컬 검증 가능**.
- **T2-4 임베딩 배치 파일경계 제거** — `indexer/vector_worker.rs:340`. 프리페치 파일단위→청크 스트림, 32 채워 embed_batch. 모델 필요.
- **T2-5 content 미적재** — `parsers/mod.rs:84`, `pipeline.rs:978`. 배치 경로서 doc.content 미적재(chunks만). total_chars는 chunks 길이합. **로컬 검증 가능**(파서).
- **T2-6 useDocumentCategories 배치 커밋** — `hooks/useDocumentCategories.ts:24`. Promise.all 단일 setCategories + 백엔드 배치 커맨드. **로컬(tsc) 검증 가능**.
- **T1-6 임베딩 배치화**(이월) — `indexer/duplicate.rs:290`, `indexer/lineage.rs:324,524`, `commands/lineage.rs:278`. 단건 embed→embed_batch. 온디맨드.

### Tier 3 (대공사)
- **T3-1 리스트 렌더** — ⚠️ **JS 가상화 폐기**(사용자 지시: 스크롤 버그). `content-visibility:auto`만,
  그것도 "더보기 수백개 스크롤 버벅임" 실측 후 결정. 스크롤 로직 절대 불변.
- **T3-3 Lindera 토큰화 병렬 프로듀서** — `pipeline.rs:706`. 단일 컨슈머 병목 → 병렬 파싱단계로.
  thread_local Segmenter 필요(&mut Mutex 직렬). 모델 무관, 로컬 가능.
- **T3-4 v17 부분인덱스** — `db/mod.rs:1121`. last_opened_at/size 인덱스(홈 "최근 문서" 풀스캔). additive 마이그레이션.
- **T3-5 파서 스레드 상한** — `utils/disk_info.rs:173`. SSD min(4)→min(8), RAM 조건부(저사양 회귀 주의).
- **T3-6 Ctrl+F DOM 하이라이트** — `PreviewPanel.tsx:755`. 찾기어마다 마크다운 재파싱 → TreeWalker DOM.
- **T3-2 kordoc persistent worker** — 별도 레포(chrisryugj/kordoc). 파일당 Node 콜드스타트. 큰 작업.

### Phase 4 — 검증 + 릴리스 v3.0.9
버전 3곳 bump(package.json, Cargo.toml, tauri.conf.json) + CHANGELOG + main push + `./scripts/release.sh 3.0.9`.
release.sh가 pnpm build + cargo fmt/check/clippy(-D)/test 전체 게이트.

### Tier 4 — 건드리지 말 것 (의도적 트레이드오프)
- 청크 본문 이중저장(chunks.content + chunks_fts): 형태소 재현율/미리보기용. 재색인 고위험.
- 릴리스 프로파일(Cargo.toml codegen-units=4, lto=thin): CI 빌드 메모리 위한 양보. 주석 근거.

## ⚠️ 미해결/주의
- **프론트 T1-2 스크롤 실테스트 미완**: `pnpm tauri:dev` → 검색결과 카드 확장/축소 눌러 스크롤 앵커
  정상인지. 스크롤 로직(handleToggle*)은 안 건드렸고 prop 배선만 바꿈. 이상하면 되돌림(58a4685 프론트 부분).
- **ai.rs RAG search_hybrid**: sync 인라인(워커 블로킹 잔존). LLM 대기 지배적이라 보류. 필요시 통째 spawn_blocking(return→Result 리팩터 필요).
- 임베딩 모델(KoSimCSE model_int8.onnx)은 첫 앱 실행 시 `seed_bundled_models`가 resources→models_dir 복사.
  맥북서 앱 한 번 띄우면 seed됨(그래야 벡터 경로 벤치/검증 가능).

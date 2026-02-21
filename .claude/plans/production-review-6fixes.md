# Anything (Docufinder) - Production Review + 6건 수정 계획

> **대상 환경**: i3-12100 (4C/8T), 8~16GB RAM, C: SSD (용량 제한), D: HDD (자료 저장)
> **배포 일정**: 다음 주 사내 공개
> **리뷰어**: 30년차 아키텍처 관점

---

## Part 1: 프로덕션 리뷰 결과 (18건)

### Executive Summary

전체적으로 **아키텍처 설계가 탄탄함**. Clean Architecture, 패닉 안전성, 락 순서 관리, zip bomb 방어, HDD 감지 + 적응형 스레딩 등 높은 수준의 엔지니어링.

**타겟 하드웨어(i3 + 8GB + HDD) 기준** 배포 전 수정 필요 이슈 존재.

**점수: 82/100** → P0/P1 해결 시 **90+** 가능

---

### P0 - 배포 전 필수 수정 (Blockers)

#### 1. SQLite 커넥션 풀링 없음 — 매 호출마다 새 연결
- **파일**: `src-tauri/src/db/mod.rs:14-44`
- **현상**: `get_connection()`이 매번 `Connection::open()` + PRAGMA 8개 실행
- **영향**: 검색할 때마다 HDD에서 10~30ms 오버헤드
- **수정**: `Arc<Mutex<Connection>>` 또는 `r2d2-sqlite` 풀 도입

#### 2. AppContainer가 `Mutex` — 읽기도 직렬화
- **파일**: `src-tauri/src/lib.rs:295` — `app.manage(Mutex::new(container))`
- **현상**: 검색(읽기)과 인덱싱(쓰기)이 같은 Mutex를 놓고 경쟁
- **영향**: 인덱싱 중 검색이 2~5초 블로킹
- **수정**: `Mutex` → `RwLock`으로 교체

#### 3. HDD에서 mmap_size 64MB는 역효과
- **파일**: `src-tauri/src/db/mod.rs:38`
- **현상**: HDD에서 mmap은 랜덤 I/O → 디스크 헤드 thrashing
- **영향**: 8GB RAM에서 ONNX 모델 + SQLite 캐시 + mmap 64MB 경합
- **수정**: HDD일 때 `mmap_size = 0` (disk_info.rs 활용)

#### 4. 미서명 MSI — SmartScreen 차단
- **파일**: `src-tauri/tauri.conf.json:42` — `certificateThumbprint: null`
- **영향**: GPO 적용 시 설치 불가 가능
- **수정**: 사내 코드서명 인증서 또는 IT팀 예외 요청

#### 5. 앱 데이터가 전부 C: 드라이브 고정
- **파일**: `src-tauri/src/application/container.rs:57-61`
- **현상**: DB, 벡터 인덱스, 모델 모두 `AppData\Roaming\`
- **영향**: 모델 ~260MB + DB/벡터 수백MB~GB가 C:에
- **수정**: 데이터 디렉토리 변경 옵션 추가

---

### P1 - 사용자 경험에 직접 영향 (High)

#### 6. Embedder Mutex가 검색과 인덱싱을 직렬화
- **파일**: `src-tauri/src/embedder/mod.rs:37` — `session: Mutex<Session>`
- **영향**: 128청크 배치 중 시맨틱 검색 2~5초 블로킹
- **수정안**: 검색/인덱싱용 Session 분리 또는 배치 사이 yield point

#### 7. 시맨틱 검색 시 실시간 재임베딩 (Enrichment)
- **파일**: `src-tauri/src/application/services/search_service.rs:19`
- **영향**: i3에서 500~1000ms 추가 지연
- **수정안**: enrichment optional화 또는 인덱싱 시 사전 계산

#### 8. 첫 실행 시 모델 다운로드가 동기 + UI 없음
- **파일**: `src-tauri/src/lib.rs:187`
- **영향**: 수십 초~수분 동안 앱이 안 뜨는 것처럼 보임
- **수정**: 비동기 + 스플래시/진행률 표시

#### 9. 로그 파일 무한 누적
- **파일**: `src-tauri/src/lib.rs:31-57`
- **영향**: 수개월 사용 시 C: 로그 누적
- **수정**: `max_log_files(7)` 또는 `max_log_files(30)`

#### 10. 그룹 뷰 가상화 미적용
- **파일**: `src/components/search/SearchResultList.tsx:344-360`
- **영향**: 그룹 뷰에서 DOM 과다 → i3에서 렌더링 지연
- **수정**: 그룹 뷰에도 가상 스크롤 적용

#### 11. React.memo 적용 부족 (3/25 컴포넌트)
- **대상**: SearchResultList, Sidebar, SearchBar, CompactSearchBar, Header 등
- **영향**: 검색 타이핑/인덱싱 이벤트마다 불필요한 리렌더
- **수정**: 핫 패스 컴포넌트에 React.memo 적용

---

### P2 - 안정성/유지보수 (Medium)

| # | 이슈 | 파일 |
|---|------|------|
| 12 | PDF 디태치 스레드 최대 160MB | `parsers/pdf.rs:9-16` |
| 13 | 벡터 HashMap 무한 성장 | `search/vector.rs:39-44` |
| 14 | HDD 스로틀 10ms 너무 짧음 | `utils/disk_info.rs` |
| 15 | 키보드 핸들러 매 렌더 재생성 | `App.tsx:277-314` |
| 16 | 벡터 저장 중 읽기 블로킹 | `vector_worker.rs:302-308` |
| 17 | PS ExecutionPolicy Bypass | `package.json:12` |
| 18 | 디버그 커맨드 프로덕션 노출 | `lib.rs:572` |

---

### 잘 된 부분 (Strengths)

| 영역 | 내용 |
|------|------|
| 패닉 안전성 | `panic = "unwind"` + `catch_unwind(AssertUnwindSafe)` |
| Zip bomb 방어 | 압축률, 엔트리 수, 비압축 크기 체크 |
| 락 순서 관리 | 벡터 인덱스 lock ordering 문서화 + 준수 |
| WAL + busy_timeout | 인덱싱/검색 동시성 올바른 설정 |
| HDD 감지 + 적응형 스레딩 | 디스크 타입별 자동 조정 |
| 벡터 워커 파이프라인 | Producer-Consumer + crossbeam + drop 데드락 방지 |
| 최소 권한 원칙 | Capabilities fs 없음, 커맨드 레벨 경로 검증 |
| 스트리밍 인덱싱 | Rayon 병렬 → 채널 → 배치 DB 쓰기로 OOM 방지 |
| 인텐시티 조절 | Fast/Balanced/Background + Windows 스레드 우선순위 |
| 크래시 로깅 | panic hook → crash.log |
| CSP 정책 | `script-src 'self'` |
| 모델 무결성 | SHA-256 검증 + 크기 제한 |

---

## Part 2: 즉시 수정 6건 실행 계획

### Context
추가 코드 리뷰에서 발견된 6개 이슈(HIGH 3, MEDIUM 2, LOW 1).
검색 품질 영구 저하, 메모리 급증, 상태 불일치 등 운영 안정성에 직결.

---

### 1. [HIGH-1] 벡터 인덱싱 부분실패 완료 마킹
**파일**: `src-tauri/src/indexer/vector_worker.rs`

**현재 코드 (line 311-316)**:
```rust
if !cancel_flag.load(Ordering::Relaxed) {
    if let Err(e) = db::mark_file_vector_indexed(&conn, prefetched.file_id) {
        tracing::warn!("[VectorWorker] Failed to mark file {}: {}", prefetched.file_id, e);
    }
}
```

**변경**:
- `Ok(prefetched)` 분기 시작에 `let mut file_failed_chunks: usize = 0;` 추가
- 임베딩 실패 시(line 274-277): `file_failed_chunks += batch.len();`
- 벡터 add 실패 시(line 282-284): `file_failed_chunks += 1;`
- 파일 완료 마킹(line 312): `file_failed_chunks == 0`일 때만 `mark_file_vector_indexed()`
- 실패 시 warn 로그에 파일명+실패 청크 수

**효과**: 실패 파일 pending 유지 → 다음 사이클 자동 재시도

---

### 2. [HIGH-2] 프리패치 버퍼 크기 축소
**파일**: `src-tauri/src/indexer/vector_worker.rs`

**변경**: `PREFETCH_BUFFER_SIZE` `4` → `2` (line 27)

**근거**: 병목은 임베딩, DB I/O 아님. 버퍼 2면 파이프라인 유지 + 메모리 = 3파일분

---

### 3. [HIGH-3] watcher 경로 파일 크기 제한 적용
**파일**: `src-tauri/src/indexer/manager.rs`, `src-tauri/src/application/container.rs`

**변경**:
- `IndexContext`에 `max_file_size_mb: u64` 필드 추가
- `container.rs:188` IndexContext 생성 시 settings에서 전달
- `process_pending_files():233-248` 파싱 전 크기 체크 → 초과 시 메타데이터만 저장

---

### 4. [MEDIUM-4] 파일명 캐시 truncated 시 DB fallback
**파일**: `src-tauri/src/search/filename_cache.rs`, `src-tauri/src/application/services/search_service.rs`

**변경**:
- `CacheData`에 `truncated: bool` 필드
- `load_from_db()` truncate 시 플래그 설정
- `is_truncated()` public 메서드
- `search_service.rs:140` 조건: `!c.is_empty() && !c.is_truncated()` → truncated면 DB LIKE 전환

---

### 5. [MEDIUM-5] 인덱싱 실패 시 상태 복구
**파일**: `src-tauri/src/commands/index.rs`

**변경**: `add_folder()`, `reindex_folder()`, `resume_indexing()` 3개 함수
```rust
// .map_err(ApiError::from)? 대신 match로
let result = match service.index_folder_fts(...).await {
    Ok(r) => r,
    Err(e) => {
        if let Ok(conn) = crate::db::get_connection(&db_path) {
            let _ = crate::db::set_folder_indexing_status(&conn, &path, "failed");
        }
        return Err(ApiError::from(e));
    }
};
```

---

### 6. [LOW-6] 테스트 기대값 수정
**파일**: `src-tauri/src/utils/disk_info.rs:171`
```rust
// `/`는 is_ascii_alphabetic() false → None이 맞음
assert_eq!(get_drive_letter(Path::new("/home/user")), None);
```

---

## 검증

```bash
cd "c:\github_project\Docufinder\src-tauri" && cargo check
cargo test
cargo clippy -- -W warnings
cd "c:\github_project\Docufinder" && pnpm build
```

수동 검증:
- 벡터 인덱싱 후 `SELECT COUNT(*) FROM files WHERE fts_indexed_at IS NOT NULL AND vector_indexed_at IS NULL`
- watcher 폴더에 대용량 파일 추가 시 스킵 로그
- 인덱싱 에러 후 폴더 상태 "failed" 복구 확인

---

## 수정 우선순위 로드맵

### 즉시 (이번 세션)
Part 2의 6건 수정 + 검증

### Week 1 (배포 전)
| # | 작업 | 난이도 | 효과 |
|---|------|--------|------|
| P0-1 | SQLite 커넥션 캐싱 | 중 | 검색 10~30ms 개선 |
| P0-2 | Mutex → RwLock | 하 | 인덱싱 중 검색 블로킹 해소 |
| P0-3 | HDD mmap_size=0 | 하 | HDD 랜덤 I/O 제거 |
| P0-4 | MSI 서명 / IT 예외 | 별도 | 설치 차단 방지 |
| P1-9 | 로그 보존 기간 | 하 | C: 누적 방지 |

### Week 2+ (안정화)
| # | 작업 | 난이도 | 효과 |
|---|------|--------|------|
| P0-5 | 데이터 디렉토리 설정 | 중 | C: 공간 해소 |
| P1-8 | 모델 다운로드 비동기 | 중 | 첫 실행 UX |
| P1-10 | 그룹 뷰 가상 스크롤 | 중 | 대량 결과 렌더링 |
| P1-11 | React.memo 확대 | 하 | UI 반응성 |

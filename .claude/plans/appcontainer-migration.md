# AppContainer 마이그레이션 계획

## 목표
- AppState → AppContainer 완전 대체
- DI 패턴으로 서비스 주입
- 테스트 용이성 향상

## 현재 상태

### AppState (lib.rs:45)
```rust
pub struct AppState {
    pub db_path: PathBuf,
    pub vector_index_path: PathBuf,
    pub models_dir: PathBuf,
    embedder: OnceCell<SharedEmbedder>,
    vector_index: OnceCell<Arc<VectorIndex>>,
    watch_manager: OnceCell<RwLock<WatchManager>>,
    indexing_cancel_flag: Arc<AtomicBool>,
    vector_worker: RwLock<VectorWorker>,
    tokenizer: OnceCell<SharedTokenizer>,
    reranker: OnceCell<SharedReranker>,
}
```
- **용도**: Tauri state로 등록, commands에서 직접 접근
- **상태**: 실제 사용 중

### AppContainer (application/container.rs:23)
```rust
pub struct AppContainer {
    // 동일한 필드들...
    // + Service factory 메서드
}
```
- **추가 기능**: search_service(), index_service(), folder_service()
- **상태**: dead_code

## 마이그레이션 단계

### Phase 1: AppContainer 완성 ✅
- [x] AppState의 모든 기능을 AppContainer로 통합
- [x] watch_manager 반환 타입 통일 (`Arc<RwLock<WatchManager>>`)
- [x] vector_worker 공유 문제 해결 (`Arc<RwLock<VectorWorker>>`)

### Phase 2: lib.rs 전환 ✅
- [x] `Mutex<AppState>` → `Mutex<AppContainer>` 교체
- [x] setup 함수에서 AppContainer 생성
- [x] on_window_event 핸들러 수정

### Phase 3: Commands 전환 ✅
- [x] commands/search.rs: `container.search_service()` 사용
- [x] commands/index.rs: AppContainer 타입 사용
- [x] commands/settings.rs: AppContainer 타입 사용

### Phase 4: 정리 ✅
- [x] AppState 삭제 (lib.rs에서 170줄+ 제거)
- [x] 미사용 코드 정리
- [x] 빌드 확인

## 주의사항

### 1. Tauri State 제약
```rust
// Tauri는 State<'_, T>로 접근
// T: Send + Sync 필요
// Mutex<AppContainer>로 래핑 필요
```

### 2. Service Factory 문제
```rust
// 현재 IndexService 생성:
index_service(&self) -> Result<IndexService, ApiError> {
    Ok(IndexService::new(
        self.db_path.clone(),
        self.get_embedder().ok(),
        self.get_vector_index().ok(),
        Arc::new(RwLock::new(VectorWorker::new())), // ❌ 새 인스턴스!
        self.indexing_cancel_flag.clone(),
    ))
}

// 해결: vector_worker를 Arc로 공유
vector_worker: Arc<RwLock<VectorWorker>>,
```

### 3. watch_manager 반환 타입
```rust
// AppState: &RwLock<WatchManager> 반환
// AppContainer: RwLock<WatchManager> 반환 (새로 생성)
// 통일 필요
```

## 예상 코드 변경

### commands/index.rs (예시)
```rust
// Before:
pub async fn get_index_status(state: State<'_, Mutex<AppState>>) -> ApiResult<IndexStatus> {
    let (db_path, semantic_available, vectors_count) = {
        let state = state.lock()?;
        // 직접 접근...
    };
    // 로직...
}

// After:
pub async fn get_index_status(container: State<'_, Mutex<AppContainer>>) -> ApiResult<IndexStatus> {
    let service = {
        let container = container.lock()?;
        container.index_service()?
    };
    service.get_status().await.map_err(ApiError::from)
}
```

## 진행 시 체크리스트

- [x] cargo check 통과
- [x] cargo build 통과
- [ ] pnpm tauri:dev 정상 동작 (수동 테스트 필요)
- [ ] 인덱싱 기능 테스트
- [ ] 검색 기능 테스트
- [ ] 벡터 인덱싱 테스트

## 완료 일시
- 2026-01-26
- 실제 소요 시간: ~30분

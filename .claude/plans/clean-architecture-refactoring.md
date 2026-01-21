# DocuFinder 배포 전 최종 점검 + 클린 아키텍처 도입

## 요약

**목표**: 전체 코드베이스에 클린 아키텍처 도입 + Critical 버그 수정
**기간**: 2-3주 (여유 있음)
**범위**: Rust 백엔드 + React 프론트엔드 전면 리팩토링

---

## 현재 상태 진단

### Rust 백엔드 (51% 클린 아키텍처 준수)
| 문제 | 위치 | 심각도 |
|------|------|--------|
| **VectorIndex 데드락** | `search/vector.rs:119-137` | 🔴 Critical |
| **WatchManager 종료 실패** | `indexer/manager.rs` | 🔴 Critical |
| **DIP 위반** | commands → db 직접 의존 | 🟠 High |
| **God Object** | AppState (5+ 책임) | 🟠 High |
| **Repository 패턴 부재** | db/mod.rs raw CRUD | 🟠 High |

### React 프론트엔드
| 문제 | 위치 | 심각도 |
|------|------|--------|
| **App.tsx 비대화** | 589줄 | 🟠 High |
| **Props Drilling** | 5단계 | 🟠 High |
| **Race Condition** | useSearch.ts | 🟠 High |
| **메모리 누수** | App.tsx 이벤트 리스너 | 🟠 High |
| **Error Boundary 없음** | 전체 앱 | 🟡 Medium |

---

## 아키텍처 설계

### Rust 백엔드 - 새 디렉토리 구조

```
src-tauri/src/
├── domain/                      # Domain Layer
│   ├── entities/                # File, Chunk, Folder
│   ├── value_objects/           # FileId, ChunkId, Embedding
│   ├── repositories/            # Repository Traits (추상화)
│   └── errors/                  # DomainError
│
├── application/                 # Application Layer
│   ├── dto/                     # SearchQuery, SearchResponse
│   ├── services/                # SearchService, IndexService
│   └── errors/                  # AppError
│
├── infrastructure/              # Infrastructure Layer
│   ├── persistence/
│   │   ├── sqlite/              # SqliteFileRepository 등
│   │   └── vector/              # UsearchVectorRepository
│   ├── ai/                      # OnnxEmbedder
│   ├── parsers/                 # 기존 유지
│   └── watchers/                # FileWatcher (종료 수정)
│
├── presentation/                # Presentation Layer
│   ├── commands/                # Tauri Commands → Service 의존
│   └── errors/                  # ApiError
│
└── di/
    └── container.rs             # AppContainer (DI)
```

### React 프론트엔드 - 새 디렉토리 구조

```
src/
├── app/                         # Application Core
│   ├── App.tsx                  # <150줄
│   ├── AppContent.tsx
│   ├── AppProviders.tsx
│   └── ErrorBoundary.tsx
│
├── features/                    # Feature Modules
│   ├── search/
│   │   ├── components/          # SearchBar, SearchResultList
│   │   ├── store/               # searchStore.ts (Zustand)
│   │   └── hooks/
│   ├── indexing/
│   ├── sidebar/
│   └── settings/
│
└── shared/                      # Shared Utilities
    ├── api/                     # Tauri IPC 추상화
    ├── components/ui/           # Button, Modal, Toast
    ├── hooks/
    └── types/
```

---

## 구현 계획

### Week 1: Rust 백엔드

#### Day 1-2: Domain Layer ✅ 완료
- [x] 디렉토리 구조 생성
- [x] Entities: `File`, `Chunk`, `WatchedFolder`
- [x] Value Objects: `FileId`, `ChunkId`, `Embedding`
- [x] Repository Traits: `FileRepository`, `ChunkRepository`, `VectorRepository`, `EmbedderPort`
- [x] `DomainError` 정의

#### Day 3-4: Infrastructure Layer ✅ 완료
- [x] `SqliteFileRepository` 구현
- [x] `SqliteChunkRepository` 구현
- [x] `UsearchVectorRepository` 구현 (**데드락 수정**: tokio::RwLock 사용)
- [x] `OnnxEmbedderAdapter` 구현
- [x] `FileWatcher` 종료 수정 (shutdown flag + join)

#### Day 5: Application Layer + DI
- [ ] DTOs: `SearchQuery`, `SearchResponse`, `IndexDTO`
- [ ] Services: `SearchService`, `IndexService`, `FolderService`
- [ ] `AppContainer` (DI 컨테이너)

#### Day 6-7: Presentation 전환 + 테스트
- [ ] Commands → Service 의존으로 변경
- [ ] `lib.rs`: AppState → AppContainer 교체
- [ ] 통합 테스트
- [ ] 레거시 코드 정리

### Week 2: React 프론트엔드

#### Day 1: Foundation
- [ ] `pnpm add zustand`
- [ ] API Layer: `client.ts`, `search.api.ts`, `indexing.api.ts`, `settings.api.ts`
- [ ] `ErrorBoundary.tsx`

#### Day 2-3: Stores
- [ ] `searchStore.ts` (**Race Condition 수정**: AbortController)
- [ ] `indexingStore.ts` (**메모리 누수 수정**: cleanup 함수)
- [ ] `settingsStore.ts`
- [ ] `uiStore.ts`

#### Day 4-5: Feature Migration
- [ ] Search 컴포넌트 이동 + Store 연결
- [ ] Sidebar 컴포넌트 이동 + Store 연결
- [ ] Settings 컴포넌트 이동 + Store 연결
- [ ] Shared UI 컴포넌트 이동

#### Day 6: App.tsx 재작성
- [ ] 새 `App.tsx` (120줄)
- [ ] `AppContent.tsx`, `AppProviders.tsx`
- [ ] 기존 App.tsx 삭제

#### Day 7: 정리 + 테스트
- [ ] 레거시 hooks 삭제
- [ ] E2E 테스트
- [ ] 번들 사이즈 확인

### Week 3: 인덱싱 성능 최적화 + 최종 검증

#### Day 1: Quick Wins (80-90% 개선)
- [ ] SQLite 캐시 증가 (`db/mod.rs`)
- [ ] 벡터 저장 빈도 감소 (`vector_worker.rs`)
- [ ] 배치 크기 동적 조정 (`vector_worker.rs`)
- [ ] ONNX 세션 최적화 (`embedder/mod.rs`)
- [ ] DB 배치 INSERT (`db/mod.rs`, `pipeline.rs`)

#### Day 2-3: 중기 최적화
- [ ] ZIP 병렬 파싱 (`hwpx.rs`, `docx.rs`)
- [ ] PDF 타임아웃 개선 (`pdf.rs`)

#### Day 4-5: 최종 검증
- [ ] 전체 기능 테스트 (`pnpm tauri:dev`)
- [ ] **인덱싱 성능 벤치마크** (1000파일 기준)
- [ ] 메모리 누수 테스트
- [ ] 코드 리뷰
- [ ] 문서화

---

## 핵심 수정 사항

### 1. VectorIndex 데드락 수정
```rust
// tokio::RwLock 사용 + 명시적 락 순서
let existing_key = {
    let id_map = self.id_map.read().await;
    id_map.get(&chunk_id).copied()
}; // 락 해제

if let Some(key) = existing_key {
    self.remove_internal(key).await?; // 별도 락
}
```

### 2. WatchManager 종료 수정
```rust
pub fn shutdown(&mut self) {
    self.shutdown_flag.store(true, Ordering::Relaxed);
    if let Some(handle) = self.worker_thread.take() {
        let _ = handle.join();
    }
}
```

### 3. Race Condition 수정 (React)
```typescript
let abortController: AbortController | null = null;

executeSearch: async () => {
  if (abortController) abortController.abort();
  abortController = new AbortController();
  // ...
}
```

### 4. App.tsx 축소 (589줄 → 120줄)
- Zustand stores로 상태 관리 이동
- 비즈니스 로직 → Store actions
- UI 조합만 담당

---

## 수정할 주요 파일

### Rust (생성 35개, 수정 7개, 삭제 2개)
| 액션 | 파일 |
|------|------|
| 생성 | `domain/**/*.rs` (13개) |
| 생성 | `application/**/*.rs` (10개) |
| 생성 | `infrastructure/**/*.rs` (10개) |
| 생성 | `di/container.rs` |
| 수정 | `lib.rs`, `commands/*.rs` |
| 삭제 | `db/mod.rs` (683줄), `search/vector.rs` (기존) |

### React (생성 47개, 이동 32개, 삭제 7개)
| 액션 | 파일 |
|------|------|
| 생성 | `shared/api/*.ts` (6개) |
| 생성 | `features/*/store/*.ts` (5개) |
| 생성 | `app/*.tsx` (4개) |
| 이동 | `components/**/*` → `features/*/components/` |
| 삭제 | 기존 `App.tsx`, 레거시 hooks |

---

---

## 인덱싱 성능 최적화 (추가)

### 현재 병목 분석

| 단계 | 소요 시간 비율 | 병목도 |
|------|---------------|--------|
| 벡터 임베딩 (ONNX) | **50%** | 🔴 최우선 |
| 파일 파싱 (ZIP+XML) | 20% | 🟠 중간 |
| FTS 인덱싱 (SQLite) | 15% | 🟠 중간 |
| 벡터 인덱스 저장 | 10% | 🟢 낮음 |
| 파일 I/O | 5% | 🟢 낮음 |

> **Everything과의 차이**: Everything은 NTFS MFT 직접 읽기 + 파일명만 인덱싱. DocuFinder는 파일 내용 파싱 + AI 임베딩이므로 근본적으로 다름.

### Quick Wins (1일, 예상 80-90% 개선)

#### 1. SQLite 캐시 증가 (1분, +5%)
```rust
// db/mod.rs:32
conn.pragma_update(None, "cache_size", -131072)?;  // 64MB → 128MB
```

#### 2. 벡터 저장 빈도 감소 (1시간, +8%)
```rust
// vector_worker.rs:21
const SAVE_INTERVAL: usize = 2000;  // 500 → 2000
```

#### 3. 배치 크기 동적 조정 (1-2시간, +25%)
```rust
// vector_worker.rs - 문서 길이에 따라 64~256 조정
let batch_size = match avg_chunk_len {
    0..=200 => 256,
    201..=400 => 128,
    _ => 64,
};
```

#### 4. ONNX 세션 최적화 (1시간, +12%)
```rust
// embedder/mod.rs:58-67
.with_inter_threads(2)?
.with_memory_pattern(true)?
```

#### 5. DB 배치 INSERT (2-3시간, +20%)
```rust
// db/mod.rs - 새 함수 추가
pub fn insert_chunks_batch(conn: &Connection, file_id: i64, chunks: &[...]) -> Result<Vec<i64>>
```

### 중기 개선 (1주일, 추가 +30%)

#### 6. ZIP 압축 해제 병렬화
- `hwpx.rs`, `docx.rs` - section별 병렬 파싱

#### 7. PDF 타임아웃 동적 조정
```rust
// pdf.rs - 파일 크기 기반 (1MB당 1초, 최소 3초)
fn calculate_timeout(file_size: u64) -> u64
```

### 장기 개선 (선택적, 5-10배)

#### 8. GPU 가속 (DirectML/CUDA)
```toml
# Cargo.toml
ort = { version = "2.0.0-rc.11", features = ["cuda", "tensorrt"] }
```
- **장점**: 5-10배 성능 향상
- **단점**: 런타임 의존성 추가, 배포 복잡도 증가

### 예상 효과

| 단계 | 개선률 | 소요 시간 |
|------|-------|----------|
| Quick Wins | **+80-90%** | 1일 |
| 중기 개선 | **+30%** (누적 2.1배) | 1주일 |
| GPU 가속 | **+500%** (누적 5-10배) | 1주일+ |

---

## 검증 방법

### 기능 테스트
```bash
pnpm tauri:dev
```
- [ ] 키워드 검색 동작
- [ ] 시맨틱 검색 동작
- [ ] 하이브리드 검색 동작
- [ ] 파일명 검색 동작
- [ ] 폴더 추가/삭제
- [ ] 실시간 인덱싱 진행률
- [ ] 벡터 인덱싱 (백그라운드)
- [ ] 설정 저장/불러오기
- [ ] 테마 변경

### 버그 수정 확인
- [ ] 대량 벡터 추가 시 데드락 없음
- [ ] 앱 종료 시 WatchManager 정상 종료
- [ ] 빠른 검색어 변경 시 결과 일관성
- [ ] 설정 모달 열기/닫기 반복 시 메모리 누수 없음

### 아키텍처 검증
- [ ] Commands에서 db/sqlite 직접 import 없음
- [ ] Repository Trait 통해서만 데이터 접근
- [ ] App.tsx 200줄 이하
- [ ] Props drilling 0단계

---

## 예상 효과

| 지표 | Before | After |
|------|--------|-------|
| 클린 아키텍처 준수 (Rust) | 51% | 90%+ |
| App.tsx 줄 수 | 589 | <150 |
| Props Drilling | 5단계 | 0단계 |
| Critical 버그 | 2건 | 0건 |
| Error Boundary | 없음 | 있음 |
| 테스트 가능성 | 낮음 | 높음 |

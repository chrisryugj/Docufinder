# Anything - 로컬 문서 검색 앱 설계 계획

> 폴더 내 HWPX/Office 문서를 인덱싱하고, 키워드 + AI 시맨틱 검색을 지원하는 데스크톱 앱

## 요구사항 요약

| 항목 | 결정 |
|------|------|
| 앱 형태 | Tauri 2 (Rust + React) |
| 검색 방식 | 키워드(FTS5) + 시맨틱(로컬 임베딩) + 하이브리드(RRF) |
| 지원 파일 | HWPX, DOCX, XLSX, PDF, TXT |
| 임베딩 | 로컬 모델 (KoSimCSE-roberta-multitask, 768차원) |
| 재정렬 | ms-marco-MiniLM-L6-v2 (Cross-Encoder) |
| 형태소 분석 | Lindera 2.0 (한국어 어절 AND + 형태소 OR) |
| 결과 표시 | 파일명+위치, 문맥 미리보기, 파일 열기, 수정일 표시 |
| 네트워크 | 완전 오프라인 지원 |
| 배포 | 회사 직원 PC (MSI 설치파일, 코드 서명) |

---

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                        │
├─────────────────────────────────────────────────────────────┤
│  Frontend (React 19 + TypeScript + Tailwind CSS 4)          │
│  ┌─────────────┬─────────────┬─────────────┐               │
│  │ SearchBar   │ ResultList  │ Settings    │               │
│  │ - 키워드입력│ - 페이지네이션 - 테마      │               │
│  │ - 검색모드  │ - 하이라이트│ - 파일열기  │               │
│  └─────────────┴─────────────┴─────────────┘               │
├─────────────────────────────────────────────────────────────┤
│  Backend (Rust - Clean Architecture)                        │
│  ┌─────────────┬─────────────┬─────────────┐               │
│  │ Commands    │ Application │ Domain      │               │
│  │ (Tauri IPC) │ (Services)  │ (Entities)  │               │
│  └─────────────┴─────────────┴─────────────┘               │
│  ┌─────────────┬─────────────┬─────────────┐               │
│  │ FileWatcher │ Parsers     │ SearchEngine│               │
│  │ - notify    │ - HWPX      │ - FTS5      │               │
│  │ - 변경감지  │ - DOCX/XLSX │ - Vector DB │               │
│  │ - 유휴감지  │ - PDF       │ - Reranker  │               │
│  └─────────────┴─────────────┴─────────────┘               │
│                        │                                    │
│  ┌─────────────────────┴─────────────────────┐             │
│  │ Storage: SQLite (FTS5 + 메타데이터)       │             │
│  │          + 벡터 인덱스 (usearch HNSW)     │             │
│  └───────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────┘
```

---

## 개발 단계

### Phase 1: 기반 구축 (MVP) ✅ 완료
- [x] Tauri 프로젝트 셋업
- [x] 기본 UI (검색창, 결과목록)
- [x] TXT 파일 파싱 + FTS5 검색
- [x] 폴더 선택 및 기본 인덱싱

### Phase 2: 파일 파서 확장 ✅ 완료
- [x] HWPX 파서 (zip + quick-xml)
- [x] DOCX 파서 (zip + quick-xml)
- [x] XLSX 파서 (calamine)
- [x] PDF 파서 (pdf-extract)

### Phase 3: 시맨틱 검색 ✅ 완료
- [x] ONNX 런타임 통합 (ort 2.0.0-rc.11)
- [x] KoSimCSE-roberta-multitask 임베딩 모델 (768차원)
- [x] 벡터 인덱스 (usearch 2.23)
- [x] 하이브리드 검색 (RRF)
- [x] 결과 재정렬 (ms-marco-MiniLM-L6-v2)
- [x] 한국어 형태소 분석 (Lindera 2.0)

### Phase 4: 고급 기능 ✅ 완료
- [x] 파일 변경 감지 (notify + WatchManager)
- [x] 증분 인덱싱 (백그라운드 자동 처리)
- [x] 문맥 미리보기 + 하이라이트
- [x] 파일 열기 (기본 앱 연동)
- [x] 검색 모드 선택 UI (keyword/semantic/hybrid/filename)
- [x] 2단계 인덱싱 (FTS 즉시 → 벡터 백그라운드)
- [x] SSD/HDD 자동 감지 + 적응형 스레딩
- [x] 유휴 감지 (백그라운드 파싱)
- [x] 파일명 인메모리 캐시 (Everything 스타일)
- [x] 즐겨찾기 폴더
- [x] 인덱싱 진행률 + 취소

### Phase 5: 배포 준비 ✅ 완료
- [x] 보안 강화 (압축 폭탄 방어, SHA-256 모델 검증, CSP)
- [x] 크래시 핸들러 (panic hook + crash.log)
- [x] DB 트랜잭션 원자성 보장
- [x] VectorIndex lock safety (TOCTOU 수정, LockPoisoned 처리)
- [x] 프로덕션 리뷰 4차 (88/100)
- [x] MSI 설치파일 + 코드 서명
- [x] CI/CD (GitHub Actions)
- [x] 결과 페이지네이션 + 설정 연동

### 남은 작업 (P2)
- [ ] 이중 FS 순회 통합 (index.rs, pipeline.rs)
- [ ] data_root 설정 기능 (container.rs, settings.rs)
- [ ] PDF timeout 5s→3s (parsers/pdf.rs)

---

## 핵심 컴포넌트

### 파일 파서 (Rust)

| 형식 | 라이브러리 | 추출 대상 | 보안 |
|------|------------|-----------|------|
| HWPX | `zip` + `quick-xml` | 본문, 표, 메타데이터 | 압축 폭탄 방어, Read::take(50MB) |
| DOCX | `zip` + `quick-xml` | 본문, 표 | 압축 폭탄 방어 |
| XLSX | `calamine` | 셀 데이터 | 100MB 제한 |
| PDF | `pdf-extract` | 텍스트 | 5초 타임아웃, 스레드 격리 |
| TXT | 내장 | 전체 텍스트 | 50MB 제한 |

### 검색 엔진

**키워드 검색 (FTS5)**:
- 형태소 분석: 어절 AND + 형태소 OR
- 예: "고용보험 부과" → `("고용" OR "보험") AND "부과"`

**시맨틱 검색**:
- 임베딩 모델: KoSimCSE-roberta-multitask (ONNX, 768차원)
- 벡터 인덱스: usearch (Rust 네이티브, HNSW)
- 재정렬: ms-marco-MiniLM-L6-v2 (Cross-Encoder)
- 청크 크기: 512자, 오버랩 64자

**파일명 검색**:
- 인메모리 캐시 (FilenameCache) → ~5ms
- 20만+ 파일 시 DB LIKE 폴백

---

## 프로젝트 구조

```
Anything/
├── src-tauri/              # Rust 백엔드 (Clean Architecture)
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs          # AppContainer 초기화
│   │   ├── commands/       # Tauri 커맨드
│   │   ├── application/    # 응용 계층 (services, dto, container)
│   │   ├── domain/         # 도메인 계층 (entities, repositories)
│   │   ├── infrastructure/ # 인프라 계층 (persistence, vector)
│   │   ├── parsers/        # 파일 파서
│   │   ├── search/         # 검색 엔진
│   │   ├── indexer/        # 인덱싱 (pipeline, vector_worker, background_parser)
│   │   ├── embedder/       # ONNX 임베딩
│   │   ├── tokenizer/      # 한국어 형태소 분석
│   │   ├── reranker/       # 결과 재정렬
│   │   ├── db/             # SQLite 스키마
│   │   └── utils/          # disk_info, idle_detector
│   └── Cargo.toml
├── src/                    # React 프론트엔드
│   ├── components/         # UI 컴포넌트 (33개)
│   ├── hooks/              # 커스텀 훅 (14개)
│   ├── types/              # 타입 정의
│   ├── utils/              # 유틸리티
│   └── App.tsx
├── models/                 # 임베딩 모델 (빌드 시 번들)
├── .github/workflows/      # CI/CD
├── PLAN.md                 # 이 파일
└── CLAUDE.md               # 개발 지침
```

---

## 참고 자료

- [Tauri 공식 문서](https://tauri.app)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [usearch](https://github.com/unum-cloud/usearch)
- [KoSimCSE](https://huggingface.co/BM-K/KoSimCSE-roberta-multitask)
- [ms-marco-MiniLM-L6-v2](https://huggingface.co/cross-encoder/ms-marco-MiniLM-L-6-v2)

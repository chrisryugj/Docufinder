# Anything - 로컬 문서 검색 앱 설계 계획

> 폴더 내 HWPX/Office 문서를 인덱싱하고, 키워드 + AI 시맨틱 검색을 지원하는 데스크톱 앱

## 📋 요구사항 요약

| 항목 | 결정 |
|------|------|
| 앱 형태 | Tauri (Rust + React) |
| 검색 방식 | 키워드(FTS5) + 시맨틱(로컬 임베딩) |
| 지원 파일 | HWPX, DOCX, XLSX, PDF, TXT, MD |
| 임베딩 | 로컬 모델 (multilingual-e5-small, 118MB) |
| 결과 표시 | 파일명+위치, 문맥 미리보기, 파일 열기 |
| 네트워크 | 완전 오프라인 지원 |
| 배포 | 회사 직원 PC (MSI 설치파일) |

---

## 🏗️ 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                        │
├─────────────────────────────────────────────────────────────┤
│  Frontend (React + TypeScript + Tailwind)                   │
│  ┌─────────────┬─────────────┬─────────────┐               │
│  │ SearchBar   │ ResultList  │ PreviewPane │               │
│  │ - 키워드입력│ - 파일목록  │ - 문맥표시  │               │
│  │ - 검색모드  │ - 하이라이트│ - 파일열기  │               │
│  └─────────────┴─────────────┴─────────────┘               │
├─────────────────────────────────────────────────────────────┤
│  Backend (Rust)                                             │
│  ┌─────────────┬─────────────┬─────────────┐               │
│  │ FileWatcher │ Parsers     │ SearchEngine│               │
│  │ - notify    │ - HWPX      │ - FTS5      │               │
│  │ - 변경감지  │ - DOCX/XLSX │ - Vector DB │               │
│  │             │ - PDF       │ - Embedder  │               │
│  └─────────────┴─────────────┴─────────────┘               │
│                        │                                    │
│  ┌─────────────────────┴─────────────────────┐             │
│  │ Storage: SQLite (FTS5 + 메타데이터)       │             │
│  │          + 벡터 인덱스 (hnswlib/usearch)  │             │
│  └───────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 개발 단계

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
- [x] e5-small 임베딩 모델 (multilingual-e5-small, 384차원)
- [x] 벡터 인덱스 (usearch 2.23)
- [x] 하이브리드 검색 (RRF)

### Phase 4: 고급 기능 ✅ 완료
- [x] 파일 변경 감지 (notify + WatchManager)
- [x] 증분 인덱싱 (백그라운드 자동 처리)
- [x] 문맥 미리보기 + 하이라이트
- [x] 파일 열기 (기본 앱 연동)
- [x] 검색 모드 선택 UI (keyword/semantic/hybrid)

### Phase 5: 배포
- [ ] MSI 설치파일 생성
- [ ] 자동 업데이트 설정
- [ ] 사용자 가이드 문서

---

## 📦 핵심 컴포넌트

### 파일 파서 (Rust)

| 형식 | 라이브러리 | 추출 대상 |
|------|------------|-----------|
| HWPX | `zip` + `quick-xml` | 본문, 표, 메타데이터 |
| DOCX | `docx-rs` | 본문, 표 |
| XLSX | `calamine` | 셀 데이터 |
| PDF | `pdf-extract` + `poppler` 폴백 | 텍스트 |
| TXT/MD | 내장 | 전체 텍스트 |

### 검색 엔진

**키워드 검색 (FTS5)**:
```sql
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    content,
    tokenize='unicode61'
);
```

**시맨틱 검색**:
- 임베딩 모델: `intfloat/multilingual-e5-small` (ONNX)
- 벡터 인덱스: `usearch` (Rust 네이티브, HNSW)
- 청크 크기: 512자, 오버랩 64자

---

## 📁 프로젝트 구조

```
Anything/
├── src-tauri/              # Rust 백엔드
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands/       # Tauri 커맨드
│   │   ├── parsers/        # 파일 파서
│   │   ├── search/         # 검색 엔진
│   │   ├── indexer/        # 인덱싱
│   │   └── db/             # 데이터베이스
│   └── Cargo.toml
├── src/                    # React 프론트엔드
│   ├── components/
│   ├── hooks/
│   ├── store/
│   └── App.tsx
├── models/                 # 임베딩 모델 (Phase 3)
├── PLAN.md                 # 이 파일
└── CLAUDE.md               # 개발 지침
```

---

## ⚠️ 리스크 및 대응

| 리스크 | 대응 |
|--------|------|
| PDF 한글 추출 실패 | poppler 바인딩 + OCR 폴백 |
| 임베딩 속도 느림 | 배치 처리, 백그라운드, 증분 |
| HWPX 복잡한 구조 | Auto_maeri 파서 참고 |

---

## 🔗 참고 자료

- [Tauri 공식 문서](https://tauri.app)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [usearch](https://github.com/unum-cloud/usearch)
- [multilingual-e5-small](https://huggingface.co/intfloat/multilingual-e5-small)
- Auto_maeri HWPX 파서: `packages/core/src/parser/`

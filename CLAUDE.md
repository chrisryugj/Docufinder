# Anything 프로젝트

로컬 문서 검색 앱 - HWPX, Office, PDF 지원 (Tauri + React)

## Quick Start

```bash
# 프론트엔드 개발 서버
pnpm dev

# Tauri 개발 모드 (Rust 빌드 포함)
pnpm tauri:dev

# 프로덕션 빌드
pnpm tauri:build
```

## 프로젝트 구조

```
Anything/
├── src-tauri/              # Rust 백엔드 (Clean Architecture)
│   ├── src/
│   │   ├── main.rs         # 앱 진입점
│   │   ├── lib.rs          # AppContainer 초기화 + 크래시 핸들러
│   │   ├── error.rs        # 글로벌 에러 타입
│   │   ├── constants.rs    # 상수 정의
│   │   ├── model_downloader.rs  # ONNX 모델 다운로드 + SHA-256 검증
│   │   ├── commands/       # Tauri IPC 커맨드 (search, index, settings, file)
│   │   ├── application/    # 응용 계층 (container, services, dto)
│   │   ├── domain/         # 도메인 계층 (entities, repositories, value_objects)
│   │   ├── infrastructure/ # 인프라 계층 (persistence, embedding, vector)
│   │   ├── parsers/        # 문서 파서 (hwpx, docx, xlsx, pdf, txt)
│   │   ├── search/         # 검색 엔진 (fts, vector, hybrid, filename, filename_cache)
│   │   ├── indexer/        # 인덱싱 (pipeline, manager, vector_worker, background_parser)
│   │   ├── embedder/       # ONNX 임베딩 (KoSimCSE, 768차원)
│   │   ├── tokenizer/      # 한국어 형태소 분석 (Lindera)
│   │   ├── reranker/       # 시맨틱 결과 재정렬
│   │   ├── db/             # SQLite + FTS5 스키마
│   │   └── utils/          # disk_info, idle_detector
│   └── Cargo.toml
├── src/                    # React 프론트엔드
│   ├── App.tsx
│   ├── ErrorBoundary.tsx
│   ├── components/
│   │   ├── ui/             # Button, Modal, Toast, Badge, Tooltip, FileIcon 등
│   │   ├── layout/         # Header, StatusBar, ErrorBanner
│   │   ├── sidebar/        # Sidebar, FolderTree, RecentSearches
│   │   ├── search/         # SearchBar, SearchFilters, SearchResultList, ResultContextMenu 등
│   │   ├── settings/       # SettingsModal, ColorPresetPicker
│   │   ├── onboarding/     # OnboardingModal, DisclaimerModal
│   │   └── help/           # HelpModal
│   ├── hooks/              # useSearch, useIndexStatus, useToast, useVectorIndexing 등 (14개)
│   ├── types/              # api, error, search, settings 타입 정의
│   ├── utils/              # cleanPath, formatRelativeTime, invokeWithTimeout
│   └── index.css           # 테마 CSS 변수 (라이트/다크)
├── .github/workflows/      # CI (TypeScript 빌드 + Rust check/test/clippy)
└── .claude/memory/         # Memory Bank (컨텍스트 유지)
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Frontend | React 19 + TypeScript 5.9 + Tailwind CSS 4 |
| Backend | Rust 2021 + Tauri 2.10 |
| 검색 | SQLite FTS5 (키워드) + usearch (벡터) + RRF 하이브리드 |
| 임베딩 | ort 2.0 (ONNX) + KoSimCSE-roberta-multitask (768차원) |
| 형태소 분석 | Lindera 2.0 (한국어) |
| 재정렬 | ms-marco-MiniLM-L6-v2 (Cross-Encoder) |
| 파싱 | zip, quick-xml, calamine, pdf-extract |
| 파일 감시 | notify 8 (증분 인덱싱) |

## 주요 명령어

| 명령어 | 설명 |
|--------|------|
| `pnpm dev` | Vite 개발 서버 |
| `pnpm tauri:dev` | Tauri 개발 모드 |
| `pnpm tauri:build` | MSI 설치파일 생성 |
| `pnpm run download-model` | ONNX 모델 다운로드 |

## 개발 Phase

| Phase | 내용 | 상태 |
|-------|------|------|
| 1 | 기반 구축 (Tauri 셋업, TXT 검색) | ✅ 완료 |
| 2 | 파일 파서 (HWPX, DOCX, XLSX, PDF) | ✅ 완료 |
| 3 | 시맨틱 검색 (ONNX, KoSimCSE) | ✅ 완료 |
| 4 | 고급 기능 (실시간 감시, 증분 인덱싱) | ✅ 완료 |
| 5 | 배포 준비 (보안 강화, 프로덕션 리뷰 4차) | ✅ 완료 |

## 주요 기능

| 기능 | 설명 |
|------|------|
| 하이브리드 검색 | 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 + 재정렬 |
| 파일명 검색 | Everything 스타일 파일명 검색 (인메모리 캐시) |
| 실시간 감시 | 폴더 변경 자동 감지 + 증분 인덱싱 |
| 2단계 인덱싱 | FTS 즉시 완료 → 벡터 백그라운드 처리 |
| 인덱싱 진행률 | 실시간 진행률 + 취소 버튼 |
| 즐겨찾기 폴더 | 자주 사용 폴더 핀 고정 |
| HDD 최적화 | SSD/HDD 자동 감지 + 적응형 스레딩 |
| 보안 | 압축 폭탄 방어, SHA-256 모델 검증, CSP |
| 문서 요약 | TextRank 추출적 요약 (TF-IDF 바이그램, 오프라인) |
| 법령 참조 링크 | 정규식 기반 법령 자동 감지 → law.go.kr 링크 |

## 참고 문서

| 문서 | 설명 |
|------|------|
| [PLAN.md](PLAN.md) | 상세 설계 계획 |
| [BUILD_GUIDE.md](BUILD_GUIDE.md) | 빌드 가이드 |
| [DEPLOYMENT.md](DEPLOYMENT.md) | 배포 가이드 |
| [DESIGN.md](DESIGN.md) | 디자인 시스템 |

## 코딩 컨벤션

- TypeScript strict mode
- Rust 2021 edition
- 함수형 React 컴포넌트
- Tailwind CSS 유틸리티
- Clean Architecture (Rust 백엔드)

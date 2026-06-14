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
├── src-tauri/              # Rust 백엔드 (commands → application/services → search·indexer·db)
│   ├── src/
│   │   ├── main.rs         # 앱 진입점
│   │   ├── lib.rs          # setup/초기화 + 크래시 핸들러 + invoke_handler 등록
│   │   ├── error.rs        # 글로벌 에러 타입 (ApiError)
│   │   ├── constants.rs    # 상수 정의
│   │   ├── model_downloader.rs  # ONNX 모델 다운로드 + SHA-256 검증
│   │   ├── panic_filter.rs # BENIGN_PANIC_SOURCES 크래시 필터
│   │   ├── webview2_runtime.rs  # WebView2 fixed-runtime inject (이슈 #24)
│   │   ├── breadcrumb.rs   # 크래시 진단 브레드크럼
│   │   ├── commands/       # Tauri IPC 커맨드 (search, index, settings, file, ai, lineage 등)
│   │   ├── application/    # 응용 계층 (container, services/search_service, dto)
│   │   ├── parsers/        # 문서 파서 (hwpx, docx, xlsx, pdf, txt, kordoc 사이드카)
│   │   ├── search/         # 검색 엔진 (fts, vector, hybrid, filename_cache, nl_query, query_syntax)
│   │   ├── indexer/        # 인덱싱 (pipeline, manager, vector_worker, collector, sync, lineage)
│   │   ├── embedder/       # ONNX 임베딩 (KoSimCSE, 768차원)
│   │   ├── tokenizer/      # 한국어 형태소 분석 (Lindera)
│   │   ├── ocr/            # PaddleOCR ONNX (이미지/스캔 PDF)
│   │   ├── llm/            # Gemini API (RAG 질의응답)
│   │   ├── db/             # SQLite + FTS5 스키마 (mod, migration, pool)
│   │   └── utils/          # disk_info, network_path, elevation, text_normalize 등
│   └── Cargo.toml
├── src/                    # React 프론트엔드
│   ├── App.tsx
│   ├── ErrorBoundary.tsx
│   ├── components/
│   │   ├── ui/             # Button, Modal, Toast, Badge, Tooltip, FileIcon 등
│   │   ├── layout/         # Header, StatusBar, AutoIndexPrompt, AppModals
│   │   ├── sidebar/        # Sidebar, FolderTree, SmartFolders, RecentSearches
│   │   ├── search/         # SearchBar, SearchResultList, PreviewPanel, CommandPalette 등
│   │   ├── settings/       # SettingsModal + tabs/
│   │   ├── onboarding/     # tourSteps, DisclaimerModal (투어 기반)
│   │   ├── updater/        # 업데이트 알림
│   │   └── help/           # HelpModal
│   ├── hooks/              # useSearch, useIndexStatus, useSmartFolders 등 (~30개)
│   ├── contexts/           # SearchContext, UIContext
│   ├── types/              # api, error, search, settings 타입 정의
│   ├── utils/              # searchTextUtils, cleanPath, invokeWithTimeout 등
│   └── styles/             # variables(테마 토큰)/components/animations/utilities
└── .github/workflows/      # CI (TypeScript 빌드 + Rust check/test/clippy)
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Frontend | React 19 + TypeScript 5.9 + Tailwind CSS 4 |
| Backend | Rust 2021 + Tauri 2.10 |
| 검색 | SQLite FTS5 (키워드) + usearch (벡터) + RRF 하이브리드 |
| 임베딩 | ort 2.0 (ONNX) + KoSimCSE-roberta-multitask INT8 (768차원) |
| 형태소 분석 | Lindera 2.0 (한국어) |
| OCR | PaddleOCR ONNX (det + rec-ko) |
| AI 질의응답 | Gemini API (RAG) |
| 파싱 | zip, quick-xml, calamine, pdf-extract, kordoc (HWP) |
| 파일 감시 | notify 8 (증분 인덱싱) |

## 주요 명령어

| 명령어 | 설명 |
|--------|------|
| `pnpm dev` | Vite 개발 서버 |
| `pnpm tauri:dev` | Tauri 개발 모드 |
| `pnpm tauri:build` | MSI 설치파일 생성 |
| `pnpm run download-model` | ONNX 모델 다운로드 |
| `./scripts/release.sh X.Y.Z` | 릴리즈 — 로컬 CI 검증 통과 시에만 태그 생성+푸시 |

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
| 하이브리드 검색 | 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 |
| 검색 연산자 (v3.0) | `"구문"` `-제외` `ext:` `path:` `before:/after:` — search/query_syntax.rs |
| 근접 검색 (v3.0) | `~N` 형태소 인지 FTS5 NEAR (Lindera 명사 추출 + prefix) |
| 파일명 검색 | Everything 스타일 파일명 검색 (인메모리 캐시) |
| 실시간 감시 | 폴더 변경 자동 감지 + 증분 인덱싱 (mtime+size 무변경 스킵) |
| 2단계 인덱싱 | FTS 즉시 완료 → 벡터 백그라운드 처리 (배치 저장) |
| 인덱싱 진행률 | 실시간 진행률 + 취소 버튼 |
| 즐겨찾기 폴더 | 자주 사용 폴더 핀 고정 |
| HDD 최적화 | SSD/HDD 자동 감지 + 적응형 스레딩 |
| 보안 | 압축 폭탄 방어, SHA-256 모델 검증, CSP |
| 문서 요약 | TextRank 추출적 요약 (TF-IDF 바이그램, 오프라인) |
| 법령 참조 링크 | 정규식 기반 법령 자동 감지 → law.go.kr 링크 |
| 미리보기 찾기 (v3.0) | Ctrl+F 인앱 찾기 바 — 매치 카운트 + 이전/다음 |
| 문서 비교 (v3.0) | 우클릭 '비교 대상으로 선택' → 임의 두 문서 청크 diff |

## 참고 문서

| 문서 | 설명 |
|------|------|
| [README.md](README.md) | 프로젝트 소개 |
| [CHANGELOG.md](CHANGELOG.md) | 변경 이력 |
| [DESIGN.md](DESIGN.md) | 디자인 시스템 |

## 코딩 컨벤션

- TypeScript strict mode
- Rust 2021 edition
- 함수형 React 컴포넌트
- Tailwind CSS 유틸리티
- Clean Architecture (Rust 백엔드)

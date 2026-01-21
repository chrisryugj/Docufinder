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
├── src-tauri/              # Rust 백엔드
│   ├── src/
│   │   ├── main.rs         # 앱 진입점
│   │   ├── lib.rs          # AppState (embedder, vector_index, watch_manager)
│   │   ├── commands/       # search, index, settings 커맨드
│   │   ├── parsers/        # hwpx, docx, xlsx, pdf, txt 파서
│   │   ├── search/         # FTS5, vector (usearch), hybrid (RRF), filename
│   │   ├── indexer/        # pipeline (진행률), manager (파일감시)
│   │   ├── embedder/       # ONNX 임베딩 (e5-small)
│   │   └── db/             # SQLite + FTS5 스키마
│   └── Cargo.toml
├── src/                    # React 프론트엔드
│   ├── components/
│   │   ├── ui/             # Button, Modal, Toast, Badge 등
│   │   ├── layout/         # Header, StatusBar, ErrorBanner
│   │   ├── sidebar/        # FolderTree, RecentSearches
│   │   ├── search/         # SearchBar, SearchFilters, SearchResultList
│   │   └── settings/       # SettingsModal
│   ├── hooks/              # useSearch, useIndexStatus, useToast 등
│   ├── types/              # 타입 정의
│   └── App.tsx
└── .claude/memory/         # Memory Bank (컨텍스트 유지)
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Frontend | React 19 + TypeScript + Tailwind CSS |
| Backend | Rust + Tauri 2 |
| 검색 | SQLite FTS5 (키워드) + usearch (벡터) + RRF 하이브리드 |
| 임베딩 | ort 2.0 (ONNX) + multilingual-e5-small (384차원) |
| 파싱 | zip, quick-xml, calamine, pdf-extract |
| 파일 감시 | notify (증분 인덱싱)

## 주요 명령어

| 명령어 | 설명 |
|--------|------|
| `pnpm dev` | Vite 개발 서버 |
| `pnpm tauri:dev` | Tauri 개발 모드 |
| `pnpm tauri:build` | MSI 설치파일 생성 |

## 개발 Phase

| Phase | 내용 | 상태 |
|-------|------|------|
| 1 | 기반 구축 (Tauri 셋업, TXT 검색) | ✅ 완료 |
| 2 | 파일 파서 (HWPX, DOCX, XLSX, PDF) | ✅ 완료 |
| 3 | 시맨틱 검색 (ONNX, e5-small) | ✅ 완료 |
| 4 | 고급 기능 (실시간 감시, 증분 인덱싱) | ✅ 완료 |
| 5 | 배포 (MSI, 자동 업데이트) | 🔄 진행예정 |

## 주요 기능

| 기능 | 설명 |
|------|------|
| 하이브리드 검색 | 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 |
| 파일명 검색 | Everything 스타일 파일명 검색 |
| 실시간 감시 | 폴더 변경 자동 감지 + 증분 인덱싱 |
| 인덱싱 진행률 | 실시간 진행률 + 취소 버튼 |
| 즐겨찾기 폴더 | 자주 사용 폴더 핀 고정 |
| 컴팩트 모드 | 결과 밀도 조절 (기본/컴팩트) |

## 참고 문서

| 문서 | 설명 |
|------|------|
| [PLAN.md](PLAN.md) | 상세 설계 계획 |
| [.claude/memory/activeContext.md](.claude/memory/activeContext.md) | 현재 작업 컨텍스트 |

## 코딩 컨벤션

- TypeScript strict mode
- Rust 2021 edition
- 함수형 React 컴포넌트
- Tailwind CSS 유틸리티

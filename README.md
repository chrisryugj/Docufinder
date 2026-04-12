# Anything

**내 컴퓨터의 모든 문서를 한 번에 검색** --- 키워드 + AI 시맨틱 + RAG 질의응답

[![Version](https://img.shields.io/badge/version-2.1.0-blue.svg)](https://github.com/chrisryugj/Docufinder/releases)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.10-24C8D8.svg)](https://tauri.app)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

![Anything 데모](./promo-video/out/video.mp4)

---

## What's New in v2.1.0

> 프로덕션 감사 34개 이슈 수정 + RAG 품질 대폭 개선

<details>
<summary><b>v2.1.0 (2026-04-11)</b> — 프로덕션 감사 + RAG 고도화</summary>

### Added
- AI RAG 출처 문서 번호 매칭 (근거 파일 강조)
- 파일 태그 시스템 (분류/검색용 커스텀 태그)
- 검색 결과 CSV/JSON 내보내기
- 폴더 트리 드래그&드롭 정렬
- 시스템 트레이 최소화 + 자동 시작 옵션
- 윈도우 상태 복원 (크기/위치 기억)

### Changed
- Anything 브랜딩 전면 적용
- RAG 프롬프트 개선 (출처 인라인 제거 + 수치 정확 인용)
- RAG 참조 파일을 실제 컨텍스트 사용분만 표시
- AI 패널 글자 크기 증가 (가독성)
- FTS 쿼리 최적화 (스마트 쿼리 파싱)

### Fixed
- DB 동시성 34개 이슈 일괄 수정
- 파서 안정성 강화 (DOCX/XLSX/PDF/TXT 에러 핸들링)
- 인덱싱 중 파일 삭제 시 크래시 방지
- 대용량 폴더 추가 시 UI 멈춤 해결

### Security
- 프로덕션 감사 전체 통과
- kordoc 번들링 (node.exe + cli.js MSI 포함)

</details>

<details>
<summary><b>v2.0.0 (2026-04-11)</b> — AI 문서 분석 + OCR</summary>

- AI 문서 분석 (Gemini RAG), AI 파일 QA, AI 요약
- OCR 지원 (PaddleOCR 기반 스캔 PDF)
- HWP 파일 변환 (kordoc 번들링)
- 전체 PC 인덱싱, 검색 범위 필터링, 유사 문서 찾기
- 중복 파일 탐지, 문서 통계, 법령 참조 링크
- OTA 자동 업데이트

</details>

<details>
<summary><b>v1.0.0 (2026-02-22)</b> — 초기 릴리스</summary>

- 하이브리드 검색 (FTS5 + 벡터 + RRF + Cross-Encoder)
- Everything 스타일 파일명 검색
- 실시간 폴더 감시 + 증분 인덱싱
- HWPX, DOCX, XLSX, PDF, TXT 지원
- 다크/라이트 테마, 색상 프리셋

</details>

---

## 주요 기능

| 기능 | 설명 |
|------|------|
| **하이브리드 검색** | 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 + Cross-Encoder 재정렬 |
| **AI RAG 질의응답** | Gemini 기반 문맥 Q&A — 출처 문서 번호 매칭 |
| **AI 요약** | TextRank 추출적 요약 (오프라인) + LLM 요약 |
| **파일명 검색** | Everything 스타일 인메모리 캐시 검색 |
| **실시간 감시** | 폴더 변경 자동 감지 + 증분 인덱싱 |
| **OCR** | PaddleOCR 기반 스캔 PDF 텍스트 추출 |
| **HWP 변환** | kordoc 번들링 — .hwp → .hwpx 자동 변환 |
| **법령 링크** | 정규식 기반 법령 자동 감지 → law.go.kr 연결 |
| **파일 태그** | 커스텀 태그로 문서 분류/검색 |
| **내보내기** | 검색 결과 CSV/JSON 다운로드 |

### 지원 문서 형식

| 형식 | 확장자 | 비고 |
|------|--------|------|
| 한글 | `.hwpx`, `.hwp` | HWP는 자동 변환 |
| 워드 | `.docx` | |
| 엑셀 | `.xlsx` | |
| PDF | `.pdf` | 스캔 PDF OCR 지원 |
| 텍스트 | `.txt` | EUC-KR/CP949 자동 감지 |

---

## 설치

### Windows 사용자

1. [Releases](https://github.com/chrisryugj/Docufinder/releases) 페이지에서 최신 `.msi` 다운로드
2. 설치 파일 실행
3. 첫 실행 시 ONNX 모델 자동 다운로드 (약 420MB, 1회)

**요구사항**: Windows 10 21H2+ / Windows 11, 4GB RAM 권장

> **Windows 보안 경고가 뜨나요?**
>
> 개인 개발 앱이라 아직 Microsoft 인증서가 없어서 설치 시 경고가 표시될 수 있습니다.
>
> <details>
> <summary><b>A. "Windows의 PC 보호" 화면이 뜰 때</b></summary>
>
> 1. **"추가 정보"** 를 클릭합니다
> 2. **"실행"** 버튼을 클릭합니다
> </details>
>
> <details>
> <summary><b>B. "스마트 앱 컨트롤이 차단" 화면이 뜰 때 (Win 11)</b></summary>
>
> 1. 설치 파일 우클릭 → **"속성"**
> 2. 하단 **"차단 해제"** 체크 → 확인
> 3. 설치 파일 다시 실행
> </details>
>
> <details>
> <summary><b>C. 더블클릭해도 아무 반응이 없을 때</b></summary>
>
> 1. 백신(V3, 알약 등)의 **"실시간 감시"** 를 일시 중지
> 2. 설치 파일 다시 더블클릭
> 3. 설치 완료 후 실시간 감시 다시 켜기
> </details>
>
> 사용자가 늘어나면 이 경고는 자연스럽게 사라집니다.

### 자동 업데이트

설치 후 앱이 자동으로 새 버전을 감지하여 업데이트 배너를 표시합니다.

---

## 사용법

### 1단계: 폴더 등록
앱 실행 → 좌측 "폴더 추가" → 문서 폴더 선택 → 자동 인덱싱 시작

### 2단계: 검색
상단 검색창에 입력 → Enter → 결과 클릭(미리보기) / 더블클릭(열기)

### 3단계: AI 활용
- **RAG 질의**: 검색 후 AI 패널에서 문서 기반 질문
- **파일 QA**: 단일 파일 우클릭 → AI에게 질문
- **요약**: 파일 우클릭 → AI 요약

### 검색 모드

| 모드 | 단축키 | 설명 |
|------|--------|------|
| 하이브리드 | 기본 | 키워드 + 의미 검색 결합 |
| 키워드 | - | 정확한 단어 매칭 (형태소 분석) |
| 시맨틱 | - | 의미 기반 유사 문서 검색 |
| 파일명 | - | Everything 스타일 파일명 검색 |

---

## 기술 스택

| 영역 | 기술 |
|------|------|
| Frontend | React 19 + TypeScript 5.9 + Tailwind CSS 4 |
| Backend | Rust 2021 + Tauri 2.10 |
| 검색 | SQLite FTS5 + usearch (HNSW) + RRF 하이브리드 |
| 임베딩 | ONNX Runtime + KoSimCSE-roberta-multitask (768차원) |
| 형태소 분석 | Lindera 2.0 (한국어) |
| 재정렬 | ms-marco-MiniLM-L6-v2 (Cross-Encoder) |
| AI | Gemini API (RAG 질의응답) |
| OCR | PaddleOCR (스캔 PDF) |
| 파싱 | zip, quick-xml, calamine, pdf-extract, kordoc |
| 파일 감시 | notify 8 (증분 인덱싱) |

---

## 개발

### 로컬 개발 환경

```bash
# 의존성 설치
pnpm install

# ONNX 모델 다운로드 (첫 빌드 시)
pnpm run download-model

# 개발 서버 실행
pnpm tauri:dev

# 프로덕션 빌드 (MSI 생성)
pnpm tauri:build
```

### 빌드 요구사항

- Windows 10/11 x64
- Node.js 22 LTS + pnpm 10
- Rust 1.92+ (stable)
- Visual Studio Build Tools 2022 (C++ workload)

### 프로젝트 구조

```
Anything/
├── src/                    # React 프론트엔드
│   ├── components/         # UI 컴포넌트
│   ├── hooks/              # 커스텀 훅 (14개)
│   ├── types/              # TypeScript 타입
│   └── utils/              # 유틸리티
├── src-tauri/              # Rust 백엔드 (Clean Architecture)
│   ├── src/
│   │   ├── commands/       # Tauri IPC 커맨드
│   │   ├── application/    # 응용 계층 (services, container)
│   │   ├── domain/         # 도메인 계층 (entities, repositories)
│   │   ├── infrastructure/ # 인프라 계층
│   │   ├── parsers/        # 문서 파서 (hwpx, docx, xlsx, pdf, txt, kordoc)
│   │   ├── search/         # 검색 엔진 (fts, vector, hybrid, filename)
│   │   ├── indexer/        # 인덱싱 (pipeline, sync, vector_worker)
│   │   ├── embedder/       # ONNX 임베딩
│   │   ├── tokenizer/      # 형태소 분석 (Lindera)
│   │   └── reranker/       # 결과 재정렬
│   └── Cargo.toml
├── .github/workflows/      # CI (ci.yml) + Release (publish.yml)
├── scripts/                # 빌드 스크립트 (PowerShell)
└── promo-video/            # 소개 영상 (Remotion)
```

자세한 내용: [BUILD_GUIDE.md](BUILD_GUIDE.md) | [DEPLOYMENT.md](DEPLOYMENT.md) | [CHANGELOG.md](CHANGELOG.md)

---

## FAQ

<details>
<summary><b>인터넷 연결이 필요한가요?</b></summary>
검색과 인덱싱은 100% 로컬 처리. AI RAG 질의응답만 Gemini API 연결이 필요합니다.
</details>

<details>
<summary><b>HWP 파일도 검색되나요?</b></summary>
네! kordoc 엔진이 .hwp를 자동으로 .hwpx로 변환하여 검색합니다. 별도 설치 불필요.
</details>

<details>
<summary><b>인덱싱에 시간이 얼마나 걸리나요?</b></summary>
SSD 기준 약 1,000개 문서에 2-5분. HDD는 자동 감지하여 적응형 스레딩으로 최적화합니다.
</details>

<details>
<summary><b>파일을 수정하면 다시 인덱싱해야 하나요?</b></summary>
아니요. 실시간 폴더 감시로 파일 추가/수정/삭제를 자동 반영합니다.
</details>

---

## 라이선스

[MIT License](LICENSE) - Copyright 2025-2026 Chris

---

## 기여

버그 리포트, 기능 제안, PR 모두 환영합니다!

1. [Issues](https://github.com/chrisryugj/Docufinder/issues)에서 이슈 등록
2. Fork → 기능 개발 → PR 제출

---

**Made with [Tauri](https://tauri.app) + [React](https://react.dev)**

<p align="center">
  <img src="public/anything.png" alt="Anything" width="80" />
</p>

<h1 align="center">Anything</h1>

<p align="center">
  <b>내 PC 문서를 통째로 검색하는 로컬 검색 엔진</b><br/>
  파일명 몰라도, 열어보지 않아도. 문서 안의 <i>내용</i>으로 찾습니다.
</p>

<p align="center">
  <a href="https://github.com/chrisryugj/Docufinder/releases"><img src="https://img.shields.io/badge/download-Windows-0078D4?style=for-the-badge&logo=windows" alt="Download" /></a>
</p>

<p align="center">
  <a href="https://github.com/chrisryugj/Docufinder/releases"><img src="https://img.shields.io/badge/version-2.5.0-blue.svg" alt="Version" /></a>
  <a href="https://tauri.app"><img src="https://img.shields.io/badge/Tauri-2.10-24C8D8.svg" alt="Tauri 2" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSL%201.1-blue.svg" alt="License" /></a>
</p>

<p align="center">
  <img src="docs/promo.gif" alt="Anything - 문서 내용 검색 데모" width="820" />
</p>

---

## 이런 걸 할 수 있습니다

### 문서 내용 검색
폴더를 등록하면 자동으로 인덱싱합니다. 검색창에 키워드를 치면 **문서 안의 본문**에서 결과를 찾아줍니다. 수천 개 문서도 1초 안에.

### 파일명 검색
Everything처럼 파일명 일부만 입력하면 인메모리 캐시에서 **즉시** 찾습니다. 인덱싱이 끝나기 전에도 사용 가능.

### AI 질의응답 (선택)
"2026년 예산 얼마야?", "연차 조건이 뭐야?" 같은 자연어 질문을 하면 인덱싱된 문서에서 관련 부분을 찾아 답변합니다. **근거 문서 + 페이지**까지 표시. Gemini API 키가 필요하며, 없어도 검색은 정상 동작합니다.

### AI 문서 요약 (선택)
파일 우클릭 → 요약. 계약서, 보고서, 회의록 등 문서 타입에 맞춰 핵심만 뽑아줍니다. 인터넷 없이도 오프라인 요약(TextRank) 가능.

### 실시간 동기화
파일을 추가/수정/삭제하면 자동으로 반영됩니다. 수동 재인덱싱 필요 없음.

---

## 지원 파일 형식

| 형식 | 확장자 | 비고 |
|------|--------|------|
| 한글 | `.hwpx` `.hwp` | HWP는 kordoc 엔진으로 자동 변환 |
| 워드 | `.docx` | |
| 파워포인트 | `.pptx` | |
| 엑셀 | `.xlsx` `.xls` | 시트·행 위치까지 추적 |
| PDF | `.pdf` | 스캔 PDF는 OCR 자동 적용 |
| 이미지 | `.jpg` `.png` `.bmp` `.tiff` | OCR로 텍스트 추출 |
| 텍스트 | `.txt` `.md` | EUC-KR/CP949 자동 감지 |

---

## 설치

### 다운로드

[Releases](https://github.com/chrisryugj/Docufinder/releases) 페이지에서 `.msi` 파일을 받아 실행하면 끝.

- **Windows 10 (21H2 이상)** 또는 **Windows 11**
- RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB 이상
- **인터넷 연결 필요** (최초 1회) — 모델 자동 다운로드 (약 420MB)
- 관리자 권한 필요 (MSI 표준 설치, UAC 프롬프트 1회)
- 이후 새 버전이 나오면 앱이 자동으로 알려줍니다

> WebView2, VC++ 런타임은 설치 파일에 포함되어 있어 별도 설치 불필요합니다.

<details>
<summary><b>설치 시 보안 경고가 뜰 때</b> (클릭)</summary>

개인 개발 앱이라 Microsoft 코드서명 인증서(연 수십만 원)가 없어서 경고가 표시됩니다. **악성코드가 아니며**, 소스코드는 전부 이 저장소에서 확인할 수 있습니다.

**1. "Windows의 PC 보호" 파란 창**
- 좌측 하단 **"추가 정보"** 클릭
- 아래쪽 **"실행"** 버튼 클릭

**2. "스마트 앱 컨트롤이 차단" (Windows 11)**
- 파일 탐색기에서 MSI 파일 **우클릭 → 속성**
- 하단 **"차단 해제"** 체크 후 적용
- 다시 더블클릭으로 실행

**3. 다운로드 자체가 막힐 때 (Edge/Chrome)**
- 브라우저 다운로드 창에서 **"..."(점 3개)** → **"유지"** 선택
- Edge: **"안전하지 않은 파일 유지"** 링크 클릭

**4. 백신이 파일을 자동 격리/삭제할 때**
- Windows Defender: 설정 → "바이러스 및 위협 방지" → "보호 기록"에서 **복원**
- 타사 백신 (V3, 알약 등): 실시간 감시 일시 중지 후 재시도
- 기업 PC는 IT 관리자 문의 (AppLocker 정책)

**5. 설치 후 앱이 안 열릴 때**
- `%APPDATA%\com.anything.app\crash.log` 내용과 함께 [Issues](https://github.com/chrisryugj/Docufinder/issues) 제보

</details>

---

## 사용법

1. 앱 실행 → 좌측 **"폴더 추가"** → 문서 폴더 선택 (자동 인덱싱 시작)
2. 검색창에 입력 → 결과 클릭으로 미리보기, 더블클릭으로 파일 열기
3. (선택) AI 패널에서 자연어 질문 → 문서 기반 답변 확인

### 검색 모드

| 모드 | 설명 |
|------|------|
| 키워드 | 기본값. 정확한 단어 매칭 |
| 하이브리드 | 키워드 + 의미 검색 결합 |
| 시맨틱 | 의미 기반 유사 문서 검색 |
| 파일명 | Everything 스타일 파일명 검색 |

> 하이브리드/시맨틱 모드는 설정에서 시맨틱 검색을 활성화해야 사용할 수 있습니다.

---

## 보안 & 데이터 흐름

**AI 기능을 끄면 네트워크 통신이 완전히 제로입니다.** 폐쇄망·내부망 환경에서 그대로 사용할 수 있습니다.

| 기능 | 데이터 위치 | 외부 전송 |
|------|------------|----------|
| 문서 파싱·인덱싱 | 로컬 SQLite | 없음 |
| 키워드·시맨틱 검색 | 로컬 FTS5 + 벡터 DB | 없음 |
| 임베딩 (KoSimCSE) | 로컬 ONNX 모델 | 없음 |
| OCR (PaddleOCR) | 로컬 ONNX 모델 | 없음 |
| 파일명 검색 | 로컬 인메모리 캐시 | 없음 |
| AI 질의응답 | **Gemini API** | 질문 + 관련 청크만 전송 |
| AI 요약 (온라인) | **Gemini API** | 문서 텍스트 전송 |
| AI 요약 (오프라인) | 로컬 TextRank | 없음 |

- **원본 파일은 절대 복사되지 않습니다** — 인덱스만 생성
- **AI 기능은 설정에서 완전히 비활성화** 가능 → 순수 로컬 검색 도구로 동작
- **API 키는 사용자 PC 로컬에만 저장** — 서버를 거치지 않음
- **자동 업데이트 확인**은 GitHub Releases 엔드포인트만 조회 (비활성화 가능)

---

## 아키텍처

```
┌─────────────────────────────────────────────────┐
│  React 19 + TypeScript + Tailwind CSS           │  ← UI
├─────────────────────────────────────────────────┤
│  Tauri 2 IPC                                    │  ← 브릿지
├─────────────────────────────────────────────────┤
│  Rust Backend (Clean Architecture)              │
│  ┌───────────┬───────────┬────────────────────┐ │
│  │  Parsers  │  Indexer  │  Search Engine     │ │
│  │  hwpx     │  FTS5     │  키워드 (FTS5)     │ │
│  │  docx     │  벡터     │  시맨틱 (usearch)  │ │
│  │  xlsx     │  파일감시  │  하이브리드 (RRF)  │ │
│  │  pdf/ocr  │           │  파일명 (캐시)     │ │
│  │  txt      │           │                    │ │
│  └───────────┴───────────┴────────────────────┘ │
│  ┌────────────────────────────────────────────┐  │
│  │  SQLite (FTS5) · usearch (HNSW) · ONNX    │  │  ← 저장소
│  └────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

| 영역 | 기술 |
|------|------|
| Frontend | React 19, TypeScript 5.9, Tailwind CSS 4 |
| Backend | Rust 2021, Tauri 2.10 |
| 검색 | SQLite FTS5 + usearch HNSW + RRF 병합 |
| 한국어 처리 | Lindera 2.0 형태소 분석 |
| 임베딩 | ONNX Runtime, KoSimCSE-roberta (768차원) |
| AI | Gemini API (RAG) |
| OCR | PaddleOCR ONNX |
| HWP 파싱 | [kordoc](https://www.npmjs.com/package/kordoc) (번들 포함) |

---

## 개발자용

```bash
pnpm install          # 의존성 설치
pnpm run download-model  # ONNX 모델 다운로드 (첫 빌드 시)
pnpm tauri:dev        # 개발 모드
pnpm tauri:build      # 프로덕션 빌드 (MSI)
```

**빌드 요구사항**: Windows 10/11 x64 · Node.js 22 LTS + pnpm 10 · Rust 1.92+ · Visual Studio Build Tools 2022

자세한 내용은 [BUILD_GUIDE.md](BUILD_GUIDE.md) · [DEPLOYMENT.md](DEPLOYMENT.md)를 참고하세요.

---

## FAQ

<details>
<summary><b>폐쇄망/내부망에서 쓸 수 있나요?</b></summary>
네. AI 기능을 끄면 앱이 외부와 통신하는 경로가 없습니다. ONNX 모델 파일만 수동으로 복사하면 검색·인덱싱·임베딩·OCR 전부 오프라인 동작합니다.
</details>

<details>
<summary><b>파일이 외부로 전송되나요?</b></summary>
AI 질의응답을 쓸 때만 질문과 관련된 텍스트 조각이 Gemini API로 전송됩니다. AI를 끄면 전송되는 데이터는 없습니다. 원본 파일은 어떤 경우에도 외부로 나가지 않습니다.
</details>

<details>
<summary><b>HWP 파일도 검색되나요?</b></summary>
네. kordoc 엔진이 앱에 내장되어 있어서 한컴오피스 없이도 .hwp 파일을 파싱합니다.
</details>

<details>
<summary><b>인덱싱은 얼마나 걸리나요?</b></summary>
SSD 기준 약 1,000개 문서에 2~5분. FTS 인덱싱이 먼저 완료되어 바로 검색할 수 있고, 벡터 인덱싱은 백그라운드에서 이어집니다.
</details>

---

## 라이선스

[Business Source License 1.1](LICENSE) — Copyright 2025-2026 chrisryugj

- 비프로덕션(개발·테스트·학습) 자유 사용
- 프로덕션/상용은 별도 라이선스 필요
- 2030-04-15 이후 Apache License 2.0 자동 전환

상용 라이선스 문의: ryuseungin@gmail.com

---

버그 리포트·기능 제안은 [Issues](https://github.com/chrisryugj/Docufinder/issues)에서 환영합니다.

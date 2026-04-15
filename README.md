# Anything

**내 컴퓨터가 나한테 대답하기 시작했다.** 키워드 검색부터 AI 문서 질의응답까지, 전부 로컬에서.

[![Version](https://img.shields.io/badge/version-2.1.0-blue.svg)](https://github.com/chrisryugj/Docufinder/releases)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.10-24C8D8.svg)](https://tauri.app)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

> *매년 쌓이는 보고서·공문·회의록 수천 개. 어디에 뭐가 있는지 기억하는 사람은 없습니다. 그래서 만들었습니다.*

![Anything 데모](./promo-video/out/promo.gif)

---

## 이런 질문이 됩니다

사용법은 단순합니다. **그냥 자연어로 물어보세요.** 인덱싱된 문서 수천 개 중에서 관련 부분만 찾아서 답해줍니다.

### "2026년 노인일자리 사업 예산 얼마야?"

```
📊 2026년 노인일자리 사업 예산
```

→ 해당 연도의 **예산표**를 찾아서 사업별 배정액을 그대로 인용합니다. 표가 청크 경계에 걸려도 앞뒤 청크를 함께 읽어서 **표가 잘리지 않습니다**. 답변 끝에는 근거가 된 **문서 파일명 + 페이지**가 함께 표시됩니다.

### "연차 사용 조건이 어떻게 돼?"

```
📜 복무규정 / 인사규정
```

→ 회사/기관 내부 문서에서 **연차 조항 원문**을 찾고, 조건·한도·예외를 정리합니다. 규정 간 충돌이 있으면 **문서별로 나눠서** 보여줍니다.

### "지난주에 받은 입찰공고 중에 IT 관련된 거만"

```
📅 최근 7일 · 유형 필터 · 키워드 매칭
```

→ 자연어에서 **기간·파일 타입·키워드를 자동 추출**해서 필터로 변환합니다. "지난주", "올해", "2026년" 같은 표현을 모두 인식합니다.

### "이 계약서 핵심만 3줄로"

```
📝 단일 파일 요약
```

→ 파일 우클릭 → AI 요약. 문서 타입별 프롬프트(계약서/보고서/회의록)로 **편향 없이 추출**합니다. 인터넷 없이 쓰고 싶으면 **TextRank 오프라인 모드**도 있습니다.

### "아까 본 그 보고서 어디 있었지?"

```
🔍 파일명 검색 (Everything 스타일)
```

→ 파일명 일부만 쳐도 인메모리 캐시에서 **즉시** 찾습니다. 인덱싱 기다릴 필요 없습니다.

---

## 왜 로컬인가

클라우드 검색 서비스는 편하지만 **민감한 문서를 외부로 올릴 수 없는 조직**이 많습니다. Anything은:

- **검색·인덱싱은 100% 로컬** — 임베딩 모델(KoSimCSE 768차원)도 사용자 PC에서 돌아갑니다.
- **AI 질의응답만 선택적으로 Gemini API** — 싫으면 끄면 됩니다. 검색만으로도 쓸 만합니다.
- **파일을 복사하지 않습니다** — 인덱스만 만듭니다. 원본은 그대로.
- **오프라인 모드 제공** — 출장 중에도, 폐쇄망에서도 동작합니다.

---

## 무엇을 검색할 수 있나

| 형식 | 확장자 | 비고 |
|------|--------|------|
| 한글 | `.hwpx`, `.hwp` | kordoc 엔진으로 HWP 자동 변환 |
| 워드 | `.docx` | |
| 엑셀 | `.xlsx` | 시트·행 위치까지 추적 |
| PDF | `.pdf` | 스캔 PDF는 PaddleOCR로 OCR |
| 텍스트 | `.txt` | EUC-KR/CP949 자동 감지 |

검색 엔진은 **하이브리드**입니다. SQLite FTS5 키워드 매칭 + KoSimCSE 벡터 시맨틱 + RRF 병합 + Cross-Encoder 재정렬. 한국어 형태소는 Lindera로 분석합니다.

---

## v2.1.0 변경사항

<details open>
<summary><b>v2.1.0 (2026-04)</b> — RAG 품질 + 프로덕션 감사</summary>

- **RAG 이웃 청크 확장** — 표/리스트가 청크 경계에서 잘리던 문제 해결. 검색으로 걸린 청크의 앞뒤 ±1 청크를 자동으로 함께 LLM에 전달해 **"표가 잘렸다"고 답하는 문제**를 근본 해결.
- **DB 무결성 검사 백그라운드화** — 스타트업에 수십 초 걸리던 `PRAGMA integrity_check`를 백그라운드 `quick_check`로 전환. **앱 시작 즉시 검색 가능**.
- **AI RAG 출처 번호 매칭** — LLM 답변에서 `[출처: 1, 3]` 패턴을 파싱해 실제 참조된 문서만 "근거" 뱃지로 강조.
- **프로덕션 감사 34개 이슈 수정** — DB 동시성, 파서 안정성, 인덱싱 중 파일 삭제 크래시 방지, 대용량 폴더 UI 멈춤 해결.
- **RAG 프롬프트 고도화** — 출처 인라인 제거 + 수치 정확 인용 강화, 참조 파일을 실제 컨텍스트 사용분만 표시.
- **검색 범위 필터** — 폴더 스코프 드롭다운 + 파일명 검색도 범위 필터 적용.
- **내보내기** — 검색 결과 CSV/JSON 다운로드.
- **시스템 트레이** — 트레이 최소화 + 자동 시작 옵션.

</details>

<details>
<summary><b>v2.0.0 (2026-04)</b> — AI 문서 분석 + OCR</summary>

- AI 문서 분석 (Gemini RAG), AI 파일 QA, AI 요약 (3가지 모드)
- OCR 지원 (PaddleOCR 기반 스캔 PDF)
- HWP 파일 변환 — kordoc 엔진 번들링 (node.exe 포함, 사용자 Node.js 설치 불필요)
- 전체 PC 인덱싱, 검색 범위 필터링, 유사 문서 찾기
- 중복 파일 탐지, 문서 통계, 법령 참조 자동 링크
- OTA 자동 업데이트

</details>

<details>
<summary><b>v1.0.0 (2026-02)</b> — 초기 릴리스</summary>

- 하이브리드 검색 (FTS5 + 벡터 + RRF + Cross-Encoder)
- Everything 스타일 파일명 검색 (인메모리 캐시)
- 실시간 폴더 감시 + 증분 인덱싱
- HWPX, DOCX, XLSX, PDF, TXT 지원
- 다크/라이트 테마, 색상 프리셋

</details>

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

설치 후 앱이 새 버전을 자동 감지해서 업데이트 배너를 띄워줍니다.

---

## 사용 흐름

**1단계**: 앱 실행 → 좌측 "폴더 추가" → 문서 폴더 선택 → 자동 인덱싱 시작 (백그라운드)
**2단계**: 상단 검색창에 입력 → Enter → 결과 클릭으로 미리보기, 더블클릭으로 원본 열기
**3단계**: 필요하면 우측 AI 패널에서 자연어 질문 → 문서 기반 답변 + 근거 표시

### 검색 모드

| 모드 | 설명 |
|------|------|
| **하이브리드** | 기본값. 키워드 + 의미 검색 결합 (추천) |
| **키워드** | FTS5 정확 매칭 (형태소 분석 기반) |
| **시맨틱** | 의미 기반 유사 문서 검색 |
| **파일명** | Everything 스타일 — 인덱싱 불필요 |

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
| OCR | PaddleOCR ONNX (스캔 PDF) |
| 파싱 | zip, quick-xml, calamine, pdf-extract, [kordoc](https://www.npmjs.com/package/kordoc) |
| 파일 감시 | notify 8 (증분 인덱싱) |

---

## 개발

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

**빌드 요구사항**: Windows 10/11 x64 · Node.js 22 LTS + pnpm 10 · Rust 1.92+ · Visual Studio Build Tools 2022

자세한 내용: [BUILD_GUIDE.md](BUILD_GUIDE.md) · [DEPLOYMENT.md](DEPLOYMENT.md) · [CHANGELOG.md](CHANGELOG.md)

---

## FAQ

<details>
<summary><b>인터넷 연결이 필요한가요?</b></summary>
검색과 인덱싱은 100% 로컬 처리입니다. AI RAG 질의응답만 Gemini API 연결이 필요합니다. API 키는 설정에서 직접 입력하며, 비활성화도 가능합니다.
</details>

<details>
<summary><b>HWP 파일도 검색되나요?</b></summary>
네. kordoc 엔진을 번들링해서 <code>.hwp</code> 파일을 자동으로 <code>.hwpx</code>로 변환한 뒤 파싱합니다. 사용자가 Node.js나 한컴오피스를 별도로 설치할 필요 없습니다.
</details>

<details>
<summary><b>인덱싱에 시간이 얼마나 걸리나요?</b></summary>
SSD 기준 약 1,000개 문서에 2-5분. HDD는 자동 감지하여 적응형 스레딩으로 최적화합니다. 인덱싱은 2단계 — FTS 먼저 완료(즉시 검색 가능) → 벡터 백그라운드 처리.
</details>

<details>
<summary><b>파일을 수정하면 다시 인덱싱해야 하나요?</b></summary>
아니요. 실시간 폴더 감시로 파일 추가/수정/삭제를 자동 반영합니다. <code>notify</code> 크레이트로 OS 이벤트를 구독합니다.
</details>

<details>
<summary><b>문서가 외부로 전송되나요?</b></summary>
AI 질의응답 기능을 쓸 때만 <b>질문과 관련된 청크</b>가 Gemini API로 전송됩니다. 검색·인덱싱·임베딩·요약(오프라인 모드)은 모두 로컬입니다.
</details>

<details>
<summary><b>폐쇄망에서도 쓸 수 있나요?</b></summary>
네. ONNX 모델을 수동 복사하면 인덱싱·검색 전부 오프라인 동작합니다. AI 기능만 비활성화됩니다.
</details>

---

## 라이선스

[Business Source License 1.1](LICENSE) — Copyright 2025-2026 chrisryugj (개친절한 류주임)

- **비프로덕션 사용 허용**: 개발·테스트·평가·학습 목적으로 자유롭게 사용·수정·재배포 가능
- **프로덕션 사용 제한**: 상용/프로덕션 용도는 별도 상용 라이선스 필요
- **Change Date**: 2030-04-15 이후 Apache License 2.0으로 자동 전환

상용 라이선스 문의: ryuseungin@gmail.com

버그 리포트·기능 제안·PR 환영합니다. [Issues](https://github.com/chrisryugj/Docufinder/issues)에서 등록해주세요.

---

**Made with [Tauri](https://tauri.app) + [React](https://react.dev) + [kordoc](https://www.npmjs.com/package/kordoc)**

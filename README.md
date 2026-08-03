<p align="center">
  <img src="public/anything.png" alt="Anything" width="80" />
</p>

<h1 align="center">Anything</h1>

<p align="center">
  <b>내 PC 문서를 통째로 뒤지는 100% 로컬 검색 엔진</b><br/>
  파일명을 몰라도, 열어보지 않아도 — 문서 <i>안의 내용</i>으로 찾습니다.<br/>
  한글(HWP/HWPX)·워드·엑셀·PDF·이미지까지, 전부 내 컴퓨터 안에서.
</p>

<p align="center">
  <a href="https://github.com/chrisryugj/Docufinder/releases"><img src="https://img.shields.io/badge/Windows-x64-0078D4?style=for-the-badge&logo=windows&logoColor=white" alt="Download for Windows" /></a>
  &nbsp;
  <a href="https://github.com/chrisryugj/Docufinder/releases"><img src="https://img.shields.io/badge/macOS-Apple%20Silicon-000000?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS" /></a>
</p>

<p align="center">
  <a href="https://github.com/chrisryugj/Docufinder/releases"><img src="https://img.shields.io/badge/version-3.5.0-blue.svg" alt="Version" /></a>
  <a href="https://tauri.app"><img src="https://img.shields.io/badge/Tauri-2.10-24C8D8.svg" alt="Tauri 2" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSL%201.1-blue.svg" alt="License" /></a>
</p>

<p align="center">
  <a href="https://youtu.be/xuCGvqG6_SE"><img src="docs/video-demo.jpg" alt="Anything(문서내용검색) 활용하기 — 영상 보기" width="820" /></a>
</p>

<p align="center">
  <sub>▶ 클릭하면 유튜브에서 재생됩니다.</sub>
</p>

---

## 이런 걸 할 수 있습니다

### 문서 내용 검색
폴더를 등록하면 자동으로 인덱싱합니다. 검색창에 키워드를 치면 **문서 안의 본문**에서 결과를 찾아줍니다. 수천 개 문서도 1초 안에.

### 파일명 검색
Everything처럼 파일명 일부만 입력하면 인메모리 캐시에서 **즉시** 찾습니다. 인덱싱이 끝나기 전에도 사용 가능. 결과는 **정렬 가능한 컬럼 뷰**(이름·경로·크기·수정일·유형)로 표시됩니다 — 컬럼을 클릭해 정렬하고, 경계를 드래그해 너비를 조절(더블클릭 시 자동 맞춤)하며, 헤더 우클릭으로 컬럼을 켜고 끌 수 있습니다. 너비·정렬·표시 설정은 자동 저장됩니다.

### AI 질의응답 (선택)
"2026년 예산 얼마야?", "연차 조건이 뭐야?" 같은 자연어 질문을 하면 인덱싱된 문서에서 관련 부분을 찾아 답변합니다. **근거 문서 + 페이지**까지 표시. Gemini 또는 OpenAI 호환 서버(사내·오프라인 LLM 포함)의 API 키가 필요하며, 없어도 검색은 정상 동작합니다.

### AI 문서 요약 (선택)
파일 우클릭 → 요약. 계약서, 보고서, 회의록 등 문서 타입에 맞춰 핵심만 뽑아줍니다. 요약은 AI 제공자(Gemini 또는 OpenAI 호환 서버)를 호출하므로 API 키 설정이 필요합니다.

### 실시간 동기화
파일을 추가/수정/삭제하면 자동으로 반영됩니다. 수동 재인덱싱 필요 없음.

### 문서 미리보기 & 크게 보기
결과를 클릭하면 문서를 미리보기로 렌더합니다. 병합·중첩된 표도 원본 구조 그대로 표시하고, 전체화면 팝업 뷰어에서 커서 기준 휠 줌·더블클릭 확대·너비/페이지 맞춤으로 크게 볼 수 있습니다. Ctrl+F로 문서 안에서 바로 찾기도 됩니다.

**원본 레이아웃 보기 (v3.2)** — 텍스트 미리보기 외에 원본 조판 그대로 보는 뷰(단축키 2)를 지원합니다. `.hwpx`(kordoc SVG — 글꼴·가로문서·다구역까지 충실 렌더), `.pdf`(원본 페이지 이미지), **`.hwp`(rhwp 네이티브 렌더 — 한컴 설치 없이)** 삼종 모두.

### 문서 비교
같은 문서의 버전이 여러 개일 때, 결과 우클릭 → '비교 대상으로 선택' 후 다른 결과와 비교하면 두 문서의 달라진 부분을 나란히 보여줍니다.

### 파일 드래그아웃
검색 결과를 다른 앱이나 웹 페이지로 바로 끌어다 놓을 수 있습니다 (파일 탐색기 드래그와 동일). **Shift를 누른 채 드래그하면 복사 대신 이동**합니다.

---

## 지원 파일 형식

| 형식 | 확장자 | 비고 |
|------|--------|------|
| 한글 | `.hwpx` `.hwp` | HWP5 + **HWP3 (1996~2002 구버전)** kordoc 엔진으로 자동 변환 |
| 워드 | `.docx` | |
| 파워포인트 | `.pptx` | |
| 엑셀 | `.xlsx` `.xls` | 시트·행 위치까지 추적 |
| PDF | `.pdf` | 스캔 PDF OCR · 수식(LaTeX) 인식 — 설정에서 켜기 |
| 이미지 | `.jpg` `.png` `.bmp` `.tiff` | OCR로 텍스트 추출 — 설정에서 켜기 |
| 텍스트 | `.txt` `.md` | EUC-KR/CP949 자동 감지 |
| 메일 | `.eml` | 제목·보낸사람·받는사람·본문 검색 (charset 자동 디코딩) |

---

## 설치

### 다운로드 — 어느 파일을 받아야 하나?

[Releases](https://github.com/chrisryugj/Docufinder/releases) 페이지에서 본인 환경에 맞게 받으세요.

| 내 PC 상황 | 받을 파일 | 크기 |
|------------|-----------|------|
| 집 / 개인 PC, Windows 10 (21H2 이상) / Windows 11 | **`Anything_<버전>_x64-setup.exe`** 하나만 | ~365 MB |
| 회사 PC인데 관리자 권한이 없거나, Windows 10 LTSC 같은 구버전, 또는 첫 실행 때 WebView2 오류가 떴던 분 | **`Anything_<버전>_x64-setup.exe`** + **`MicrosoftEdgeWebView2RuntimeInstallerX64.exe`** | ~365 MB + ~190 MB |

> **WebView2 런타임이 핵심입니다.** 이 앱은 Windows 의 Microsoft Edge WebView2 런타임 위에서 화면을 그립니다. 개인 PC 는 보통 이미 깔려 있어 setup 하나로 끝나지만, WebView2 가 없는 회사 PC / LTSC 환경에서는 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 를 **먼저(가능하면 관리자 권한으로)** 설치한 뒤 앱 setup 을 실행하세요.

**공통 요건**
- RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB 이상
- 최초 1회 인터넷 연결 (검색 모델 자동 다운로드, 약 420MB)
- 이후 새 버전이 나오면 앱 안에서 자동으로 알려줍니다

**오프라인 사내망 / 망간(단방향) 전송 PC 에 설치하려면**

> 과거의 통합본(`...-ltsc-setup.exe`, ~573MB)은 망간 전송에서 파일이 깨지거나 실행이 안 되는 문제로 폐기했습니다. 아래처럼 **두 파일을 나눠서** 옮기세요 (이슈 #29 제보자 검증 — 큰 단일 파일보다 전송 성공률이 높습니다).

1. 인터넷 가능한 PC 에서 두 파일을 받기
   - `Anything_<버전>_x64-setup.exe`
   - `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`
2. 두 파일을 **각각** USB / 망간 전송으로 사내 PC 로 옮기기 (한 번에 하나씩)
3. 사내 PC 에서 **`MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 를 먼저 설치** (가능하면 관리자 권한)
4. 이어서 `Anything_<버전>_x64-setup.exe` 더블클릭 — 끝
   - WebView2 가 이미 있는 PC 라면 1번에서 앱 setup 하나만 옮겨도 됩니다.

> **OCR 도 설치본만으로 동작합니다** (v3.4.3+). OCR 엔진·스캔 PDF 처리에 필요한 파일이 모두
> 설치본에 들어 있어 실행 중 내려받지 않습니다. 네트워크 시도 자체를 원천 차단하고 싶다면
> 시스템 환경변수 **`DOCUFINDER_OFFLINE=1`** 을 설정하세요 (선택 — 평상시엔 필요 없습니다).
> 이 스위치를 켜면 '레이아웃 분석'(온라인 전용 선택 기능)만 조용히 비활성되고, OCR 본체와
> 검색·인덱싱은 그대로 동작합니다.

<details>
<summary><b>설치 시 보안 경고가 뜰 때</b> (클릭)</summary>

개인 개발 앱이라 Microsoft 코드서명 인증서(연 수십만 원)가 없어서 경고가 표시됩니다. **악성코드가 아니며**, 소스코드는 전부 이 저장소에서 확인할 수 있습니다.

**1. "Windows의 PC 보호" 파란 창**
- 좌측 하단 **"추가 정보"** 클릭
- 아래쪽 **"실행"** 버튼 클릭

**2. "스마트 앱 컨트롤이 차단" (Windows 11)**
- 파일 탐색기에서 설치 파일(.exe) **우클릭 → 속성**
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

**6. "WebView2 런타임 오류" 가 떠서 앱이 안 열려요**
- 다이얼로그에 *"You may have it installed on another user account..."* 같은 영문이 보이거나, "WebView2" 가 어쩌고 하는 오류로 시작 자체가 안 되는 경우.
- **해결: `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 를 받아 (가능하면 관리자 권한으로) 설치한 뒤 앱을 다시 실행하세요.** 기존 앱은 제거하지 않아도 됩니다.
- 인덱싱 / 폴더 설정 등 사용자 데이터는 그대로 보존됩니다.
- 이래도 안 열리면 `%APPDATA%\com.anything.app\logs\` 안의 최신 로그 파일과 함께 [Issues](https://github.com/chrisryugj/Docufinder/issues) 로 제보해 주세요.

</details>

### macOS (Apple Silicon)

[Releases](https://github.com/chrisryugj/Docufinder/releases) 페이지에서 `.dmg` 파일을 받아 실행.

- **macOS 11 (Big Sur) 이상** · Apple Silicon (M1/M2/M3) 전용
- Intel Mac 미지원 (필요 시 [Issues](https://github.com/chrisryugj/Docufinder/issues)에 요청)
- RAM 8GB 이상 권장 · 디스크 여유 1GB 이상
- **자동 업데이트 미지원** — 새 버전은 수동 다운로드

**설치 순서**
1. `.dmg` 더블클릭 → Applications 폴더로 드래그
2. **첫 실행만**: Applications 폴더에서 Anything 우클릭 → "열기" → 경고창에서 다시 "열기"

**"손상된 앱"으로 표시될 때** (Gatekeeper quarantine)

```bash
xattr -dr com.apple.quarantine /Applications/Anything.app
```

> Apple Developer ID 인증서 미보유로 ad-hoc 서명만 적용되어 있습니다. 악성코드가 아니며 소스는 이 저장소에서 확인 가능합니다.

---

## 사용법

1. 앱 실행 → 좌측 **"폴더 추가"** → 문서 폴더 선택 (자동 인덱싱 시작)
2. 검색창에 입력 → 결과 클릭으로 미리보기, 더블클릭으로 파일 열기
3. (선택) 검색바 모드를 'Anything'으로 바꿔 질문 → 문서 기반 AI 답변 (설정 → AI 탭에서 API 키 등록)

### 검색 모드

| 모드 | 설명 |
|------|------|
| 키워드 | 기본값. 정확한 단어 매칭 + 아래 검색 연산자 |
| 파일명 | Everything 스타일 파일명 즉시 검색 |
| 스마트 (자연어) | "작년 예산 한글 문서"처럼 말로 조건 조합 — 검색바 토글 |
| Anything (질문) | 문서를 근거로 AI가 답변, 출처 문서·페이지 표시 — 검색바 토글 |

> 설정에서 시맨틱(AI) 검색을 켜면 유사 문서 찾기·유사 중복 탐지와 AI 질의응답의 근거 검색에 의미(벡터) 검색이 활용됩니다.

**검색 연산자** (키워드 모드) — 검색창에서 바로 조합:

| 연산자 | 예 | 동작 |
|--------|-----|------|
| `"구문"` | `"예산 삭감"` | 정확한 구문(인접 순서) |
| `-제외` | `예산 -초안` | 해당 단어 제외 |
| `ext:` | `ext:hwp,pdf` | 확장자 필터 (레거시 그룹 자동 확장) |
| `path:` | `path:인사` | 폴더 경로 필터 |
| `after:` `before:` | `after:2024-06` | 수정일 범위 |
| `~N` | `예산 삭감 ~10` | 형태소 인지 근접 검색 (N토큰 이내) |

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
| AI 질의응답 | **Gemini / OpenAI 호환** | 질문 + 관련 청크만 전송 |
| AI 요약 | **Gemini / OpenAI 호환** | 문서 텍스트 전송 |

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
| AI | Gemini · OpenAI 호환 provider (RAG) |
| OCR | PaddleOCR ONNX + PP-DocLayout 레이아웃 분석 |
| HWP 파싱 | [kordoc](https://www.npmjs.com/package/kordoc) (번들 포함) |

---

## 개발자용

```bash
# `pnpm run bundle-kordoc` 에 필요
git clone https://github.com/chrisryugj/kordoc # Docufinder 폴더 바깥에 클론
cd kordoc
npm install
npm run build
```

```bash
pnpm install          # 의존성 설치
pnpm run download-model  # ONNX 모델 다운로드 (첫 빌드 시)
pnpm run bundle-kordoc # kordoc 및 node.exe 번들링 (첫 빌드 시, kordoc 업데이트 시)
pnpm tauri:dev        # 개발 모드
pnpm tauri:build      # 프로덕션 빌드 (NSIS 설치파일 .exe)
```

**빌드 요구사항**: Windows 10/11 x64 · Node.js 22 LTS + pnpm 10 · Rust 1.92+ · Visual Studio Build Tools 2022


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

배포본에 포함되는 제3자 저작물(PDFium, ONNX Runtime, PaddleOCR, Node.js,
Pretendard, kordoc 및 그 의존성)의 저작권·라이선스 고지는
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 를 참조. 위 BUSL 조건은
제3자 저작물에는 적용되지 않는다.

---

버그 리포트·기능 제안은 [Issues](https://github.com/chrisryugj/Docufinder/issues)에서 환영합니다.

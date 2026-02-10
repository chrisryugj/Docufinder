# Anything (Docufinder) - 빌드 가이드

> 로컬 문서 검색 앱. 이 문서는 git clone 후 빌드까지의 전체 과정을 정리한 것.

## 검증된 환경 (2026-02-10 기준)

| 항목 | 버전 | 비고 |
|------|------|------|
| **OS** | Windows 10/11 x64 | Windows 전용 |
| **Node.js** | v22.20.0 | LTS 권장 |
| **pnpm** | 10.20.0 | `corepack enable && corepack prepare pnpm@10.20.0` |
| **Rust** | 1.92.0 (stable) | `rustup default stable` |
| **Cargo** | 1.92.0 | Rust와 함께 설치됨 |
| **Visual Studio Build Tools** | 2022+ | C++ 빌드 도구 필수 (아래 참고) |

---

## 1단계: 시스템 사전 요구사항

### 1-1. Visual Studio Build Tools (필수)

Rust + Tauri 빌드에 C/C++ 컴파일러가 필요함.

1. [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/ko/visual-cpp-build-tools/) 다운로드
2. 설치 시 **"C++를 사용한 데스크톱 개발"** 워크로드 선택
3. 개별 구성 요소에서 확인:
   - MSVC v143 빌드 도구
   - Windows 10/11 SDK
   - C++ CMake 도구

### 1-2. Rust 설치

```powershell
# rustup 설치 (이미 있으면 스킵)
winget install Rustlang.Rustup

# stable 툴체인 확인
rustup default stable
rustup update

# 버전 확인
rustc --version   # 1.92.0+
cargo --version   # 1.92.0+
```

### 1-3. Node.js + pnpm 설치

```powershell
# Node.js v22 LTS 설치
winget install OpenJS.NodeJS.LTS

# pnpm 활성화 (Node.js 내장 corepack 사용)
corepack enable
corepack prepare pnpm@10.20.0 --activate

# 버전 확인
node --version    # v22.x.x
pnpm --version    # 10.20.0
```

### 1-4. Tauri CLI (Cargo 경유 설치 불필요)

Tauri CLI는 `@tauri-apps/cli` npm 패키지로 포함되어 있음. 별도 `cargo install tauri-cli` 불필요.

---

## 2단계: 프로젝트 클론 & 의존성 설치

```powershell
git clone <repo-url> Docufinder
cd Docufinder

# 프론트엔드 의존성 설치
pnpm install
```

---

## 3단계: ONNX 모델 다운로드 (필수)

시맨틱 검색에 ONNX 모델이 필요함. **인터넷 연결 필수**.

```powershell
pnpm run download-model
```

이 스크립트가 다운로드하는 파일:

| 파일 | 경로 | 크기 | 출처 |
|------|------|------|------|
| `onnxruntime.dll` | `src-tauri/models/multilingual-e5-small/` | ~16MB | GitHub onnxruntime v1.20.1 |
| `model.onnx` (e5-small) | `src-tauri/models/multilingual-e5-small/` | ~90MB | HuggingFace |
| `tokenizer.json` (e5-small) | `src-tauri/models/multilingual-e5-small/` | ~700KB | HuggingFace |
| `model.onnx` (reranker) | `src-tauri/models/ms-marco-MiniLM-L6-v2/` | ~23MB | HuggingFace |
| `tokenizer.json` (reranker) | `src-tauri/models/ms-marco-MiniLM-L6-v2/` | ~700KB | HuggingFace |

> **회사 프록시 환경**: PowerShell `Invoke-WebRequest`가 프록시를 탈 수 있음. 실패 시 수동 다운로드 후 위 경로에 배치.

---

## 4단계: 빌드 & 실행

### 개발 모드

```powershell
pnpm tauri:dev
```

- Vite 개발 서버 (localhost:5173) + Rust 백엔드 동시 실행
- **첫 빌드는 Rust 컴파일 때문에 5~15분** 소요 (이후 증분 빌드로 빠름)
- 핫 리로드: 프론트엔드 변경 즉시 반영, Rust 변경 시 자동 리빌드

### 프로덕션 빌드 (MSI 설치파일)

```powershell
pnpm tauri:build
```

- 빌드 결과: `src-tauri/target/release/bundle/msi/Anything_0.1.0_x64_ko-KR.msi`

---

## 의존성 상세 버전

### 프론트엔드 (package.json)

| 패키지 | 버전 | 용도 |
|--------|------|------|
| `react` | ^19.2.3 | UI 프레임워크 |
| `react-dom` | ^19.2.3 | React DOM 렌더러 |
| `@tauri-apps/api` | ^2.9.1 | Tauri IPC 통신 |
| `@tauri-apps/plugin-dialog` | ^2.6.0 | 네이티브 다이얼로그 |
| `@tauri-apps/plugin-process` | ^2.3.1 | 프로세스 제어 |
| `@tauri-apps/plugin-shell` | ^2.3.4 | 셸 명령 실행 |
| `@tanstack/react-virtual` | ^3.11.2 | 가상 스크롤 |
| `lucide-react` | ^0.469.0 | 아이콘 |
| `typescript` | ^5.9.3 | 타입 체커 |
| `vite` | ^7.3.1 | 번들러 |
| `tailwindcss` | ^4.1.18 | CSS 프레임워크 |
| `@tailwindcss/vite` | ^4.1.18 | Tailwind Vite 플러그인 |
| `@vitejs/plugin-react` | ^5.1.2 | React Vite 플러그인 |
| `@tauri-apps/cli` | ^2.9.6 | Tauri CLI |

### 백엔드 Rust (Cargo.toml)

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| `tauri` | ^2.10 | 앱 프레임워크 |
| `tauri-build` | ^2.5 | 빌드 스크립트 |
| `tauri-plugin-shell` | ^2.3 | 셸 플러그인 |
| `tauri-plugin-dialog` | ^2.6 | 다이얼로그 플러그인 |
| `tauri-plugin-process` | ^2.3 | 프로세스 플러그인 |
| `tauri-plugin-autostart` | ^2.5 | 자동 시작 |
| `tauri-plugin-window-state` | ^2.4 | 윈도우 상태 저장 |
| `rusqlite` | 0.32 (bundled) | SQLite + FTS5 |
| `zip` | 2 | ZIP 파일 읽기 (HWPX/DOCX) |
| `quick-xml` | 0.37 | XML 파싱 |
| `calamine` | 0.26 | Excel 파싱 |
| `pdf-extract` | 0.7 | PDF 텍스트 추출 |
| `ort` | =2.0.0-rc.11 | ONNX Runtime 바인딩 (**버전 고정**) |
| `usearch` | 2.23 | 벡터 인덱스 |
| `tokenizers` | 0.19 | HuggingFace 토크나이저 |
| `ndarray` | 0.16 | 다차원 배열 |
| `lindera` | 2.0 | 한국어 형태소 분석 |
| `notify` | 8 | 파일 시스템 감시 |
| `tokio` | 1 | 비동기 런타임 |
| `rayon` | 1.10 | 병렬 처리 |
| `ureq` | 3 | HTTP 클라이언트 |
| `thiserror` | 2 | 에러 타입 |
| `tracing` | 0.1 | 로깅 |
| `chrono` | 0.4 | 날짜/시간 |

> **주의**: `ort = "=2.0.0-rc.11"`은 **정확한 버전 고정**(= prefix). RC 버전이지만 현재 최신이며, 다른 버전으로 변경 시 빌드 실패 가능.

---

## 흔한 빌드 에러 & 해결

### 1. `error: linker 'link.exe' not found`

**원인**: Visual Studio Build Tools 미설치
**해결**: 1-1 단계 참고, "C++를 사용한 데스크톱 개발" 워크로드 설치

### 2. `error: failed to run custom build command for 'ort'`

**원인**: ONNX Runtime 빌드 의존성 문제
**해결**: `ort`는 `download-binaries` feature로 자동 다운로드함. 프록시 환경이면 환경변수 설정 필요:
```powershell
$env:HTTPS_PROXY = "http://proxy.company.com:8080"
cargo clean
pnpm tauri:dev
```

### 3. `error: failed to run custom build command for 'tokenizers'`

**원인**: `tokenizers` 크레이트의 `onig` feature가 C 컴파일러 필요
**해결**: Visual Studio Build Tools + CMake 설치 확인

### 4. `model.onnx not found` 또는 시맨틱 검색 안됨

**원인**: ONNX 모델 미다운로드
**해결**: `pnpm run download-model` 실행

### 5. `pnpm: command not found`

**해결**:
```powershell
corepack enable
corepack prepare pnpm@10.20.0 --activate
```

### 6. Vite 포트 충돌 (5173)

**원인**: 다른 프로세스가 5173 포트 사용 중
**해결**: 해당 프로세스 종료 후 재실행

### 7. 첫 빌드가 너무 오래 걸림

**정상임**. Rust 첫 컴파일은 모든 크레이트를 빌드하므로 5~15분 소요. `src-tauri/target/` 폴더가 생성되면 이후 증분 빌드로 빠름.

### 8. 회사 프록시 환경에서 다운로드 실패

```powershell
# Rust/Cargo 프록시 설정
$env:HTTPS_PROXY = "http://proxy.company.com:8080"
$env:HTTP_PROXY = "http://proxy.company.com:8080"

# git 프록시 설정
git config --global http.proxy http://proxy.company.com:8080

# PowerShell 다운로드 (모델 스크립트용)
[System.Net.WebRequest]::DefaultWebProxy.Credentials = [System.Net.CredentialCache]::DefaultNetworkCredentials
```

---

## 빠른 검증 체크리스트

```
[ ] rustc --version     → 1.92.0+
[ ] cargo --version     → 1.92.0+
[ ] node --version      → v22.x.x
[ ] pnpm --version      → 10.20.0
[ ] Visual Studio Build Tools 2022 설치됨 (C++ 워크로드)
[ ] git clone 완료
[ ] pnpm install 성공
[ ] pnpm run download-model 성공 (5개 파일 다운로드)
[ ] pnpm tauri:dev 실행 → 앱 창 뜸
```

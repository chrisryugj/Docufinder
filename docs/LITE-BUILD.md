# Anything Lite — 내부망(폐쇄망) 전용 빌드

일반 배포판과 **같은 소스**에서 Cargo feature 하나를 빼고 빌드한 flavor. 목적은 기능 축소가
아니라 **보안솔루션 행위 탐지 표면 제거**다. 내부망 PC 에 설치할 때 IT 부서 검토를 통과시키는
것이 이 빌드의 존재 이유다.

```
online (기본)  : cargo --features default            → Anything
lite  (내부망) : cargo --no-default-features -F custom-protocol → Anything Lite
```

---

## 1. 왜 만들었나 — 실제로 검출됐던 것들

이슈 [#35](https://github.com/chrisryugj/Docufinder/issues/35), [#23](https://github.com/chrisryugj/Docufinder/issues/23) 에서 확인된 실측 사례:

| 증상 | 원인 | 분류 |
|---|---|---|
| 작업관리자에 프로세스가 아예 안 뜸 | OCR 켜면 실행 중 GitHub 에서 `pdfium.dll` 을 내려받아 로드 | dropper (ZombieZERO 계열이 격리) |
| v3.4.3 이후에도 기동 실패 | AppData 에 쓴 PE 를 같은 프로세스가 로드 | dropper 상관 |
| 화면이 안 뜨고 60초 후 진단창 | AhnLab V3 가 `msedgewebview2.exe` 자식 프로세스 생성을 차단 | 자식 프로세스 정책 |

핵심은 **"실행 중인 서명 없는 프로세스가 외부에서 바이너리를 받아 실행/로드한다"** 는 한 문장이다.
행위 기반 엔진은 이 패턴 하나로 격리를 건다. lite 는 그 문장이 성립할 여지를 코드에서 없앤다.

> **남아 있는 근본 문제: 코드 서명 부재.** `tauri.conf.json` 의 `certificateThumbprint` 가
> `null` 이고 CI 에도 Authenticode 서명 단계가 없다. 서명 없는 PE 라는 사실이 아래 모든 항목의
> 심각도를 증폭시킨다. lite 는 탐지 표면을 최소화하지만 **서명을 대신하지는 못한다.**
> OV/EV 코드서명 인증서 도입이 여전히 1순위 과제다.

---

## 2. lite 에서 사라지는 것 (컴파일 단계에서)

| 제거 대상 | 왜 위험한가 | 근거 위치 |
|---|---|---|
| `ureq` HTTP 클라이언트 **전체** | 링크조차 안 되므로 바이너리에 HTTP/TLS 스택·외부 호스트 문자열이 없다 | `Cargo.toml` `online` feature |
| Telegram 오류 리포트 | 서명 없는 exe 가 외부 호스트로 주기적 HTTPS POST → **C2 비컨과 동형** | `commands/telemetry.rs` |
| 자동 업데이트 (updater·process 플러그인) | `dialog:false` + `installMode:passive` = 사용자 확인 없이 설치본을 받아 실행. dropper 휴리스틱 정면 | `lib.rs`, `tauri.lite.conf.json` |
| GitHub Releases API 조회 | 폐쇄망에서 6시간마다 실패하는 아웃바운드가 IDS 로그에 알람으로 쌓임 | `commands/file.rs` |
| 모델/DLL 런타임 다운로드 | 이슈 #35 의 직접 원인 | `model_downloader` 모듈째 |
| AppData 로 PE 복사 (`seed_bundled_models`) | "프로세스가 쓴 PE 를 그 프로세스가 로드" 상관 | `lib.rs` setup |
| LLM 호출 (Gemini · OpenAI 호환) | 문서 본문이 외부로 나가는 유일한 경로 | `llm/` 모듈째 |
| `powershell.exe -EncodedCommand` | 인코딩된 PowerShell 은 거의 모든 EDR 의 고신뢰 악성 룰 | `utils/disk_info.rs` |
| `rundll32.exe url.dll,FileProtocolHandler` | 대표 LOLBin (MITRE T1218.011) → `explorer.exe` 로 대체 | `commands/file.rs` |
| `LoadLibraryExW(LOAD_WITH_ALTERED_SEARCH_PATH)` 진단 | DLL 사이드로딩 탐지 룰의 정면 대상 | `utils/dll_diag.rs` |
| kordoc 수식 OCR 네이티브 deps 4종 | 서명 없는 `.node`/`.dll` 수십 개가 설치 폴더에 깔림 | `scripts/bundle-kordoc.ps1 -Lite` |
| `--ocr` / `--formula-ocr` CLI 플래그 | kordoc 자식이 HuggingFace 에서 모델을 직접 받음 (Rust 쪽 offline 스위치가 안 닿는 경로) | `parsers/kordoc.rs` `call_kordoc_sync` |

번들에서 빠지는 자산: `onnxruntime.dll`, `pdfium.dll`, `paddleocr/*`.
→ **동적 DLL 로드(dlopen) 시도 자체가 프로세스 생애에 한 번도 일어나지 않는다.**

### 설정 강제 (되살아나기 방지)

`commands/settings.rs` 의 `sanitize_for_lite` 가 **로드·저장 양쪽**에서 아래를 강제한다.
online 빌드의 `settings.json` 을 그대로 들고 오거나 파일을 손으로 고쳐도 기능이 되살아나지 않는다.

```
semantic_search_enabled = false      ocr_enabled            = false
ocr_layout_enabled      = false      formula_ocr_enabled    = false
ai_enabled              = false      error_reporting_enabled = false
search_mode: Semantic|Hybrid → Keyword   (Filename 은 로컬이라 유지)
```

lite 는 identifier 가 `com.anything.lite` 로 갈린다. 같은 PC 에 일반판이 깔려 있어도
설정·DB·로그가 섞이지 않는다 — 안 그러면 lite 의 강제 off 가 일반판 설정을 덮어쓴다.

---

## 3. lite 에 남는 기능

검색기 본체는 그대로다.

- 전문 검색 (SQLite FTS5 + Lindera 한국어 형태소, 근접 검색 `~N`, 검색 연산자)
- 파일명 검색 (Everything 스타일 인메모리 캐시)
- HWP/HWPX/DOCX/XLSX/PDF/이미지·`.eml` 파싱, 미리보기(kordoc 마크다운 + rhwp 원본 레이아웃 SVG)
- 실시간 감시 + 증분 인덱싱, 문서 비교, 버전 계보(lineage), 태그·북마크, 중복 찾기, CSV/MD 내보내기

빠지는 기능: **시맨틱/하이브리드 검색 · OCR · AI 질의응답/요약 · 자동 업데이트 · 오류 자동 리포트.**

---

## 4. 남아 있는 프로세스 생성 — IT 부서 허용 요청용

lite 도 아래 두 가지는 필요하다. 검출되면 예외 등록을 요청할 대상이다.

| 프로세스 | 용도 | 비고 |
|---|---|---|
| `node.exe` (설치 폴더 번들) | kordoc 사이드카 — HWP/HWPX/DOCX/PDF → 마크다운 변환. 이게 없으면 HWP 인덱싱 전수 실패 | 자식은 번들된 `cli.js` 만 실행. **네트워크 호출 없음** |
| `msedgewebview2.exe` | Tauri 의 UI 렌더러 (WebView2) | 앱 UI 그 자체. 차단되면 창이 안 뜬다 (이슈 #23) |
| `explorer.exe` | 파일/폴더/URL 열기 | 사용자가 결과를 클릭할 때만 |

그 외 Win32 사용: 매핑 드라이브→UNC 해석용 `HKCU\Network` **읽기**(쓰기 없음), 자식 정리용
Job Object. 지속화(Run 키·서비스 등록) 없음, 프로세스 인젝션 없음.

---

## 5. 빌드

```powershell
# 사전: kordoc 소스가 있어야 한다 (KORDOC_DIR 또는 알려진 경로)
pnpm install
pnpm run tauri:build:lite
```

`tauri:build:lite` 가 하는 일:
1. `bundle-kordoc.ps1 -Lite` — node.exe + kordoc dist + 최소 node_modules (수식 OCR deps 제외)
2. `tauri build --config src-tauri/tauri.lite.conf.json --no-default-features --features custom-protocol`
   - `beforeBuildCommand` 가 `pnpm build:lite` (= `VITE_LITE=1`) 를 돌려 프론트 UI 도 함께 게이팅

`download-model` / `download-vcredist` 는 돌리지 않는다 — lite 가 번들하지 않는 자산이다.

개발 모드: `pnpm run tauri:dev:lite`

### 프론트 flavor 플래그

`src/utils/buildFlavor.ts` 의 `IS_LITE` 하나가 유일한 출처다. Vite 가 빌드 시점 상수로 접어
죽은 분기를 트리셰이킹한다. **프론트와 백엔드 flavor 는 반드시 짝을 맞춰야 한다** —
`tauri:build:lite` 스크립트가 그 짝을 보장하므로 개별 명령을 손으로 조합하지 말 것.

---

## 6. 검증 (배포 전 필수)

두 flavor 가 모두 깨지지 않았는지 확인한다.

```bash
cd src-tauri
cargo clippy --all-targets -- -D warnings                                   # online
TAURI_CONFIG="$(cat tauri.lite.conf.json)" \
  cargo clippy --no-default-features --features custom-protocol --all-targets -- -D warnings
```

> lite 를 `cargo` 로 직접 검사할 땐 `TAURI_CONFIG` 를 넘겨야 한다. 안 넘기면 build script 가
> 기본 `tauri.conf.json` 을 읽어 `updater:default` 권한을 찾다가 실패한다.

IT 부서 제출용 증빙 — 빌드 산출물에 네트워크 흔적이 없음을 직접 보여줄 수 있다.
릴리스마다 CI 가 자동으로 같은 검사를 돌리며, 하나라도 걸리면 배포가 실패한다.

```powershell
# lite 는 아무것도 안 나오고, 같은 검사에서 online 빌드는 4종 전부 검출된다(실측 대조군).
$exe  = ".\docufinder.exe"   # 설치 폴더의 실행 파일
$text = [System.Text.Encoding]::GetEncoding(28591).GetString([System.IO.File]::ReadAllBytes($exe))
@('api.telegram.org','huggingface.co','api.github.com','generativelanguage.googleapis.com') |
  ForEach-Object { "{0,-36} {1}" -f $_, $(if ($text.Contains($_)) { "검출" } else { "없음" }) }
```

> `Select-String -Encoding Byte` 는 Windows PowerShell 5.1 에서만 동작한다 — PowerShell 7
> 에서는 그 파라미터가 없어 실패하므로 위 방식을 쓴다.

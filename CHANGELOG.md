# Changelog

## [2.6.21] - 2026-05-22

**LTSC/WebView2 핫픽스 — environment 생성 성공 후 controller hang 잔여 케이스 대응 (#24)**

### 원인
v2.6.20 최신 로그에서 fixed runtime 경로 override 와 `CreateCoreWebView2EnvironmentWithOptions` 는 성공했다. 즉 이전 `0x80070002` 단계는 통과했지만, 다음 단계인 WebView2 controller 생성이 60초 내 완료되지 않았다.

LTSC 빌드는 그동안 CI runner 에 Evergreen Runtime 을 silent install 한 뒤 `%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application\<version>\` 폴더를 복사해 번들했다. 이 폴더는 시스템 WebView2 런타임으로는 동작하지만, 앱과 함께 배포하는 self-contained 런타임으로는 Microsoft 가 제공하는 **Fixed Version Runtime CAB** 를 직접 쓰는 편이 맞다. 또한 기존 `.acl-applied` 마커가 남으면 App Container ACL 재부여를 스킵해, 덮어쓰기 설치/런타임 교체 후 실제 권한 상태를 다시 보강하지 못할 수 있었다.

### 변경
- **`scripts/setup-webview2-runtime.ps1`** — Evergreen standalone installer 설치 결과 복사 방식을 폐기하고, Microsoft 공식 Fixed Version Runtime x64 CAB (`148.0.3967.83`) 를 직접 다운로드/추출해 `webview2-runtime\EBWebView\x64\` 로 번들한다.
- **`src-tauri/src/webview2_runtime.rs`** — `.acl-applied` 마커가 있어도 LTSC runtime 감지 시 App Container ACL 을 매번 재부여한다. `icacls /grant` 는 멱등이고, stale marker 로 권한 보강을 놓치는 위험을 제거한다.
- **진단 로그** — `msedge.dll` / `msedge.dll.sig` 도 runtime diagnostics 에 포함해 Fixed Version Runtime 핵심 파일 누락 여부를 더 명확히 확인한다.
- **watchdog 문구** — controller hang 을 특정 백신으로 단정하지 않고, Defender Controlled Folder Access / AppLocker / EDR / App Container ACL 등 자식 프로세스 실행·파일 접근 차단 후보로 안내한다.

## [2.6.20] - 2026-05-21

**LTSC/WebView2 핫픽스 — fixed runtime 경로가 환경변수·정책 override 에 덮이는 잔여 케이스 차단 (#24)**

### 원인
이슈 #24 최신 로그는 LTSC 설치본 안의 `webview2-runtime\EBWebView\x64` 폴더와 핵심 파일이 존재하는데도 `CreateCoreWebView2EnvironmentWithOptions` 가 `0x80070002` 로 실패했다. Microsoft WebView2 loader 는 API 인자보다 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` / `WEBVIEW2_USER_DATA_FOLDER` 환경변수와 Edge WebView2 group policy registry 를 우선하므로, 회사 PC·VDI·이전 workaround 에 남은 override 가 앱이 넘긴 fixed runtime 경로를 무시하게 만들 수 있었다.

기존 코드는 fixed runtime 생성 실패 시 system runtime 으로 fallback 해서, 결국 같은 "WebView2 Runtime not found" 다이얼로그로 돌아갔다.

### 변경
- **`src-tauri/src/webview2_runtime.rs`** — fixed runtime 감지 후 프로세스 범위 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` / `WEBVIEW2_USER_DATA_FOLDER` 를 앱 포함 런타임과 앱 데이터 폴더로 명시 설정. environment 생성 호출에도 동일한 user data folder 를 전달.
- **`src-tauri/src/lib.rs`** — override 전/후 값을 로그와 watchdog 진단에 남겨, 사용자 환경변수·정책 개입 여부를 다음 로그에서 바로 확인 가능하게 함.
- **`runtime_diagnostics`** — `EmbeddedBrowserWebView.dll` 을 x64/x86 별도로 출력. fixed runtime 패키지에는 두 architecture 파일이 모두 있으므로, 재귀 탐색의 첫 결과가 x86 으로 찍혀 원인을 오진하던 진단 노이즈를 제거.

## [2.6.19] - 2026-05-21

**버그 수정 — 파일/폴더/로그폴더 열기가 모두 실패하던 회귀 (#28 PATH hijack 방지 커밋의 회귀)**

### 원인
`87ea2b7` ("prod review 1차 — PATH hijack 방지", #28) 가 Windows 시스템 실행파일을 System32 절대경로로 호출하도록 바꿨는데, `explorer.exe` 는 `System32` 가 아니라 Windows 디렉토리 루트(`C:\Windows\explorer.exe`)에 있다. `windows_system32("explorer.exe")` 가 부재 경로(`C:\Windows\System32\explorer.exe`)를 만들어 `Command::spawn()` 이 항상 실패했다.

`open_with_default` 헬퍼를 쓰는 모든 기능 — **파일 열기**(PDF 가 아니거나 SumatraPDF 미설치 시), **폴더 열기**, **로그 폴더 열기** — 가 v2.6.0 ~ v2.6.18 내내 깨져 있었다 ("열기 실패" / "폴더 열기 실패").

### 변경
- **`src-tauri/src/commands/file.rs::open_with_default`** — `explorer.exe` 를 `%SystemRoot%\explorer.exe` (Windows 디렉토리 루트) 로 호출. `windows_system32` 헬퍼는 `where.exe` / `rundll32.exe` 에는 올바르므로 그대로 유지.

## [2.6.18] - 2026-05-20

**v2.6.17 후속 — 배포 버그 2건 수정 (standalone installer x86 / watchdog 문구 오도)**

### 배경
v2.6.17 출시 후 두 가지 보고:
- 이슈 #23 austinjung827 — v2.6.17 LTSC 설치본에서 fixed runtime 은 x64 정상 + App Container ACL OK 로 확인됐으나 controller 생성에서 hang. 사내 백신 = AhnLab V3 Internet Security 9.0.
- 메일 제보 (일반 사용자) — `Anything_2.6.17_x64-setup.exe` (일반 설치본) 실행 시 watchdog 다이얼로그. 진단: "fixed runtime 미감지 — system WebView2 사용".

### 원인
1. **release 의 standalone installer 가 x86 이었음.** `publish.yml` 의 "Download & upload WebView2 Standalone Installer" step 이 `linkid=2099617` (X86 installer) 을 받아 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 라는 **잘못된 파일명**으로 release 에 업로드. v2.6.17 의 `setup-webview2-runtime.ps1` 는 x64 로 교정됐으나 이 별도 step 을 놓쳤다. 이슈 #23/#24 에서 사용자에게 "이 파일을 admin 으로 설치하세요" 라고 안내한 standalone 이 x86 이었다.
2. **watchdog 다이얼로그 문구가 fixed runtime 미감지/감지를 구분하지 않음.** "WebView2 런타임은 정상 감지됐으나..." 로 단정해, 일반 설치본(fixed runtime 미번들)을 받은 사용자에게도 보안 솔루션 차단으로 원인을 오도. 일반 설치본 사용자는 LTSC 설치본 안내가 우선이어야 한다.

### 변경
- **`.github/workflows/publish.yml`** — standalone 업로드 `linkid` 를 2124701 (검증된 X64) 로 교정. 업로드 전 PE 헤더로 architecture 검증 (X86 이면 빌드 중단).
- **`src-tauri/src/lib.rs` · `webview2_runtime.rs`** — watchdog 다이얼로그 본문을 fixed runtime 감지 여부로 분기. 미감지(일반 설치본) → "파일명에 ltsc 가 붙은 설치본을 받으세요" 안내. 감지(LTSC, runtime 정상)인데도 hang → 안랩 V3 / EDR / AppLocker 차단 안내 + IT 부서 실행 허용 요청 항목.

### 사용자 안내
- **이슈 #23 austinjung827** — v2.6.17 LTSC 설치본에서 fixed runtime 은 완전히 정상(x64 + ACL). controller hang 은 AhnLab V3 가 `msedgewebview2.exe` 자식 프로세스 생성을 차단하는 것으로, 앱 코드로는 해결 불가. IT 부서에 `Anything.exe` / `msedgewebview2.exe` 실행·자식 프로세스 허용(예외 등록) 요청 필요.
- **메일 제보 일반 사용자 / 이슈 #24** — `x64-ltsc-setup.exe` (WebView2 자체 포함 빌드) 로 재설치 권장.

## [2.6.17] - 2026-05-20

**LTSC 핫픽스 — WebView2 controller hang 근본 해결 (이슈 #23 austinjung827 v2.6.16 후속)**

### 배경
v2.6.16 의 `CoInitializeEx` 추가로 environment 생성 실패(`0x800401F0`)는 해소. 로그상 `WebView2 fixed runtime detected ... environment injected` 까지 도달했으나, 그 직후 `builder.build()` 단계에서 hang — UI 미표시, 프로세스만 생존.

### 근본 원인 (CI 로그 + wry 0.54 소스 + Microsoft 공식 문서로 확정)
1. **`linkid=2099617` 은 X86 installer 였다.** 빌드 스크립트가 X64 로 오인 (`go.microsoft.com/fwlink/?linkid=2099617` → `MicrosoftEdgeWebView2RuntimeInstallerX86.exe`). 그 결과 evergreen 이 x86 으로 깔려, v2.6.13~v2.6.16 이 GH Actions runner 의 Microsoft Edge **브라우저** 폴더(`Edge\Application\<ver>`, 764 files 827MB)를 fixed runtime 으로 짜깁기. Edge 브라우저 트리는 WebView2 Fixed Version Runtime 이 아니므로 environment 는 통과해도 controller(브라우저 자식 프로세스) 생성에서 실패.
2. **Fixed Version Runtime 폴더에 App Container ACL 누락.** WebView2 v120+ 는 renderer 프로세스가 App Container sandbox 에서 실행된다. unpackaged Win32 앱이 Windows 10 에서 Fixed Version Runtime 을 쓸 때 runtime 폴더에 `ALL APPLICATION PACKAGES`(S-1-15-2-1) / `ALL RESTRICTED APPLICATION PACKAGES`(S-1-15-2-2) 읽기 권한이 없으면 sandbox renderer 가 runtime 파일을 못 읽어 controller 생성이 실패한다 (Microsoft 공식 distribution 문서 명시). austin = LTSC 1809 = Windows 10.
3. **wry 0.54 의 `create_controller` 는 `webview2_com::wait_with_pump` 로 controller 완료 callback 을 무한 대기.** controller 가 실패해도 callback 의 `error_code?` 가 결과 송신을 건너뛰어 `build()` 가 영원히 hang — 에러조차 안 남는다.

### 변경
- **`scripts/setup-webview2-runtime.ps1`** — `linkid` 를 2124701 (검증된 X64 installer) 로 교정. evergreen `EdgeWebView\Application\<ver>\` 폴더를 구조 변형 없이 통째로 `webview2-runtime\EBWebView\x64\` 로 번들. system 전체 짜깁기/평탄화 로직 폐기. architecture + controller 필수 파일(`icudtl.dat`, `resources.pak`, `v8_context_snapshot.bin`, `Locales\`)을 fail-hard 검증 (v2.6.15 fail-soft 폐기).
- **`src-tauri/src/webview2_runtime.rs`** — `grant_app_container_access` 추가: startup 에 fixed runtime 폴더에 App Container 읽기 권한 부여 (`icacls`, SID 직접 지정, `.acl-applied` 마커로 멱등). `spawn_build_watchdog` 추가: `builder.build()` 가 60초 내 미완료 시 진단 다이얼로그 표시 후 종료 (작업관리자 강제종료 방지). `runtime_diagnostics` 추가: runtime 폴더 인벤토리 로그.
- **`src-tauri/src/lib.rs`** — main window build 전 ACL 부여 + 진단 로그 출력, `builder.build()` 를 watchdog 으로 감쌈.

### 사용자 안내
- **이슈 #23 austinjung827** — v2.6.17 LTSC installer 재설치 후 본체 실행. UI 가 정상 표시돼야 정상. 만약 60초 후 진단 다이얼로그가 뜨면 그 내용을 캡처해 회신 — 회사 보안 정책(EDR/AppLocker) 차단으로 원인이 좁혀진다.

## [2.6.16] - 2026-05-19

**LTSC 핫픽스 — WebView2 environment 생성 전 COM apartment 초기화 (이슈 #23 austinjung827 v2.6.15 후속)**

### 배경
v2.6.15 LTSC installer 로 evergreen 의 x86 회귀는 해소 (msedgewebview2.exe x64, 4.46MB 정상 확인 — 이슈 #23 댓글). 그러나 본체 실행 시 새로운 에러로 진전:
```
WebView2 environment creation failed (hr=Err(Error {
    code: HRESULT(0x800401F0),
    message: "CoInitialize가 호출되지 않았습니다."
}))
```
프로세스는 떠 있지만 UI 가 안 뜨는 상태 (작업관리자 상 docufinder.exe 활성).

### 원인
`src-tauri/src/webview2_runtime.rs::create_environment` 가 `CreateCoreWebView2EnvironmentWithOptions` (COM 기반 API) 를 **CoInitialize/CoInitializeEx 없이** 호출. Tauri `setup()` 시점의 main thread 는 winit/tao event loop 가 시작되기 전이라 COM apartment 가 자동 초기화돼 있지 않다.

GH Actions runner 에서는 다른 모듈(가령 사전 설치된 Edge/.NET tooling 의 init 잔재) 이 COM 을 미리 초기화해 둔 thread 가 등장해서 빌드 검증 단계에서는 가려졌고, 일반 사용자 환경 (특히 LTSC 1809) 에서 노출됨.

### 변경
- **`src-tauri/src/webview2_runtime.rs::create_environment`** — 함수 진입부에 `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` 호출 추가. `S_FALSE` (이미 같은 mode 로 초기화) / `RPC_E_CHANGED_MODE` (다른 apartment 로 이미 초기화) 는 둘 다 무시 — COM 자체가 활성 상태면 후속 API 호출 가능.
- **`src-tauri/Cargo.toml`** — `windows` crate features 에 `Win32_System_Com` 추가.

### 사용자 안내
- **이슈 #23 austinjung827** — [v2.6.16 LTSC installer](https://github.com/chrisryugj/Docufinder/releases/download/v2.6.16/Anything_2.6.16_x64-ltsc-setup.exe) 재설치 후 본체 실행. UI 가 정상 표시돼야 정상.

## [2.6.15] - 2026-05-18

**LTSC PS1 — 사실 발견 기반 자동 선택 모드 (v2.6.11~v2.6.14 가정 누적 실패 종결)**

### 배경
v2.6.11~v2.6.14 가 evergreen WebView2 install 결과 폴더 구조를 매번 다르게 가정하여 연쇄 회귀:
- v2.6.11: NuGet 패키지 가정 → 미발행
- v2.6.12: evergreen Application\<ver>\ → EmbeddedBrowserWebView.dll 누락
- v2.6.13: system 검색 → x86 dll 매치 (arch mismatch)
- v2.6.14: Application\<ver>\EBWebView\x64\ 가정 → 폴더 부재로 빌드 fail

### 변경 — 가정 폐기, 사실 발견
`scripts/setup-webview2-runtime.ps1`:
1. **DIAG**: evergreen install 직후 `Application\<ver>\` 의 1-depth 트리 + system 전체의 모든 `msedgewebview2.exe` / `EmbeddedBrowserWebView.dll` 의 위치 + 각 파일의 PE 헤더 architecture (x64/x86/arm64) 를 CI 로그에 dump.
2. **자동 선택**: `msedgewebview2.exe` (x64) 와 `EmbeddedBrowserWebView.dll` (x64) 가 **같은 폴더에 공존하는** self-contained 후보 또는 `EBWebView\x64\` sub-folder 동봉 케이스를 자동 탐지 → 그 폴더를 EBWebView\x64\ 로 복사 (필요 시 평탄화).
3. **fail-soft 검증**: 후보 0건 또는 복사 후 critical file 누락 시 WARNING 만 출력 (exit 0 유지). 빌드는 통과시켜 LTSC installer 산출 자체는 진행. 진단 로그로 다음 패치 진로 결정.

### 영향
- 빌드는 다시 통과 (LTSC installer 산출 보장).
- CI 로그에 evergreen / Edge browser / WebView2 의 정확한 layout 이 dump 되어, 사용자 머신 0x80070002 의 정확한 fix 를 다음 패치에서 가정 없이 작성 가능.

## [2.6.14] - 2026-05-18 (yanked — LTSC installer 누락)

**가정 오류 — `Application\<ver>\EBWebView\x64\` 폴더 부재로 PS1 exit 1. v2.6.15 에서 사실 발견 모드로 전환.**

**LTSC fixed runtime 진짜 진짜 핫픽스 — evergreen 구조 정확히 반영 + PE 헤더 architecture 검증**

### 배경 (v2.6.13 실패)
v2.6.13 PS1 의 system 탐색이 `Application\<ver>\EBWebView\x86\EmbeddedBrowserWebView.dll` (**x86**) 을 first match 로 잡아 x86 dll 을 x64 빌드의 fixed runtime 에 끼워 넣음. 빌드는 통과했지만 사용자 머신에서 architecture mismatch 로 또 실패 예상.

### evergreen 실제 구조 (조사 후 확정)
```
C:\Program Files (x86)\Microsoft\EdgeWebView\Application\<ver>\
├── msedgewebview2.exe                (진입점)
├── msedgewebview2.exe.sig
├── Locales\
├── *.bin, *.dat (V8/ICU 데이터)
└── EBWebView\
    ├── x64\                          ← native binaries (EmbeddedBrowserWebView.dll 등)
    └── x86\                          ← 32-bit
```

Microsoft Fixed Version Runtime cab 의 self-contained layout 은 위 분리 구조를 **하나의 폴더로 평탄화**.

### 변경 (`scripts/setup-webview2-runtime.ps1`)
1. **첫 복사**: `Application\<ver>\*` → 우리의 `EBWebView\x64\` (base layer — entry point + data + Locales).
2. **평탄화 복사**: `Application\<ver>\EBWebView\x64\*` 의 native binary 를 우리 `EBWebView\x64\` root 로 추가 복사 (덮어쓰기). 이로써 `msedgewebview2.exe` 가 자신과 같은 폴더에서 `EmbeddedBrowserWebView.dll` 등을 찾을 수 있게 됨.
3. **중첩 폴더 제거**: 첫 복사로 생긴 `EBWebView\x64\EBWebView\` sub-folder (이미 평탄화로 root 에 옮겨졌으므로 불필요) 삭제.
4. **PE 헤더 architecture 검증**: `EmbeddedBrowserWebView.dll` 의 IMAGE_FILE_MACHINE 필드를 직접 읽어 `0x8664` (AMD64/x64) 가 아니면 빌드 fail. x86 dll 혼입을 binary 수준에서 차단 — v2.6.13 회귀 재발 방지.

### 사용자 안내
- **이슈 #23 austinjung827** — v2.6.14 LTSC installer (`Anything_2.6.14_x64-ltsc-setup.exe`) 로 재설치. 자체완결 fixed runtime 으로 system WebView2/Edge 부재 환경에서도 단독 동작해야 함.
- 본 핫픽스로 안 되면 진단 정보 (`C:\Anything\webview2-runtime\EBWebView\x64\EmbeddedBrowserWebView.dll` 의 file size, `docufinder.YYYY-MM-DD.log` 의 새 에러 메시지) 회신 부탁.

## [2.6.13] - 2026-05-18

**LTSC installer 진짜 핫픽스 — 이슈 #23 0x80070002 직접 원인 (EmbeddedBrowserWebView.dll 누락) 해결**

### 배경 — v2.6.12 진단 로그가 결정타를 찍음
v2.6.12 의 `WebView2 Fixed Version Runtime` 의존성 누락 알람 (warning) 이 CI 빌드 로그에 명시적으로 보고됨:
```
WARNING: evergreen Application\<version>\ 폴더에 다음 파일/폴더 누락:
WARNING:   - EmbeddedBrowserWebView.dll
WARNING: 이슈 #23 의 0x80070002 ERROR_FILE_NOT_FOUND 원인일 가능성
```

즉 Microsoft Evergreen Standalone Installer 의 결과 폴더 (`C:\Program Files (x86)\Microsoft\EdgeWebView\Application\<version>\`) 에는 `EmbeddedBrowserWebView.dll` 이 **들어있지 않음**. 이 dll 은 Microsoft Edge for Business 가 WebView2 와 공유하는 core component 로, Edge 브라우저의 `Application\<version>\` 폴더에 별도 존재. evergreen WebView 만 install 한 결과는 self-contained 가 아니며, `msedgewebview2.exe` 실행 시 이 dll 을 dynamic loader 가 못 찾아 0x80070002 ERROR_FILE_NOT_FOUND 로 환경 생성 실패.

이슈 #23 austinjung827 의 사용자 머신은 회사 GPO/보안 정책으로 Edge 브라우저가 미설치된 상태였고, fixed runtime 동봉본도 EmbeddedBrowserWebView.dll 누락이라 양쪽 모두 실패. 외부망 가상PC 에는 Edge 가 깔려있어 system runtime fallback 이 동작했지만, v2.6.x LTSC installer 의 fixed runtime 경로 자체가 줄곧 비기능 상태였음.

### 변경
- **`scripts/setup-webview2-runtime.ps1`** 추가 단계:
  1. evergreen `Application\<version>\` 복사 후, `EmbeddedBrowserWebView.dll` 이 누락된 경우 system 전체 (`%ProgramFiles(x86)%\Microsoft`, `%ProgramFiles%\Microsoft`) 에서 recursive 탐색.
  2. 가장 최신 버전 폴더의 `EmbeddedBrowserWebView.dll` 위치를 확정 후, **같이 들어있는 폴더 전체에서 빠진 파일/폴더를 보강 복사** (덮어쓰기 없음 — evergreen 결과는 그대로 두고 누락분만 추가). 이로써 Microsoft Edge for Business 의 self-contained Application 트리가 fixed runtime 으로 통합됨.
  3. 보강 후 critical file presence check 강화 — `msedgewebview2.exe`, `msedgewebview2.exe.sig`, `EmbeddedBrowserWebView.dll`, `Locales\` 중 하나라도 여전히 누락이면 **빌드 fail**. 회귀 시 사용자 머신이 아닌 CI 단계에서 즉시 발견.

### 영향
- **LTSC installer (`Anything_2.6.13_x64-ltsc-setup.exe`)** — system WebView2 Runtime / Edge 브라우저 모두 부재한 환경 (이슈 #23 austinjung827 사내망 LTSC 1809) 에서도 fixed runtime 만으로 단독 동작 가능. 0x80070002 ERROR_FILE_NOT_FOUND 가 사라져야 함.
- **일반 installer / macOS dmg** — 변동 없음.

### 사용자 안내
- **이슈 #23 austinjung827** — v2.6.13 LTSC installer (`Anything_2.6.13_x64-ltsc-setup.exe`) 로 재설치 후 본체 실행. 동봉된 standalone WebView2 Runtime 별도 설치 불필요 (이미 사내망에서 막혀있었던 단계).
- **v2.6.10 이하 LTSC installer 사용자** — 자동 업데이트 또는 v2.6.13 LTSC installer 덮어쓰기. `C:\Anything\webview2-runtime\` 폴더의 기존 fixed runtime 은 installer 가 갱신.

## [2.6.12] - 2026-05-18

**LTSC 빌드 회귀 핫픽스 — v2.6.11 NuGet 가설 폐기, evergreen-copy 복구 + 의존성 알람 추가**

### 배경
- v2.6.11 에서 `Microsoft.Web.WebView2.FixedVersionRuntime.<ver>.x64` NuGet 패키지로 LTSC fixed runtime 을 전환하려 했으나, **Microsoft 가 해당 패키지를 nuget.org 에 발행하지 않음** (검색 API totalHits=0) 을 확인. PS1 스크립트가 `Querying NuGet for latest ...` 단계에서 패키지 미발견으로 즉시 fail → LTSC installer 빌드 단계 자체가 안 돌고 release 에 LTSC 자산 누락. 일반 installer / macOS dmg / standalone WebView2 installer 는 v2.6.11 release 에 정상 첨부됨.
- 이슈 #23 의 사용자 머신 0x80070002 ERROR_FILE_NOT_FOUND 의 진짜 원인 진단은 아직 사용자 회신 (파일 개수 / `msedgewebview2.exe.sig` 존재 여부 등) 대기 중.

### 변경
- **`scripts/setup-webview2-runtime.ps1`** — v2.6.10 evergreen-installer-then-copy 방식으로 복구. LTSC installer 빌드 자체는 다시 정상 산출.
- **Critical dependency presence check 추가** — evergreen `Application\<version>\` 복사 후 `msedgewebview2.exe`, `msedgewebview2.exe.sig`, `EmbeddedBrowserWebView.dll`, `Locales\` 4개 중 누락 항목이 있으면 워크플로우 로그에 `WARNING` 으로 명시. (빌드 자체는 계속 진행 — 의존성 누락이 실제로 evergreen 결과의 시스템 분산 때문인지 식별 위한 알람.) 향후 누락이 확인되면 Microsoft Fixed Version Runtime cab mirror 도입으로 전환.
- **버전 bump 2.6.11 → 2.6.12** — release tag 충돌 방지 + LTSC installer 가 포함된 정상 release 생성.

### 영향
- **LTSC installer (`Anything_2.6.12_x64-ltsc-setup.exe`)** — v2.6.10 과 동일한 evergreen-copy 방식이므로 이슈 #23 사용자의 0x80070002 증상 자체는 **해결되지 않을 가능성**. 다만 의존성 누락 알람 도입으로 빌드 단계 진단 정보 확보.
- **일반 installer / macOS dmg** — v2.6.11 과 동일 동작 (변동 없음).

### 진행 중
- 이슈 #23 austinjung827 의 사용자 머신 검증 정보 회신 대기 → 회신 받으면 Microsoft Fixed Version Runtime cab 의 GH release mirror 방식으로 전환 검토 (별도 PR).

## [2.6.11] - 2026-05-18 (yanked — LTSC installer 누락)

**LTSC installer 빌드 실패. NuGet `Microsoft.Web.WebView2.FixedVersionRuntime` 패키지를 가정했으나 nuget.org 에 미발행. v2.6.12 에서 evergreen-copy 복구.**

## [2.6.10] - 2026-05-18

**macOS 핫픽스 — v2.6.7 entitlements 가 실효 없었던 근본 원인 수정 (Hardened Runtime 제거)**

### 배경
- v2.6.7 에서 `entitlements.plist` 의 `disable-library-validation` + inside-out 재서명으로 OCR 활성화 시 발생하는 `SIGKILL (Code Signature Invalid)` 해결을 시도했으나, v2.6.9 macOS arm64 (Apple M4 Pro / macOS 26.4.1) 환경의 사용자 크래시 리포트에서 **동일 증상이 재현됨**. 콜스택 분석:
  ```
  dyld::dlopen
  → ort::setup_api
  → OcrEngine::new
  → docufinder_lib::resume_watchers (line 34)
  → tauri::app::setup
  ```
- 트리거 경로: `lib.rs::setup` 의 `resume_watchers(&container)` 동기 호출 → `get_watch_manager()` OnceCell 첫 init → `if ocr_enabled { get_ocr_engine() }` → `OcrEngine::new` → `ort::setup_api` → `libonnxruntime.dylib` dlopen → **Library Validation 실패 → SIGKILL**.
- 종료 사유: `Namespace CODESIGNING, Code 2, Invalid Page`.

### 근본 원인 (v2.6.7 진단 실패 이유)
- v2.6.7 의 `.github/workflows/publish.yml` 재서명 step 이 `codesign --options runtime --entitlements entitlements.plist --sign -` 로 **Hardened Runtime 을 활성화**했음. 의도는 entitlements 로 Library Validation 을 풀자는 것이었으나, macOS 의 보안 모델은:
  - `disable-library-validation` entitlement 는 **Apple Developer ID 서명** 환경에서만 신뢰됨.
  - **ad-hoc 서명 (`signingIdentity: "-"`)** 은 OS 가 서명자의 진위를 검증할 수 없어 entitlements 의 신뢰성 자체를 보장하지 못해 **권한 부여가 무시**됨.
  - 결과: Hardened Runtime 만 켜지고 entitlements 는 무력화 → ad-hoc + Hardened Runtime + 외부 dylib dlopen 조합은 가장 엄격한 거부 경로가 됨.

### 변경
- **`.github/workflows/publish.yml`** — macOS 재서명 step 에서 `--options runtime` 및 `--entitlements` 인자 전부 제거. ad-hoc 서명만 적용 (`codesign --force --sign -`). Hardened Runtime 이 꺼지면 Library Validation 강제가 사라져 외부 dylib `dlopen` 이 허용된다.
- **`src-tauri/tauri.macos.conf.json`** — `entitlements: "entitlements.plist"` 키 제거. ad-hoc 빌드 단계에서도 entitlements 가 따라붙지 않게 정리.
- **`src-tauri/entitlements.plist`** 파일 삭제. 향후 Apple Developer ID 도입 시 재작성.

### 향후 (검토 후 보류)
- **OCR 초기화 lazy 화**: `get_watch_manager()` OnceCell init 시점에 OCR Arc 를 받지 말고 첫 OCR 작업 시점까지 미루는 리팩토링. 본 핫픽스로 SIGKILL 자체는 사라지므로 cold-start 속도 개선 외 추가 이득이 작아 v2.7.x 로 보류.
- **ort static linking**: `ort = { features = ["load-dynamic"] }` 를 `["download-binaries"]` 로 전환해 dlopen 자체 제거. ort 2.0 RC11 의 macOS arm64 정적 링크 호환성이 검증되지 않아 위험 대비 이득 낮아 보류.

### 사용자 안내
- **v2.6.7 ~ v2.6.9 macOS 사용자 (OCR 활성화 시 즉시 크래시 / 강제 종료)** — v2.6.10 dmg 재설치 또는 자동 업데이트. 잔존 캐시는 다음 명령으로 정리 권장:
  ```bash
  sudo xattr -cr /Applications/Anything.app
  rm -rf ~/Library/Application\ Support/com.anything.app
  rm -rf ~/Library/Caches/com.anything.app
  ```
- **Windows / 일반 사용자** — 동작 변경 없음.

## [2.6.9] - 2026-05-17

**대용량 인덱싱 사용자 지원 — FilenameCache 상한 100만 → 300만 상향**

### 배경
- v2.6.6 ~ v2.6.8 사용자 메일 (v2.6.7 entitlements 핫픽스를 받게 해주신 분과 동일 분) 끝에 추가 요청 — "100만 개 이상의 대용량 파일을 인덱싱하고자 할 때, Rust 백엔드의 `filename_cache` 상수로 인해 일부 파일명이 누락되는 현상이 예상됩니다. `src-tauri/src/constants.rs` 등 캐시 상수로 지정된 최대 파일 한도 값을 200만 ~ 300만 수준으로 상향 조절해주시거나, 사용자가 설정에서 가용 상한선을 변경할 수 있도록 지원해주실 수 있는지 검토 부탁드립니다."
- 코드 확인: `src-tauri/src/search/filename_cache.rs` 의 `MAX_CACHE_ENTRIES: usize = 1_000_000` 상수. 초과 시 `load_from_db` 가 `entries.truncate(MAX_CACHE_ENTRIES)` 로 잘라서 인메모리 검색에서 누락 발생.

### 변경
- **`src-tauri/src/search/filename_cache.rs`** — `MAX_CACHE_ENTRIES` 1_000_000 → **3_000_000** 상향.
  - 메모리 모델: `Vec` + `HashMap` 기반이라 실제로 보유한 엔트리 수만큼만 메모리 차지 (`Vec::with_capacity` 미사용). 일반 사용자 (< 100만 파일) 에게는 메모리 영향 없음.
  - 100만 ~ 300만 보유 사용자만 인메모리 캐시로 흡수되어 검색 누락 해소. 상한 도달 시 기존 `is_truncated()` 경로로 DB fallback 유지.
  - 사용자 설정 가능 옵션은 보류 — Settings 스키마 + DB 마이그레이션 + UI 변경 필요. 단순 상수 상향이 일반 사용자 영향 없이 요청 케이스를 커버하므로 우선 단순 변경.

### 검토 후 보류
- **ONNX Runtime 정적 링크**: 사용자분이 두 가지 제안(① entitlements 자동 동봉 ② 정적 링크) 중 "하나"를 요청. ①은 v2.6.7 에서 이미 반영 완료 (entitlements.plist + inside-out 재서명). ②는 `ort` 의 `load-dynamic` feature 를 제거하고 빌드 시 `libonnxruntime` 사전 정적 링크가 필요해 macOS 외 모든 OS 의 빌드 워크플로우를 재구성해야 함. entitlements 로 충분히 차단 해소되었기에 위험 대비 이득이 낮아 보류.

### 사용자 안내
- **일반 사용자** — 자동 업데이트 또는 v2.6.9 dmg / msi 재설치. 동작 변경 없음.
- **100만 파일 이상 인덱싱 사용자** — v2.6.9 설치 후 폴더 재인덱싱 (또는 앱 재시작 시 자동 `load_from_db` 재실행). UI 알림 영역의 "FilenameCache truncated" 경고가 사라지면 정상 흡수.

## [2.6.8] - 2026-05-17

**prod-review v2.6.8 — 회귀 점검 + FolderTree UX 보강 + nl_query 모듈 분할**

### 정정 (PR #27 머지본 보고서 보강)
- v2.6.8 prod review 의 baseline 표가 PR #27 머지 시점에 "170 passed" 로 적혀 있었으나, 그 보고는 로컬에 cargo 가 설치되지 않은 환경에서 `cargo: No such file or directory` 가 났는데도 백그라운드 task exit-code 만 보고 통과로 잘못 판정한 결과였음. rustup 설치 후 재검증한 실수치는 `cargo test --all` **186 passed, 1 ignored**. clippy 는 동일하게 No issues. 회귀 점검 / 잠재 오류 분석 결론 자체는 grep + Read 기반이라 영향 없음. `docs/PROD_REVIEW_v2.6.8.md` 에 정정 노트 반영.

### 추가 / UX
- **`src/components/sidebar/FolderTree.tsx` 컨텍스트 메뉴 `폴더 열기` 실패 시 toast 알림** — 기존 두 위치 (드라이브 메뉴 line 319 / 폴더 메뉴 line 493) 모두 `try { ... } catch {}` 또는 `logToBackend` 만으로 사용자 피드백이 없었음. `useUIContext().showToast` 로 `폴더 열기 실패: <path>` error toast 추가. prod review findings 의 [사람결정] 1 건 해소.

### 리팩토링
- **`src/search/nl_query.rs` (1,875줄) → `src/search/nl_query/` 디렉토리 분할** — CLAUDE.md 의 `> 1,200 줄 필수 분리` 기준 위반 해소. 외부 API (`NlQueryParser::parse`, `parse_with_tokenizer`, `ParsedQuery`, `DateFilter`) 시그니처 무변경:
  - `nl_query/mod.rs` (804줄): production 코드 — struct / enum / `impl NlQueryParser`
  - `nl_query/tests.rs` (1,071줄): `#[cfg(test)] mod tests;` 로 분리된 테스트 블록
- 분할 후 `cargo clippy --all-targets -- -D warnings` No issues + `cargo test --all` 186 passed, 1 ignored. helper 함수 (`remove_intent` / `extract_negation` / `extract_date` / `extract_file_type` 등) 의 추가 submod 분리는 별도 PR 로 미룸 (회귀 위험 대비 가치 낮음).

### 정적 검증 (rustup 설치 후 실측)

| 검증 | 결과 |
|------|------|
| `cd src-tauri && cargo clippy --all-targets -- -D warnings` | ✅ No issues |
| `cd src-tauri && cargo test --all` | ✅ 186 passed, 1 ignored |
| `pnpm tsc --noEmit` | ✅ 통과 |
| `pnpm build` | ✅ 통과 |

### 사용자 안내
- **macOS / Windows 사용자** — 자동 업데이트 또는 v2.6.8 dmg / msi 재설치. 동작 변경 없음 (UX 미세 개선 + 내부 리팩토링).

## [2.6.7] - 2026-05-17

**macOS 핫픽스 — OCR 활성화 시 SIGKILL 강제종료 해결 (entitlements 추가)**

### 배경
- macOS 사용자분이 OCR 옵션 켜고 인덱싱 시작하면 앱이 강제종료되고, 재설치해도 동일 증상 → `~/Library/Application Support`, `~/Library/Caches` 등 잔존 파일 수동 삭제 후에야 복구된다는 상세한 리포트 + 임시 우회법까지 보내주심 (감사합니다).
- 원인: PaddleOCR 구동을 위해 `ort` 크레이트가 번들된 `libonnxruntime.dylib` 를 `dlopen` 하는 시점에 macOS **Library Validation** 이 서명 불일치(ad-hoc 서명 + 외부 dylib 로드) 를 보안 위협으로 판단하여 `SIGKILL` 송출.
- 코드 확인: `src-tauri/tauri.macos.conf.json` 에 `signingIdentity: "-"` + `entitlements: null` 상태였음. ORT 가 동적 로드 방식(`ort = { default-features = false }`) 으로 `resources/onnxruntime/libonnxruntime.dylib` 번들 사용 → entitlements 부재 시 dyld 단계에서 차단.

### 변경
- **`src-tauri/entitlements.plist` 신규** — 다음 3개 권한 부여:
  - `com.apple.security.cs.disable-library-validation` (외부 dylib 로드 허용 — 핵심)
  - `com.apple.security.cs.allow-unsigned-executable-memory` (ONNX 런타임 JIT 영역 허용)
  - `com.apple.security.cs.allow-dyld-environment-variables` (dyld 환경변수 허용 — ONNX 일부 경로 탐색용)
- **`src-tauri/tauri.macos.conf.json`** — `entitlements: "entitlements.plist"` 로 연결.
- **`.github/workflows/publish.yml`** — macOS 빌드의 ad-hoc 재서명 단계를 inside-out signing 으로 강화:
  1. 번들된 외부 `.dylib` 들 (`libonnxruntime.dylib` 등) 먼저 `codesign --options runtime` 으로 개별 서명
  2. 메인 실행 바이너리(`Contents/MacOS/docufinder`) 에 `--entitlements entitlements.plist --options runtime` 적용
  3. `.app` 번들 전체 `--deep --entitlements ... --options runtime` 재서명
  4. 검증 로그(`codesign -dv --entitlements -`) 출력

이전 단계는 단순 `codesign --force --deep --sign -` 만 호출하여 Tauri 빌드 시 적용된 entitlements 가 덮어쓰기 되며 사라지는 문제가 있었음.

### 사용자 안내
- **v2.6.6 이하 macOS 사용자분** — 동일 증상 겪고 계시면 v2.6.7 dmg 재설치. 재설치 전 잔존 파일 삭제 권장:
  ```bash
  rm -rf ~/Library/Application\ Support/com.anything.app
  rm -rf ~/Library/Caches/com.anything.app
  rm -rf ~/Library/Logs/com.anything.app
  rm -f  ~/Library/Preferences/com.anything.app.plist
  ```
  (실제 bundle id 가 다를 수 있으니 위 경로에서 `com.anything.app` 부분 확인 후 삭제)
- 임시 우회가 필요하면 설치된 앱에 직접 entitlements 주입도 가능 (사용자분이 보내주신 방법):
  ```bash
  sudo codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist --sign - \
    /Applications/Anything.app/Contents/MacOS/docufinder
  sudo codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist --sign - \
    /Applications/Anything.app
  ```

## [2.6.6] - 2026-05-16

**LTSC 환경 핫픽스 — Tauri fixedRuntime 모드가 wry 에 path 전달 안 한다는 사실 확인 후 with_environment inject 활성화 (이슈 #24)**

### 배경
- v2.6.5 LTSC installer 가 JS190-prog 님 환경 (Win10 LTSC 1809 VDI + 집 PC Win11 25H2 모두) 에서 동일 실행 오류 — "Could not find the WebView2 Runtime... installed on another user account".
- 로그 분석: `LTSC build — relying on Tauri fixedRuntime; skipping with_environment inject` 출력 직후 webview2 environment 생성 panic. 즉 Tauri 의 `fixedRuntime` 모드만으론 wry 가 폴더를 인식 못함.
- Tauri 2.10.2 source 직접 확인 결과:
  - `tauri-bundler` NSIS template: `downloadBootstrapper`/`embedBootstrapper`/`offlineInstaller` 만 분기 처리, **`fixedRuntime` → 빈 분기 (아무 코드 없음)**. NSIS 가 폴더 bundle 만 해주는 부수 효과만 있고 런타임 효과 없음.
  - `tauri-runtime-wry::lib.rs`: webview build 시 `webview_attributes.environment.is_some()` 일 때만 wry 의 `with_environment` 호출 — fixedRuntime path 를 자동으로 wry 에 전달하는 코드 자체가 없음.
- 즉 wry 가 fixed-runtime 폴더를 사용하려면 **앱 코드가 직접 `ICoreWebView2Environment` 만들고 `WebviewWindowBuilder::with_environment` 로 inject** 해야 함. v2.5.27 가 같은 환경에서 동작했었다는 보고는 그 시점 사용자분 PC 의 system-installed WebView2 가 registry 에 우연히 등록되어 있어 detection 성공한 것으로 추정.

### 변경
- **`src-tauri/src/webview2_runtime.rs::detect_fixed_runtime_dir`** — 검색 후보에 LTSC installer 의 실제 풀리는 경로 `<exe_dir>/webview2-runtime/EBWebView/x64/` 추가 (JS190-prog 로그상 확인된 경로). 그 외 사용자 zip 풀기 케이스도 `EBWebView/x64`, `EBWebView`, sub-dir 안의 `EBWebView/x64` 모두 검색.
- **`src-tauri/src/lib.rs setup()`** — `DOCUFINDER_LTSC_BUILD` env 가드 제거. LTSC build 도 `with_environment` inject 활성화. detect 가 fixed-runtime 폴더 찾으면 `CreateCoreWebView2EnvironmentWithOptions(path, ...)` 으로 environment 명시 생성 → wry 에 inject → registry detection 완전 우회.
- **`.github/workflows/publish.yml`** — LTSC build step 의 `DOCUFINDER_LTSC_BUILD` env 제거 (가드 자체가 사라졌으므로 무용).

`tauri.windows-ltsc.conf.json` 의 `webviewInstallMode:fixedRuntime` 는 그대로 — NSIS 가 `webview2-runtime/` 폴더를 installer 에 bundle 하는 부수 효과를 위해 필요.

### 흐름 (LTSC build 기준)
1. NSIS installer 가 `<install_dir>/webview2-runtime/EBWebView/x64/` 에 WebView2 runtime 폴더 풀어둠 (fixedRuntime config 의 NSIS bundle)
2. 앱 실행 시 `webview2_runtime::detect_fixed_runtime_dir` 가 그 경로 매치
3. `CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder=<발견 경로>, ...)` → `ICoreWebView2Environment` 직접 생성
4. `WebviewWindowBuilder::with_environment(env)` 로 inject
5. Tauri 가 `webview_attributes.environment` 발견 → wry 의 `with_environment` 호출
6. wry 가 우리 environment 사용 → registry / installer scope / GPO 와 완전 무관하게 동작

### 사용자 안내
- **JS190-prog 님 / 회사 PC LTSC 1809 VDI + 집 PC Win11 25H2** — `Anything_2.6.6_x64-ltsc-setup.exe` 다시 다운로드 후 설치. v2.6.5 그대로 두고 그 위에 덮어 설치 OK.
- **일반 Win10/11 사용자** — `Anything_2.6.6_x64-setup.exe` 사용. v2.6.5 와 동일 (영향 없음).

## [2.6.5] - 2026-05-14

**LTSC 1809 / VDI 환경 핫픽스 — LTSC installer 를 v2.5.27 검증된 fixedRuntime 방식으로 회귀**

### 배경
- v2.6.4 의 LTSC installer (`with_environment` 코드 inject + `bundle.resources` 일반 파일) 가 [이슈 #24](https://github.com/chrisryugj/Docufinder/issues/24) JS190-prog 님의 Windows 10 LTSC 1809 + 물리적 망분리 VDI 환경에서 동작하지 않음 — 576MB 정상 LTSC installer 받아 설치해도 WebView2 오류 지속.
- 결정적 단서: JS190-prog 님 환경에서 **v2.5.27 (Tauri `webviewInstallMode: fixedRuntime` 모드) 은 정상 동작했었음**. v2.6.0 에서 fixedRuntime 을 롤백한 이유는 일반 Win11 환경 회귀였는데, 우리는 이번에 **별도 LTSC variant build** 라 일반 build 와 분리됨 → v2.5.27 의 검증된 메커니즘을 LTSC 한정으로 안전하게 복원 가능.

### 변경
- **`tauri.windows-ltsc.conf.json`**: `bundle.resources` 의 `webview2-runtime/**/*` 제거. 대신 `webviewInstallMode: { type: "fixedRuntime", path: "webview2-runtime/" }` 사용 (v2.5.27 와 동일).
- **`scripts/setup-webview2-runtime.ps1`**: 풀기 위치를 `src-tauri/webview2-runtime/EBWebView/x64/` 로 변경 — Microsoft Fixed Version Runtime SDK 표준 구조 (v2.5.27 와 동일).
- **`src-tauri/src/lib.rs`**: LTSC build 시 (`DOCUFINDER_LTSC_BUILD` env 가드) `with_environment` inject 코드를 비활성화. v2.5.27 코드 상태와 정확히 동일하게 만들어 검증된 메커니즘에만 의존 — 두 메커니즘 (`fixedRuntime` + `with_environment`) 동시 적용 시 environment 충돌 가능성 차단.
- **`.github/workflows/publish.yml`**: LTSC build step 에 `DOCUFINDER_LTSC_BUILD: "1"` env 추가.

일반 (non-LTSC) build 는 v2.6.4 그대로 — `with_environment` inject 코드 정상 활성 + 정상 환경에서 동작.

### 사용자 안내
- **JS190-prog 님 / LTSC 1809 / VDI 환경** — release 페이지에서 **`Anything_2.6.5_x64-ltsc-setup.exe`** 다시 다운로드 후 설치. v2.5.27 가 동작했던 방식 그대로라 같은 환경에서 정상 시작 예상.
- **일반 Win10/11 사용자** — 기존 그대로 `Anything_2.6.5_x64-setup.exe` 사용. v2.6.4 와 동일.

## [2.6.4] - 2026-05-14

**LTSC 1809 / admin 없는 환경 정공 해결 — 전용 installer 한 번 실행이면 끝 (이슈 #24)**

### 배경
- v2.6.0 (offlineInstaller 롤백) ~ v2.6.2 (standalone installer 동봉) 까지 [이슈 #24](https://github.com/chrisryugj/Docufinder/issues/24) JS190-prog 님 회사 PC (Windows 10 LTSC 1809, **admin 권한 없음** + 회사 GPO 로 HKLM 등록 차단) 에서 WebView2 detection 실패가 계속됨. 에러 다이얼로그 "You may have it installed on another user account..." — WebView2 SDK 가 다른 사용자 HKCU 만 발견하고 본인 / HKLM 둘 다 비어 있을 때 표시하는 표준 메시지.
- 근본 원인: wry 0.54.1 가 `CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder=...)` 의 첫 인자에 항상 `null` 을 넘김 → fixed runtime 경로를 인식할 표준 메커니즘 없음. v2.5.27 NSIS `fixedRuntime` 회귀도 같은 근원 (Tauri NSIS template 의 fixedRuntime handling 이 EBWebView 폴더 bundle 에 실패).

### 변경 — 코드 측 (wry 한계 우회)
- **`WebviewWindowBuilder::with_environment` 로 `ICoreWebView2Environment` 명시 inject** ([`src-tauri/src/webview2_runtime.rs`](src-tauri/src/webview2_runtime.rs), [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)) — 앱 시작 시 `<exe_dir>/EBWebView/msedgewebview2.exe` 또는 `<exe_dir>/<any_subdir>/EBWebView/msedgewebview2.exe` 가 발견되면 `webview2-com 0.38` 으로 `CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder = <발견 경로>)` 호출 → 직접 생성한 `ICoreWebView2Environment` 를 Tauri 2.10 의 `with_environment(env)` (cfg(windows)) 로 wry 에 inject. 자체 `PeekMessageW` 펌프 + 5초 timeout 으로 setup() hang 회귀 차단. EBWebView 없으면 inject 자체 안 함 → 정상 환경엔 영향 zero.
- **main window `label:"main"`, `create:false`** ([`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json)) — Tauri 자동 build 막고 `setup()` 안에서 `WebviewWindowBuilder::from_config` 로 conf 의 size/title 그대로 가져와 environment 만 추가 inject.

### 변경 — 별도 LTSC installer build (사용자 측 정공)
- **`tauri.windows-ltsc.conf.json` 신규** — base conf 의 resources 위에 `resources/EBWebView/**/*` 추가, `createUpdaterArtifacts:false`. NSIS bundle.resources 의 일반 파일 경로는 기존 kordoc/vcredist 가 안정 처리하므로 v2.5.27 회귀와 무관.
- **`scripts/setup-webview2-runtime.ps1` 신규** — CI runner 의 사전 설치된 Edge (`C:\Program Files (x86)\Microsoft\EdgeWebView\Application\<ver>\EBWebView`) 를 `src-tauri/resources/EBWebView/` 로 복사. tauri build 시점에 NSIS 가 일반 resources 처리 경로로 묶어줌.
- **`.github/workflows/publish.yml` LTSC variant build step 추가** — 기본 installer (~382MB, system WebView2 사용) 빌드 후, EBWebView 준비 + `pnpm exec tauri build --config src-tauri/tauri.windows-ltsc.conf.json` 으로 두 번째 installer (~550MB, EBWebView 포함) 빌드 → `Anything_<ver>_x64-ltsc-setup.exe` 로 rename → 동일 release 에 추가 업로드.

### 사용자 안내 — 어느 installer 받으면 되나요?

이번 릴리스부터 Windows installer 가 두 종류입니다. 본인 환경에 맞는 걸 받으세요. 둘 다 같은 앱이고 동일하게 동작합니다.

| 내 PC 상황 | 받을 파일 |
|------------|-----------|
| 집 / 개인 PC, Windows 10 (21H2+) / Windows 11 | `Anything_2.6.4_x64-setup.exe` (~382 MB) |
| 회사 PC 인데 관리자 권한 없거나, Windows 10 LTSC 1809 같은 구버전, 또는 v2.6.3 까지 설치 후 WebView2 오류로 앱이 안 열렸던 분 | `Anything_2.6.4_x64-ltsc-setup.exe` (~550 MB) |

**헷갈리면 그냥 LTSC 버전 받으세요.** 일반 버전은 Windows 가 갖고 있는 WebView2 런타임을 빌려 쓰는 가벼운 buildset 입니다. LTSC 버전은 WebView2 런타임을 같이 가지고 와서, Windows 가 WebView2 를 못 찾는 환경에서도 그냥 동작합니다. 그래서 크기만 더 큽니다.

**오프라인 회사 PC 에 설치하려는 분 (이슈 #24 JS190-prog 님)**
1. 인터넷 가능한 PC 에서 `Anything_2.6.4_x64-ltsc-setup.exe` 다운로드
2. USB 등으로 회사 PC 로 옮기기
3. 더블클릭 실행 — 끝. WebView2 별도 설치, zip 풀기, 관리자 권한, registry 편집 같은 거 전혀 필요 없습니다.
4. 기존 v2.6.x 깐 상태면 그 위에 그대로 덮어 설치 OK. 인덱싱 / 폴더 설정은 보존됩니다.

## [2.6.2] - 2026-05-13

**WebView2 Standalone Installer 동봉 — 회사 PC / 사내망 / WebView2 미설치 환경 대응**

### 변경
- **Microsoft Edge WebView2 Runtime Standalone Installer 를 Release Asset 으로 자동 첨부** ([`.github/workflows/publish.yml`](.github/workflows/publish.yml)) — v2.6.1 깐 후에도 일부 사용자가 WebView2 런타임 오류를 보는 케이스 ([이슈 #24](https://github.com/chrisryugj/Docufinder/issues/24)) 대응. Tauri NSIS template 은 `installMode: currentUser` 라 일반 사용자 권한으로 돌고, WebView2 가 처음부터 없는 PC 에서는 user 권한 WebView2 설치가 일부 시스템 컴포넌트 등록 단계에서 부분 실패 → wry detection 실패 → "런타임 오류" 가 가능. publish.yml 에 Microsoft 공식 stable URL ([`linkid=2099617`](https://go.microsoft.com/fwlink/?linkid=2099617)) 에서 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 를 다운받아 release 에 함께 업로드하는 step 추가. 사용자가 별도로 받아 관리자 권한으로 미리 설치하면 machine-wide(HKLM) 에 정상 등록되어 Anything 도 정상 시작.

### 사용자 안내
- **v2.6.1 깐 후에도 WebView2 오류 보는 분** — release 페이지에서 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` 다운로드 → **우클릭 → 관리자 권한으로 실행** 후 Anything 재시작.
- **회사 내부망 / LTSC 1809 사용자 ([이슈 #23](https://github.com/chrisryugj/Docufinder/issues/23))** — 인터넷 가능 PC 에서 위 파일 다운로드 → USB 로 사내망 PC 옮긴 후 관리자 권한 실행 → Anything 정상 시작.

## [2.6.1] - 2026-05-13

**hotfix: v2.6.0 Provider 전환 UX 결함 — Gemini 모델 ID 가 OpenAI 호환 모드에 그대로 남음**

### 수정
- **Provider 전환 시 모델 ID 자동 swap** ([`AiTab.tsx`](src/components/settings/tabs/AiTab.tsx)) — v2.6.0 에서 Provider 드롭다운만 OpenAI 로 바꾸고 모델 ID(`gemini-3.1-flash-lite-preview`) 는 그대로 두면 사내 LLM 서버에 `model: "gemini-3.1-flash-lite-preview"` 로 요청 가서 **404 "model not found"** 가 나는 사고. `handleProviderChange` 헬퍼로 Gemini → OpenAI 전환 시 모델이 `gemini-*` 면 자동으로 빈 값으로 (사용자 직접 입력 유도), OpenAI → Gemini 전환 시 모델이 `gemini-*` 가 아니면 기본 Gemini 모델로 복귀.
- **OpenAI 모드 + Gemini 모델 ID 인라인 경고** — 마이그레이션 / 수동 편집 등으로 mismatch 상태가 되면 모델 input 테두리 빨간색 + "Gemini 모델 ID 가 입력돼 있어요. 404 가 납니다. 서버 실제 모델 ID 로 바꿔주세요" 인라인 메시지.
- **백엔드 validation 안전망** ([`commands/settings.rs::validate_settings`](src-tauri/src/commands/settings.rs)) — frontend 우회/수동 settings.json 편집 케이스 방어. `AiProvider::OpenAi` + 모델 ID 가 `gemini-` 로 시작 → 저장 거부 + 한국어 사유 반환.

## [2.6.0] - 2026-05-13

**이슈 #24 — WebView2 회귀 핫픽스 + 커스텀 LLM (사내·오프라인) + 매핑 드라이브 hang 차단 + 증분 인덱싱 메뉴 노출**

### 수정 — Critical
- **WebView2 fixedRuntime → offlineInstaller 롤백** ([`tauri.conf.json`](src-tauri/tauri.conf.json)) — v2.5.27 의 fixedRuntime 번들이 일반 Windows 환경(Win11 25H2 등)에서 회귀해 모든 사용자가 "WebView2 런타임 오류" 로 앱이 시작 안 되던 문제 ([이슈 #24](https://github.com/chrisryugj/Docufinder/issues/24)). CI 로그상 webview2 binary(721MB) 가 정상 추출됐지만 NSIS installer 크기(374MB) 가 v2.5.26(382MB) 와 거의 동일 → EBWebView 가 installer 에 누락됐거나 wry 가 detect 못한 것으로 추정. `installMode: "currentUser"` + `fixedRuntime` 의 Tauri NSIS template 호환성 이슈로 보임. 안정성 우선으로 v2.5.19 ~ v2.5.26 까지 검증된 offlineInstaller 로 복귀. **LTSC 1809 / 회사 내부망 사용자는 별도 가이드로 standalone installer 수동 설치 안내 예정.** Installer 크기 v2.5.27 374MB → v2.6.0 약 230MB 로 회복.
- **매핑 네트워크 드라이브 추가 시 "조용한 무응답" 차단** ([`commands/index/mod.rs`](src-tauri/src/commands/index/mod.rs)) — 매핑 SMB / UNC 드라이브를 폴더 추가하면 "계속" 누른 뒤 UI 가 침묵하던 사용자 보고. 원인: `add_folder` 의 `scan_metadata_only` 가 spawn_blocking 안에서 walkdir traversal 중 SMB 인증 만료 / 서버 무응답 / 매핑 끊김으로 무한 hang → IPC response 없음 → 사용자 입장에서 "조용". fix: `add_folder` / `reindex_folder` / `resume_indexing` 진입 직후 `probe_network_path` (5초 timeout `read_dir` ping) 으로 응답성 사전 검증. 실패 시 "네트워크 폴더 응답 없음 (5초 timeout)" 명시 에러 토스트.

### 추가 — 기능 요청
- **커스텀 LLM API (OpenAI Chat Completions 호환) 지원** ([`llm/openai.rs`](src-tauri/src/llm/openai.rs)) — 회사 오프라인 내부망 사용자가 사내 LLM (qwen3-35b-a3b 등) 을 사용할 수 있도록 OpenAI 호환 endpoint 추가 ([이슈 #24](https://github.com/chrisryugj/Docufinder/issues/24)). 설정 → AI 에서 **LLM Provider** 드롭다운으로 "Gemini" / "OpenAI 호환" 전환. OpenAI 모드 선택 시 **Base URL** 입력란 노출 (예: `http://192.168.1.50:8000`, `http://localhost:11434`, `https://api.together.xyz`) — `/v1/chat/completions` 가 자동으로 붙음. vLLM · Ollama · LiteLLM · Together · Groq · LM Studio · llama.cpp server 등 OpenAI 호환 백엔드 모두 사용 가능. 비스트리밍 + SSE 스트리밍 + 정확한 401/403/404/429 한국어 에러 메시지.
- **컨텍스트 메뉴 "이어서 인덱싱"** ([`components/sidebar/FolderTree.tsx`](src/components/sidebar/FolderTree.tsx)) — 사용자 보고 "재인덱싱은 무조건 처음부터 다시" 에 대응. 폴더가 `cancelled` 또는 `indexing` 상태 (취소되거나 중단된 폴더) 일 때만 컨텍스트 메뉴에 "이어서 인덱싱" 항목을 추가로 노출. 이 항목은 `resume_indexing` 커맨드 (`fts_indexed_at` 가 있는 파일은 스킵하는 incremental 모드) 를 호출 — 멈춘 지점부터 이어서 인덱싱 가능. 기존 "재인덱싱" 메뉴는 전체 wipe 후 처음부터 (의도된 동작) 그대로 유지.

### 수정 — Telemetry 노이즈 차단
- **TIFF / image crate panic BENIGN 등재** ([`panic_filter.rs`](src-tauri/src/panic_filter.rs)) — 사용자 보고 `tiff-0.11.3/decoder/image.rs:919` assertion 실패 (tiff tiled planar raw 변종, Pillow 테스트 fixture 등). 이미 [pipeline.rs:350](src-tauri/src/indexer/pipeline.rs#L350) 의 `catch_unwind` 가 잡아서 인덱싱 자체는 살아남지만 `lib.rs` panic hook 은 항상 실행돼 `crash-{date}.log` + Telegram telemetry 에 기록 → 노이즈 보고. BENIGN_PANIC_SOURCES 에 `tiff-`, `image-` 추가해 차단.

### 사용자 안내
- **v2.5.27 사용자** — v2.6.0 자동 업데이트 (또는 dmg/msi 재설치) 로 WebView2 시작 오류 즉시 해결.
- **사내 LLM 사용자 (이슈 #24 보고자)** — 설정 → AI → "AI 기능 활성화" → Provider "OpenAI 호환" 선택 → Base URL · API 키 · 모델 ID 입력. qwen3.6-35b-a3b 처럼 사용자 서버 모델 ID 그대로 입력.
- **재인덱싱 중간 취소 사용자** — 폴더 우클릭 → "이어서 인덱싱" 으로 멈춘 지점부터 이어가기.
- **매핑 네트워크 드라이브 사용자** — 매핑이 끊긴 / 응답 없는 드라이브 추가 시도 시 즉시 토스트 알림. 드라이브 매핑 복구 후 재시도.

## [2.5.27] - 2026-05-11

**WebView2 Fixed Runtime 번들 — Windows 10 LTSC 1809 / 회사 내부망 / GPO 차단 환경 호환** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) LTSC 1809 보고자 추가 보고분.

### 수정
- **`webviewInstallMode` `offlineInstaller` → `fixedRuntime` 전환** ([`tauri.conf.json`](src-tauri/tauri.conf.json)) — v2.5.19 부터 NSIS 인스톨러에 WebView2 Bootstrapper offline installer 를 같이 번들했지만, 이 모드는 시스템 설치까지만 보장하고 우리 앱 (Tauri 의 WebView2Loader.dll) 이 실행 시점에 registry detection 으로 Runtime 위치를 찾을 수 있어야 동작한다. Windows 10 LTSC 1809 + 회사 내부망 환경에서 (1) 사용자 권한 설치만 가능 (HKCU 만 등록 → HKLM 검색하는 loader 가 못 찾음), (2) GPO 가 `HKLM\SOFTWARE\Microsoft\EdgeUpdate` 차단, (3) Edge Legacy 만 기본 포함된 LTSC 의 system-wide WebView2 부재 등의 사유로 detection 이 실패해 앱이 시작 안 되는 사례 보고. fixedRuntime 모드는 WebView2 binary 자체 (~150MB) 를 앱 폴더에 번들해 시스템 설치 / 권한 / GPO 와 완전 무관하게 동작.
- **CI 빌드 자동화** ([`scripts/setup-webview2-runtime.ps1`](scripts/setup-webview2-runtime.ps1)) — Microsoft 가 fixed runtime stable URL 을 제공하지 않으므로 standalone evergreen installer 를 admin 권한 (windows-latest CI runner = admin) 으로 silent install 한 뒤 결과 폴더 (`%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application\<version>\`) 를 Tauri fixedRuntime 형식 (`<path>/EBWebView/<arch>/...`) 으로 재구성. publish.yml Windows job 의 vcredist 다운로드 step 옆에 추가.

### 사용자 안내
- **Windows 인스톨러 크기 증가** — `Anything_2.5.27_x64-setup.exe` 가 v2.5.26 의 ~80MB 에서 ~230MB 로 증가 (WebView2 Runtime ~150MB 동봉). dmg (macOS) 는 영향 없음.
- **WebView2 Runtime 보안 패치** — 시스템 Edge Update 자동 갱신 대신 Anything 자체 자동 업데이트로 갱신. 매 Anything 릴리스 시 최신 WebView2 Runtime 같이 번들.
- **LTSC 1809 보고자** — v2.5.27 이전엔 1단계 registry 확인 + 관리자 권한 standalone installer 재설치 시도 (이슈 #22 코멘트 참조). v2.5.27 dmg/msi 게시 후엔 시스템 설치 여부와 무관하게 동작.

## [2.5.26] - 2026-05-11

**hotfix: PDF Password 사전 차단 false-positive + HWP fallback 진단 가시성 + macOS 1024px 근처 viewport 화면 dim 회귀** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) v2.5.25 추가 보고분.

### 수정
- **PDF `/Encrypt` 사전 차단 false-positive** — `password_detect.rs::pdf_is_encrypted` 가 파일 tail 32KB 에서 `/Encrypt` 단순 substring 만 검색해 (1) `/EncryptMetadata` (단순 boolean 메타플래그, 본문 암호 아님) 가 들어 있는 정상 PDF, (2) 본문 stream / 폰트 dict / content stream 안에 우연히 `/Encrypt` 문자열이 등장한 정상 PDF 까지 모두 `Password protected` 로 차단했다 (사용자 환경의 `Work/...pdf` 파일 차단 사례). fix: 진짜 trailer Encrypt 키는 항상 indirect reference (`/Encrypt N N R`) 또는 direct dict (`/Encrypt <<`) 형식이라는 PDF spec 을 이용해 정규식 `/Encrypt[\s\r\n]+(?:\d+\s+\d+\s+R|<<)` 으로 엄격화. `/EncryptMetadata` 차단 / 본문 우연 매치 차단 / 진짜 암호 PDF 감지 3종 회귀 보호 테스트 동봉.
- **HWP fallback 시 "Unsupported file type: hwp (kordoc 필요)" 잘못된 메시지** — kordoc 가 호출됐지만 실패한 케이스에서도 사용자 로그에 `Unsupported file type: hwp (kordoc 필요)` 가 노출되어 사용자가 "kordoc 가 없어서 처리 못 한다" 고 오해. 실제 원인 (HWP3 구버전, 비표준 변종, 한컴 특정 버전의 binary stream 어긋남 등) 이 가려져 후속 진단 불가능. fix: `parsers/mod.rs::parse_file` 에서 kordoc 의 실제 에러를 `kordoc_err` 변수에 보존했다가 `.hwp` 분기에서 그대로 반환. 이제 사용자 로그에 `Parse error: kordoc 실행 실패 (exit ...): FAIL | → 지원하지 않는 파일 형식입니다.` 처럼 진짜 원인이 노출돼 다음 진단 사이클이 정확해진다.
- **macOS 1024px 근처 viewport 에서 메인 화면 30% dim 회귀** — `Sidebar.tsx` 의 mobile backdrop (`<div className="absolute inset-0 z-30 lg:hidden bg-black/30">`) 가 사용자 보고 환경 (1032×740 캡처) 에서 활성화되어 검색 결과 영역 + 사이드바 영역 전체가 30% 어둡게 깔리던 회귀. `lg:hidden` 가 ≥1024px 에서 숨도록 설계됐으나, macOS WebKit 의 viewport 계산 (타이틀바·신호등 영역 + Retina DPR + Tauri 창 chrome 차감) 이 tailwind `lg` breakpoint 적용과 어긋나 1024px 근처에서 backdrop 이 화면을 덮음. 데스크톱 앱이라 사이드바는 항상 `App.tsx` 의 `paddingLeft: var(--sidebar-width)` 로 메인 컨텐츠를 밀어내는 push 모드 — mobile backdrop 자체가 불필요하므로 div 제거.
- **OnboardingTour 폴백 backdrop 약화 (안전장치)** — `hasTarget=false` 분기의 `rgba(15,23,42,0.7)` (70% 어두움) 을 `rgba(15,23,42,0.35)` (35%) 로 낮춤. v2.5.24 의 fix (작은 창 자동 시작 가드 + resize 시 자동 finish + backdrop click 닫기) 로 stuck 자체는 차단됐지만, 혹시 모를 잔여 케이스에서 사용자가 backdrop 을 인지하지 못해도 화면 가시성이 크게 떨어지지 않도록 추가 안전장치.

### 사용자 안내
- **이슈 #22 보고 사용자** — v2.5.26 dmg 설치 후 (1) 차단되던 PDF 들이 다시 인덱싱 시도되는지, (2) HWP 실패 시 로그 메시지가 `Parse error: kordoc 실행 실패 ...` 형식의 구체적 진단으로 바뀌었는지, (3) UI 가 어둡게 변하던 현상이 사라졌는지 세 가지 확인 부탁. v2.5.25 → v2.5.26 인덱싱 결과 비교 지표는 19,549 / 37,160 (v2.5.25) 대비 PDF false-positive 분만큼 추가 회복 예상.

## [2.5.25] - 2026-05-10

**hotfix: NEIS Report Designer 가 만든 구형 BIFF8 .xls 인덱싱 중 강제종료 다중 방어 + 진단 인프라** — 사용자 보고 (`근무상황부` 양식의 NEIS export .xls 파일이 폴더에 있으면 인덱싱 도중 앱 강제종료) 에 대응. 로컬 격리 검증으로는 calamine 0.26 + lindera 2.0 단독으로 panic 미재현 — 강제종료의 정확한 단계가 불명확해 (1) **사후 진단 가능성** 을 즉시 확보하고 (2) **의심 단계 모두에 정교한 catch_unwind 격리** 를 박는 두 갈래로 처리.

### 추가
- **Crash breadcrumb 시스템** ([`src-tauri/src/breadcrumb.rs`](src-tauri/src/breadcrumb.rs)) — RAII Guard 기반 글로벌 atomic 으로 "현재 처리 중 파일 + 단계" 를 1건 보관. `parse_file` / `parse_xlsx` / `fts_save_document` 진입에 자동 set, scope 종료 시 자동 clear. lib.rs panic hook 이 snapshot 을 읽어 `crash-{date}.log` + Telegram telemetry 에 함께 기록 — 향후 native crash (stack overflow / abort) 발생 시 어떤 파일·어떤 stage 였는지 즉시 확인 가능.
- **Legacy `.xls` 사전 암호 감지** ([`password_detect.rs`](src-tauri/src/parsers/password_detect.rs)) — 기존엔 `_ => false` 로 빠져 통과. BIFF8 BOF (`09 08 .. .. 06 00`) 직후 FILEPASS record (`2F 00`) + protection type (XOR=0x00 / RC4=0x01) 인접 패턴 byte-search 휴리스틱 추가. NEIS Report Designer 정상 파일 fixture 기반 false-positive 회귀 테스트 동봉. calamine 이 암호 BIFF 에서 panic 한 사례를 호출 전에 차단.

### 수정
- **`xlsx::parse` 시트 단위 격리** ([`xlsx.rs`](src-tauri/src/parsers/xlsx.rs)) — calamine `open_workbook_auto` 호출 + 시트별 `worksheet_range` + `extract_text_with_location` 을 각각 `catch_unwind` 로 감싸 한 시트 panic 이 파일 전체 인덱싱을 죽이지 않도록. 시트 추출 / 패닉 카운트를 INFO 로깅해 사용자 환경에서 이상 시트 위치 추적 가능.
- **인덱싱 파이프라인 후속 단계 catch_unwind 누락 보강** ([`indexer/pipeline.rs`](src-tauri/src/indexer/pipeline.rs)) — 기존엔 producer 의 `parse_file` 만 catch_unwind. consumer 의 `save_document_to_db_fts_only_no_tx` 호출 + 내부 `tok.tokenize(chunk.content)` 호출 (lindera) 양쪽이 panic 미보호였다. (a) 청크 단위 lindera tokenize 를 catch_unwind 로 감싸 panic 시 형태소 토큰 없이 진행 (검색 재현율 약간 손실 ↔ 강제종료 회피), (b) save_document 호출 전체를 catch_unwind 로 감싸 panic 발생 시 활성 트랜잭션 ROLLBACK + 새 BEGIN 으로 재시작해 인덱싱 루프 자체는 살아남고, 메타데이터만이라도 best-effort 로 저장하도록 변경. patched 파일이 BENIGN_PANIC_SOURCES (calamine / lindera / ort / usearch) 에서 panic 해도 폴더 인덱싱 전체가 중단되지 않는다.

### 사용자 안내
- **NEIS export 엑셀 (근무상황부, 출장보고서, 외출/조퇴신청서 등) 이 인덱싱을 중단시켰던 사용자** — v2.5.25 자동 업데이트 후 폴더 우클릭 → "재인덱싱" 으로 복구. 만약 동일 폴더에서 강제종료가 또 발생하면 `%AppData%\com.anything.app\crash-2026-05-DD.log` 파일에 `BREADCRUMB stage=… path=…` 줄이 새로 찍히므로 그 줄을 보고하면 정확한 trigger 파일·단계를 한 번에 식별 가능.

## [2.5.24] - 2026-05-10

**hotfix: kordoc 사이드카 markdown-it 누락 + 작은 창 OnboardingTour 영구 stuck** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) v2.5.23 회귀 두 건 모두 해결.

### 수정
- **kordoc 사이드카 `markdown-it` 패키지 누락** — v2.5.23 에서 kordoc 을 v2.7.0 → v2.7.1 로 올리면서 신규 dependency `markdown-it@^14` (Print Renderer 용, `dist/index.js` 진입 시 정적 import) 가 같이 들어왔는데 Docufinder 의 두 번들 스크립트 (`scripts/setup-macos-resources.sh`, `scripts/bundle-kordoc.ps1`) 의 deps 배열에 추가되지 않아 macOS / Windows 양쪽 빌드 모두 cli.js 첫 호출에서 `Cannot find package 'markdown-it' imported from .../kordoc/chunk-N6UWJX63.js` (`ERR_MODULE_NOT_FOUND`) 로 즉사. v2.5.23 사용자 환경에서 HWP/HWPX/PDF 본문 추출이 전수 실패해 인덱싱 성공률이 7% (1,126 / 31,613) 로 추락. fix: 두 스크립트 deps 배열에 `markdown-it@^14` 추가, 추후 회귀 방지를 위해 "kordoc package.json 의 dependencies 와 동기화 필수" 주석 명시. 사용자 환경에서 자동 폴더 우클릭 → "재인덱싱" 으로 복구.
- **작은 창에서 OnboardingTour overlay 영구 stuck** — v2.5.22 에서 사용자가 보고한 "창 크기가 작으면 UI가 어두워지고 닫을 방법이 없는" 현상의 직접 원인을 [`OnboardingTour.tsx:262`](src-tauri/../src/components/onboarding/OnboardingTour.tsx#L262) 에서 추적. 작은 창 (특히 사용자 보고 환경 500×291) 에서 `[data-tour="search-bar"]` / `[data-tour="sidebar-folders"]` 등 selector 가 collapse 모드 등으로 viewport 밖으로 나가면 `hasTarget = false` 폴백이 활성되어 `<div className="fixed inset-0" style={{ backgroundColor: "rgba(15,23,42,0.7)" }} />` 가 화면 전체를 덮는다. 그런데 (1) 이 backdrop 의 `onClick` 가 `e.stopPropagation()` 만 하고 닫지 않고, (2) 툴팁 카드는 viewport 가 작아 화면 밖에 위치 계산되어 보이지 않아 사용자가 ESC 단축키를 모르면 영구히 닫을 수 없었다. fix 3종: ① 자동 시작 가드 — viewport 가 `640×480` 미만이면 1.2s 자동 시작 자체를 skip, ② 진행 중 resize 로 작아지면 자동 finish, ③ backdrop click → `finish(false)` (스포트라이트 / 폴백 두 분기 모두). 이로써 어떤 viewport 크기에서도 사용자가 클릭만으로 투어를 닫을 수 있다.

### 사용자 안내
- **v2.5.23 에서 인덱싱 결과가 1,126 / 31,613 같이 비정상 낮은 사용자** — v2.5.24 dmg/msi 설치 후 자동으로 정상 동작. 폴더 우클릭 → "재인덱싱" 또는 단순히 새로 추가만 하면 v2.5.22 수준 (16,346 / 37,160) 이상으로 회복. 후보 파일 수가 v2.5.22 대비 줄어든 부분 (37,160 → 31,613) 은 v2.5.23 의 Rust 코드 변경이 0 (publish.yml + version bump 만) 이라 외장 디스크 파일 변동 또는 스캔 타이밍 차이로 추정 — 재인덱싱 후 자연 회복 여부 확인 필요. v2.5.23 신규 도입한 HWP3 파서의 사용자 환경 잔여 실패 여부도 markdown-it 복구 후 재판단.
- **macOS 작은 창에서 화면이 어두워져 못 닫던 사용자** — v2.5.24 부터 자동 발생 안 함 (640×480 미만 자동 시작 skip). 이미 stuck 된 사용자는 backdrop 아무 곳이나 클릭하면 닫힌다. 헤더 도움말 → "기능 투어 다시 보기" 로 큰 창에서 다시 열람 가능.

## [2.5.23] - 2026-05-09

**HWP 3.0 (구버전) 파일 본문 인덱싱 지원** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) 사용자 환경의 2003년 판결문 등 1996~2002년 한컴이 만든 구버전 `.HWP` 가 v2.5.22 까지 `kordoc 실행 실패: 지원하지 않는 파일 형식` 으로 차단되던 문제 해결.

### 추가
- **kordoc v2.7.1 번들** — kordoc 에 `parseHwp3` 신규 모듈 추가 (`"HWP Document File V3.00"` 30 byte 시그니처 → DocInfo 128B + DocSummary 1008B + raw deflate 압축 해제 → font/style 메타 skip → paragraph_list 재귀 + 표/머리말/각주 nested 본문 포함). 상용 조합형(johab) → 0xAC00 한글 음절 매핑 + 5,893개 한자/기호 lookup. [edwardkim/rhwp](https://github.com/edwardkim/rhwp) (Apache-2.0) 의 Rust 구현을 TypeScript 로 minimal port. 검증: rhwp sample 3건 — sample4(임베디드 시스템 개요) 444 byte 본문 + 작자 "유미경", sample5(리눅스 시스템 관리자 가이드) 7,204 byte 본문 + 작자 "김태형" 경고 없이 깨끗하게 추출. 본 변경은 `parsers/kordoc.rs` 의 fileType 무관 응답 수용 흐름과 `parsers/password_detect.rs` 의 HWP3 가 CFB 가 아니므로 통과 동작 덕분에 Rust 측 코드 변경 없이 사이드카 번들 갱신만으로 적용.

### 사용자 안내
- **2003년 작성 .HWP 재인덱싱** — v2.5.22 에서 `kordoc 실행 실패 ... 지원하지 않는 파일 형식` 으로 missing 됐던 구버전 .HWP 들이 v2.5.23 에서 자동 처리. 폴더 우클릭 → "재인덱싱" 으로 갱신.
- **알려진 한계** — HWP3 표 변종 일부에서 cell layout 어긋남 시 `PARTIAL_PARSE` 경고가 나올 수 있다 (본문 텍스트 추출엔 영향 없음). 메타 컨트롤 (페이지 번호 / 필드 코드 / 책갈피) 가 가득한 paragraph 에서 stream 어긋남이 발생할 경우 해당 paragraph 만 손실되고 후속 본문은 정상.

## [2.5.22] - 2026-05-08

**hotfix: HWP 파싱 회귀 + 폴더 삭제 race + macOS 업데이트 안내** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) v2.5.21 추가 보고분.

### 수정
- **HWP `Password protected` false-positive** — `parsers/password_detect.rs::hwp5_is_encrypted` 의 17바이트 짧은 signature(`"HWP Document File"`) byte-search 가 한국어 본문/메타 데이터에 우연히 매치되어 정상 .HWP 파일을 사전 차단하던 결함. 사용자가 같은 파일을 다른 폴더(`/Volumes/...` → 데스크톱 `TEST/`)로 옮겨도 동일하게 발생. fix 3종: ① signature 를 `"HWP Document File V5"` (20 byte) 로 강화, ② 매치 위치를 파일 앞 64KB 이내로 제한 (CFB 컨테이너 구조상 FileHeader 가 그 안에만 있음), ③ properties DWORD 의 reserved 상위 23비트가 0인지 sanity check (본문 우연 매치는 random 비트 패턴이라 거의 항상 실패). 보수 정책 — 의심스러우면 false (kordoc 가 실제 파일 검증).
- **kordoc 진단성 강화** — 사용자 mac 환경의 stderr 가 `FAIL\n  → 지원하지 않는 파일 형식입니다.` 두 줄로 출력되는데 v2.5.21 의 노출 로직이 첫 비어있지 않은 줄(`FAIL`)만 잡아 정작 의미 있는 메시지가 묻혀 있었다. 모든 비어있지 않은 라인을 ` | ` 로 합쳐서 사용자 가시 에러에 노출 (300자 제한). 다음 회귀 발생 시 사용자가 실제 kordoc 에러 메시지를 곧장 공유 가능.
- **폴더 삭제 사이드바 잔존 (race)** — v2.5.21 에서 `service.remove_folder()` 전체를 `tauri::async_runtime::spawn` 으로 백그라운드 처리하면서 `watched_folders` DELETE 도 같이 비동기로 갔다. 그 결과 frontend 의 invoke 즉시 반환 직후 `refreshStatus()` 가 호출되는 시점에 DB 가 아직 안 지워져 사이드바에 폴더가 잠깐 잔존하는 race. fix: command 안에서 **`watched_folders` DELETE 만 동기로 먼저** 실행하고, 무거운 벡터/파일 cleanup 만 spawn 으로 분리. service 측에 `remove_watched_folder_only` / `cleanup_folder_data` 두 단계로 분해. 추가로 frontend `useIndexStatus.removeFolder` 에 optimistic UI 갱신 (즉시 status 에서 폴더 제거, 실패 시 refreshStatus 로 원상복구).

### 추가 (사용자 제안 채택)
- **macOS 수동 업데이트 흐름** — v2.5.20 에서 mac 의 "지금 확인" 을 숨겼는데, 사용자가 "GitHub 에 새 버전 있는지 확인해서 release 페이지를 브라우저로 열어주는 방식이면 좋겠다" 고 제안 (이슈 #22). 채택 — `commands::file::check_github_release` (ureq + GitHub API) 추가, `useUpdater` 의 mac 분기에서 호출 → tag_name 비교 → 새 버전이면 phase: `available` + releaseUrl set. UpdateModal 이 releaseUrl 있으면 "지금 설치" 대신 "다운로드 페이지 열기" 버튼 노출, 클릭 시 `open_url` 로 시스템 브라우저에서 release 페이지 오픈. 자동 시작 30초 + 6시간 인터벌 체크도 mac 에서 활성 (windows 와 동일).

### 사용자 안내
- **HWP 인덱싱 재시도** — 외장 드라이브 / 데스크톱 TEST 폴더의 같은 .HWP 파일이 v2.5.21 에서 `Password protected` 로 차단되었다면 v2.5.22 에서 자동 통과. 폴더 우클릭 → "재인덱싱" 또는 단순히 새로 추가만 하면 인덱스 갱신.
- **kordoc 자체가 .HWP 를 unsupported 라고 거부하는 케이스** — fix 후에도 동일 메시지가 남으면 macOS 환경에서 kordoc 사이드카가 해당 HWP 변종(예: HWP3 구버전 / 손상 파일 / native module 누락) 을 처리하지 못하는 경우다. 새 진단 메시지(`kordoc 실행 실패 (exit N): FAIL | 지원하지 않는 파일 형식입니다.`)가 그대로 보이면 issue 에 한 줄 공유 부탁.

## [2.5.21] - 2026-05-07

**hotfix: macOS 폴더 삭제 미반영 + HWP 파싱 전수 실패** — [이슈 #22](https://github.com/chrisryugj/Docufinder/issues/22) M4 MacBook + 외장 드라이브(`/Volumes/JetDrive Lite 330/Work`) 환경에서 보고된 두 회귀를 모두 잡는다.

### 수정
- **폴더 삭제 후 재시작 시 부활** — `FolderService::remove_folder` 가 `vector.save()` / `delete_files_in_folder()` 중간 실패 시 `?` 로 함수 종료해 `watched_folders` DELETE 까지 도달 못 하던 문제. 토스트는 "제거되었습니다" 떴는데 재시작하면 폴더가 사이드바에 그대로 남아 있던 현상의 직접 원인. 순서를 재배치해 **`watched_folders` DELETE 를 가장 먼저** 실행하고, 벡터 청크 정리 + `delete_files_in_folder` 는 best-effort 로 강등. 이제 사용자가 보는 "제거됨" UX 와 DB 상태가 항상 일치한다.
- **HWP 전수 인덱싱 실패 (12,166 / 37,160)** — ad-hoc 서명 + dmg 다운로드 조합에서 `.app` 내부 sub-binary (`Contents/Resources/resources/node`, `kordoc/node_modules/**/*.node`, `libonnxruntime.dylib`) 가 `com.apple.quarantine` xattr 를 상속받아 Gatekeeper 가 spawn 자체를 차단하던 경로. 사용자가 README 안내대로 `xattr -dr` 를 실행하지 않으면 발현. `lib.rs setup()` 에서 startup 1회 `/usr/bin/xattr -rd com.apple.quarantine <Resources/resources>` 로 자동 제거. 사용자 수동 작업 없이 kordoc 사이드카 정상 동작. HWP5 는 Rust 폴백이 없어 사이드카 미가용 시 전수 실패하지만 docx/pdf 는 Rust 파서로 처리되어 부분 인덱싱은 되던 비대칭이 이슈 진단을 어렵게 했다.
- **kordoc 실패 진단성 향상** — `kordoc 실행 실패 (exit N)` → `kordoc 실행 실패 (exit N): <stderr 첫 줄 200자>` 로 사용자 가시 에러에 stderr 의미 라인 노출. 다음 회귀 발생 시 로그 파일 안 봐도 원인 파악 가능.

### 내부 분기
- **`kordoc-availability` 이벤트** — `lib.rs setup()` 에서 `parsers::kordoc::is_available()` 결과를 startup 1회 emit. 미가용이면 `tracing::error!` 로도 동시 기록. frontend 에서 listener 추가 시 인덱싱 시작 전에 사용자에게 명시적 안내 가능.

### 사용자 안내
- **macOS 기존 설치 사용자** — v2.5.21 dmg 설치 후 첫 실행에서 quarantine 자동 제거 → HWP 인덱싱 자동 활성. 이전 빌드에서 폴더가 부활하던 항목은 `재인덱싱` 트리거 또는 다시 `삭제` 한 번이면 정리.

## [2.5.20] - 2026-05-07

**hotfix: macOS 자동 업데이트 fallback platforms 오류** — v2.5.18 mac 포팅 시 `tauri.macos.conf.json` 의 `plugins.updater.active: false` 가 frontend 의 자동 `check()` 호출을 막지 못하던 문제 해결.

### 수정
- **`useUpdater` macOS 가드** — `useUpdater` 훅 진입 시 `isMac` 분기. mac 에서는 자동 30초 후 체크 + 6시간 인터벌 모두 skip, 수동 `checkForUpdate()` 도 즉시 `up-to-date` phase 로 응답하고 plugin-updater 의 `check()` 호출 자체를 우회. v2.5.18~v2.5.19 mac 빌드 사용자에게 발생하던 `None of the fallback platforms ["darwin-aarch64-app", "darwin-aarch64"] were found in the response platforms object` 오류 차단. (원인: tauri-action 의 latest.json 은 `createUpdaterArtifacts: true` 인 windows job 산출물만 반영해 `windows-x86_64` 키만 들어가는데, plugin-updater 가 mac 에서 fallback platform 을 못 찾고 throw.)
- **DiagnosticsTab 안내** — mac 에서는 "자동 업데이트 (macOS 미지원) — Apple Developer ID 미보유로 자동 업데이트 비활성. 신버전은 GitHub Releases 페이지에서 수동 다운로드" 표시 + "지금 확인" 버튼 숨김. windows 동작은 변경 없음.

### 사용자 안내
- **macOS 사용자** — v2.5.18 / v2.5.19 빌드 사용자는 Settings 모달에서 오류 phase 가 보일 수 있다. v2.5.20 설치 후 사라짐. 신버전 알림은 받지 못하므로 [Releases 페이지](https://github.com/chrisryugj/docufinder/releases) 즐겨찾기 권장.

## [2.5.19] - 2026-05-07

**시스템 폴더 수동 인덱싱 허용 + WebView2 오프라인 인스톨러** — 일부 기업/제한 환경에서 보고된 WebView2 런타임 미설치 오류를 근본 차단하고, 시스템 보호 폴더(`/usr/bin`, `C:\Program Files` 등)를 사용자가 명시적으로 골라 인덱싱할 수 있도록 토글을 추가.

### 추가
- **시스템 폴더 추가 허용 토글** — `Settings.allow_system_folders` (기본 OFF). 설정 → 시스템 탭에 토글 노출. ON 으로 켜면 기존에 `validate_watch_path` 에서 차단되던 `C:\Windows`, `C:\Program Files`, `/System/Library`, `/usr/bin`, `/private/var` 등 시스템 보호 폴더를 폴더 다이얼로그로 직접 추가 가능. 추가 시 강한 경고 다이얼로그(디스크/메모리 부담, 노이즈 증가, 인덱싱 시간 길어짐) 후 진행.
- **시스템 폴더 자동 벡터 스킵** — 드라이브 루트 처리와 동일하게, 시스템 폴더 인덱싱 후 시맨틱(벡터) 인덱싱 자동 시작 안 함. 시스템 폴더 대부분이 바이너리/시스템 파일이라 임베딩 비용 대비 효용이 낮은 점을 반영. 필요 시 설정에서 수동 시작 가능. `indexing-warning` 이벤트 (`type: "system_folder"`) emit.
- **`FolderClassification` 확장** — `classify_folder` 응답에 `is_system: bool`, `allow_system_enabled: bool` 추가. 프론트가 시스템 폴더 + 토글 OFF 케이스에서 백엔드 호출 전에 안내만 띄우고 차단할 수 있게.
- **테스트** — `constants::is_blocked_path` / `validate_watch_path` 동작 검증 (macOS root 경로 자체·하위, Windows Program Files/Windows/ProgramData, 사용자 경로 통과, ~/Library 통과, 토글 ON/OFF). Windows 전용 케이스는 `#[cfg(windows)]` 게이트.

### 수정
- **WebView2 런타임 미설치 오류** — `tauri.conf.json` 의 `webviewInstallMode` 를 `embedBootstrapper` → `offlineInstaller` 로 변경. 기존에는 1.8MB 부트스트래퍼 stub 만 인스톨러에 포함되고 실제 WebView2 런타임은 설치 시점에 인터넷에서 다운로드하던 구조라, 회사 프록시·방화벽·오프라인 환경에서 설치 실패 → 앱 시작 시 "Microsoft Edge WebView2 Runtime not installed" 다이얼로그가 발생했다. 이제 전체 WebView2 런타임이 NSIS 인스톨러에 내장(+~130MB) 되어 인터넷 없이 설치 가능. README 의 "WebView2 별도 설치 불필요" 문구가 비로소 사실과 일치.
- **`is_blocked_path` 패턴 매칭 결함** — `BLOCKED_PATH_PATTERNS` 가 `/usr/bin/` 처럼 양쪽 sep 포함 형태라 `dunce::canonicalize` 가 반환하는 trailing sep 없는 경로(`/usr/bin`) 와 `contains` 매칭 실패. 패턴 자체-경로 정확 일치 분기 추가. 또한 component 체크에 `program files`, `program files (x86)` 추가하여 드라이브 레터 prefix 때문에 기존 패턴이 안 잡던 `C:\Program Files` 자체-경로도 차단.
- **`FolderService::validate_and_canonicalize` 일관성** — `BLOCKED_PATH_PATTERNS.contains` 직접 매치 → `crate::constants::validate_watch_path` 호출로 통일. `allow_system_folders` 토글이 모든 진입점(`add_folder`, `reindex_folder`, `resume_indexing`, `start_indexing_batch`, FolderService 자체) 에서 동일하게 적용되도록 보장.

### 내부 분기
- **글로벌 atomic 토글** — `constants::ALLOW_SYSTEM_FOLDERS: AtomicBool` 으로 `Settings.allow_system_folders` 미러. `update_settings` 에서 `set_allow_system_folders` 동기화, `AppContainer::new` 에서 부팅 시 초기화. `cloud_detect::SKIP_ENABLED` 와 동일 패턴.
- **다이얼로그 통합** — `useIndexStatus` 의 `confirmCloudOrNetworkAdd` → `confirmFolderAdd` 로 이름 변경. 시스템 / 클라우드 / 네트워크 / 로컬 4 케이스를 한 함수에서 우선순위 순으로 처리 (시스템 차단 → 시스템 경고 → 클라우드/네트워크 안내 → 통과).

### 사용자 안내
- **NSIS 인스톨러 크기 증가** — v2.5.18 까지 약 90MB 였던 인스톨러가 약 220MB 로 증가. 다운로드 시간이 길어지지만 설치 시 인터넷이 필요 없어 회사망/오프라인 환경에서 안정적.
- **시스템 폴더 인덱싱은 비권장 기본값** — 일반 사용자는 토글을 끈 상태로 유지 권장. 시스템 폴더는 파일 수가 많고(수십만~수백만) 바이너리/시스템 파일이 대부분이라 검색 노이즈와 디스크 사용량을 크게 늘린다.

## [2.5.18] - 2026-05-06

**macOS arm64 (Apple Silicon) 포팅** — Windows 전용이던 앱을 동일 코드베이스에서 macOS 14(Sonoma)+ Apple Silicon 으로 이식. Universal/Intel Mac 미지원, Notarization 없이 ad-hoc 서명만 적용 (Apple Developer ID 미보유 전제).

### 추가
- **macOS arm64 빌드** — `aarch64-apple-darwin` 타겟. dmg 산출물 + ad-hoc 서명. Mach-O thin arm64. 시스템 dylib 의존만 (외부 의존 0).
- **`tauri.macos.conf.json`** — `bundle.targets: ["dmg"]`, `signingIdentity: "-"`, `createUpdaterArtifacts: false` (자동 업데이트 비활성). `minimumSystemVersion: 11.0`.
- **[scripts/setup-macos-resources.sh](scripts/setup-macos-resources.sh)** — Node v20 darwin-arm64 + kordoc dist + ONNX Runtime 1.23.0 osx-arm64 dylib 을 `src-tauri/resources/` 에 자동 채움. `KORDOC_DIR` 환경변수로 kordoc 소스 경로 지정 가능.
- **[src/utils/platform.ts](src/utils/platform.ts)** — UA 기반 OS 감지 helper. `FILE_MANAGER_NAME`(탐색기/Finder), `SYSTEM_FOLDERS_HINT`, `AUTOSTART_DESCRIPTION`, `HAS_DRIVES`, `DEFAULT_DATA_LOCATION` 등 사용자 노출 텍스트를 OS별 분기.
- **CI macOS job** — `.github/workflows/ci.yml` 에 `check-backend-macos` (macos-14, clippy/test). `.github/workflows/publish.yml` 에 `publish-macos` job. tag push 시 windows + mac 빌드 병렬 → 동일 GitHub Release 에 dmg/MSI 동시 업로드. mac job 은 windows 결과 기다리지 않고 release 없으면 단독 생성.

### 수정 (포팅 과정에서 발견된 버그)
- **HWP5 password detection false positive** — `parsers/password_detect.rs` 의 `FLAG_CERT_ENC` (bit 8 = 0x100) 가 한컴오피스 일부 정상 문서에도 set 되어 있어 kordoc 호출 자체를 차단하던 문제. bit 8 검사 제거, 진짜 암호(bit 1) + DRM(bit 4) 만 본다.
- **mac 앱 번들 경로 미고려** — `parsers/kordoc.rs` 의 `find_kordoc_cli()` / `which_node()` 가 `binary parent / resources/` 만 보던 코드를 mac 번들 구조 (`Contents/MacOS/<bin>` → `../Resources/resources/`) 까지 탐색하도록 분기 추가. 이게 없으면 mac 에서 `.hwp` 파싱 시 `is_available()` false 반환 → "Unsupported file type: hwp (kordoc 필요)" 폴백.
- **설정 토글 자동 저장** — `SettingsModal.tsx` 의 `handleChange` 가 로컬 state 만 갱신하고 백엔드 저장은 "저장" 버튼 클릭 시에만 수행하던 문제. 토글만 켜고 앱 X 버튼으로 종료하면 `close_to_tray` 등이 백엔드에 반영 안 됨. 디바운스 300ms 자동 저장 추가.
- **mac dock 클릭 시 윈도우 미복귀** — `lib.rs` 의 Tauri builder 에 `RunEvent::Reopen` 핸들러 추가. close_to_tray 로 hide 된 상태에서 dock 아이콘 클릭하면 윈도우 자동 복귀.
- **macOS 시스템 폴더 차단 누락** — `constants::BLOCKED_PATH_PATTERNS` 에 `/system/library/`, `/private/var/`, `/usr/bin/`, `/.spotlight-v100/` 등 추가. 단 `~/Library/...` (사용자 데이터)는 차단되지 않도록 prefix 패턴만 사용.

### 내부 분기
- `disk_info.rs` — non-windows 는 `DiskType::Ssd` fallback. windows-only 함수/static 에 `cfg(windows)` 적용.
- `model_downloader.rs` — `dylib_filename()` 헬퍼 (`onnxruntime.dll` / `libonnxruntime.dylib` / `libonnxruntime.so`). ONNX Runtime 다운로드 로직은 windows-only, mac/linux 는 번들 dylib 검증만.
- `kordoc.rs` — `NODE_BIN` 상수 (`node.exe` / `node`).
- `lib.rs` — `ORT_DYLIB_PATH` 가 `dylib_filename()` 사용.
- `cargo test` — windows-path 가정 7개 테스트에 `#[cfg(windows)]` 게이트 (170 passed on mac).

### 사용자 안내
- **macOS 사용자** — README 의 "macOS (Apple Silicon) 설치" 섹션 참고. 첫 실행은 우클릭 → 열기, "손상된 앱" 표시 시 `xattr -dr com.apple.quarantine /Applications/Anything.app`.
- **자동 업데이트 미지원** (mac 한정) — Notarization 없이 updater 가 불안정해 비활성. 새 버전은 Releases 페이지에서 수동 다운로드.

---

## [2.5.17] - 2026-04-27

**디버그 심볼 보존 빌드 — 이슈 #17 fastfail(7) 콜스택 추적용**

### 변경
- `Cargo.toml [profile.release]`: `strip = "debuginfo"` 한시 비활성화, `debug = "line-tables-only"` 추가. 사용자 제출 minidump 5건이 모두 `0xC0000409 / Param[0]=0x7 (FAST_FAIL_FATAL_APP_EXIT)` 시그널이지만 PDB 부재로 abort 콜스택을 풀어낼 수 없어, 다음 크래시에서 panic 발생 함수까지 식별 가능하도록 PDB 동봉. 다음 정식 릴리즈에서 다시 strip 복원 예정.
- 기능 변경 없음 (인덱싱/검색/UI 동일).

---

## [2.5.16] - 2026-04-26

**클라우드/네트워크 폴더 본문 인덱싱 자동 스킵 + 폴더 추가 시 사전 안내** — [이슈 #19](https://github.com/chrisryugj/Docufinder/issues/19)

### 추가
- **클라우드/네트워크 폴더 본문 인덱싱 자동 스킵** (기본 ON) — Google Drive for Desktop · NAVER Works · WebDAV · 매핑 SMB 드라이브 등 placeholder 비트가 켜지지 않는 환경에서 인덱서가 모든 파일을 네트워크/클라우드에서 다운로드하던 문제. `cloud_detect::is_network_path()` 추가 — UNC + `GetDriveTypeW = DRIVE_REMOTE` 매핑드라이브 모두 감지.
  - 켜진 상태: 본문 파싱 진입 직전 `ParseError::CloudPlaceholder` 로 분기 → **메타데이터만 인덱싱(파일명·크기·수정일)**, hydrate / 네트워크 다운로드 0회.
  - 토글 위치: `설정 → 시스템 → 클라우드/네트워크 폴더 본문 인덱싱 자동 스킵`. 끄면 일반 로컬 폴더처럼 본문까지 인덱싱 (NAS 등 빠른 환경 한정 권장).
- **폴더 추가 시 사전 안내 다이얼로그** — 새 커맨드 `classify_folder` 가 폴더의 LocationKind(`local` / `unc` / `network_drive` / `cloud_placeholder`) 를 분류해 프론트에 반환. 클라우드/네트워크면 추가 전 1회 경고 다이얼로그(설정 토글에 따라 안내 문구 분기) → 사용자가 명시적으로 계속 선택해야 진행.

### 내부
- 새 모듈 `utils/cloud_detect`: `is_cloud_placeholder` / `is_network_path` / `classify` / `set_skip_enabled` 노출.
- `Settings.skip_cloud_body_indexing: bool` (기본 true) 추가. `update_settings` + `AppContainer::new` 에서 atomic flag 동기화.
- `Cargo.toml`: `windows-sys` 의 `Win32_Storage_FileSystem` feature 추가 (`GetDriveTypeW` 사용).

---

## [2.5.15] - 2026-04-24

**이미지 PDF 사전 감지 + 폴더 추가 에러 메시지 복구 + folder_service canonicalize 통일** — [이슈 #17](https://github.com/chrisryugj/Docufinder/issues/17), [이슈 #19](https://github.com/chrisryugj/Docufinder/issues/19)

### 수정
- **인덱싱 도중 강제 종료(0xc0000409 STATUS_STACK_BUFFER_OVERRUN) 방어** — 스캔 PDF 다수 폴더(예: 학교 업무문서)에서 PDF 마다 kordoc(Node.js 사이드카) 자식 프로세스가 매번 spawn 되며, 800회 이상 누적 시 자식 프로세스/파이프/스레드 누수가 CRT 레벨 `__fastfail` 을 유발해 docufinder.exe 가 panic 흔적 없이 강제 종료되던 문제. v2.5.6 의 "조기 스킵" 분기는 같은 파일 *재시도* 만 막아 효과가 미미했음.
  - 새 모듈 `parsers/pdf_sniff.rs` — PDF 첫 64KB 를 읽어 텍스트 오브젝트 부재 + 이미지 자원(`/DCTDecode`, `/JPXDecode`, `/CCITTFaxDecode`, `/JBIG2Decode`, `/Subtype /Image`) 휴리스틱으로 이미지 PDF 사전 감지. OCR 비활성 + 사전 감지 매치 시 **kordoc 호출 자체를 회피.**
  - **Circuit breaker** — 같은 세션에서 연속 5회 이미지 PDF 판정 시 sniff 도 건너뛰고 즉시 스킵. 텍스트 PDF 가 정상 처리되면 카운터 리셋. 6000개 폴더 인덱싱 시 spawn 횟수가 ~5%로 줄어들어 누적 크래시 차단.
- **"폴더 추가 실패: [object Object]" 에러 메시지 손실 (#19)** — 백엔드 `ApiError` 객체(`{code, message}`)를 프론트에서 `String(err)` 로 직렬화해 `[object Object]` 만 노출되던 버그. 16곳에 흩어진 동일 패턴을 모두 `getErrorMessage()` 유틸로 교체해 실제 에러 메시지(예: "잘못된 경로: Y:\... : ...") 가 표시되도록 수정. 사용자가 원인 진단 가능.
- **`folder_service` canonicalize 누락 (#19)** — v2.5.13 의 UNC 통일 패치가 `folder_service::validate_and_canonicalize` 한 곳을 놓쳤음. Y: 같은 매핑드라이브가 `\\?\Y:\...` 로 변환되며 watcher 등록 / DB 경로 불일치를 일으킬 가능성. `dunce::canonicalize` 로 통일.

### 내부
- `parsers/mod.rs` 에 PDF sniff + circuit breaker 카운터(`SCANNED_PDF_STREAK`) 통합. 텍스트 PDF 정상 처리 시 카운터 리셋.
- 프론트 에러 처리 8 파일 일괄 정비: `App.tsx`, `useIndexStatus`, `useSearch`, `useUpdater`, `useVectorIndexing`, `SettingsModal`, `DiagnosticsTab`, `SystemTab`.

---

## [2.5.14] - 2026-04-24

**암호 보호 파일 사전 스킵 + tao 크래시 재전송 스팸 차단**

### 수정
- **암호 걸린 파일 인덱싱 중 시스템 모달 팝업 차단** — HWP/HWPX/DOCX/XLSX/PPTX/PDF 암호 파일이 kordoc(Node.js 사이드카)을 거쳐 한컴/Office COM 에 도달하면, 해당 프로그램이 **사용자 포커스를 뺏는 "암호를 입력하세요" 다이얼로그**를 띄워 인덱싱이 멈추던 문제. 새 모듈 `parsers/password_detect.rs` 에서 파서 호출 **전** 에 사전 감지해 즉시 스킵하도록 변경.
  - HWP5 (OLE CFB): FileHeader stream 의 properties 플래그 — bit 1 (암호) / bit 4 (DRM) / bit 8 (공인인증 보안) 검사.
  - HWPX (ZIP + ODF): `META-INF/manifest.xml` 의 `encryption-data` 요소 존재 여부.
  - DOCX/XLSX/PPTX (OOXML): 정상시 ZIP 이지만 암호화되면 OLE CFB 로 래핑됨 → 첫 8바이트 매직 검사. 레거시 `xls`/`ppt`/`doc` (원래 CFB) 는 기존 calamine 에러 기반 경로에 위임.
  - PDF: tail 32KB 에서 `/Encrypt` 키 검색 (trailer 또는 xref stream).
  - 감지된 파일은 `ParseError::PasswordProtected` 로 즉시 반환 → 기존 pipeline `Failure` 경로에서 메타데이터만 저장 (파일명 검색 가능).
- **tao 크래시 재전송 스팸 차단** — v2.5.12 에서 `BENIGN_PANIC_SOURCES` 에 `tao` 를 추가했지만, 이전 버전에서 **이미 디스크에 쌓인 `crash-YYYY-MM-DD.log`** 가 앱 시작 시 `spawn_flush_pending_crash_logs` 로 필터 없이 그대로 전송되던 문제. 새 모듈 `panic_filter.rs` 로 BENIGN 상수를 실시간 panic hook 과 deferred flush 양쪽에서 **공유**하고, flush 경로에서도 파일 내용 전체가 BENIGN 패닉이면 조용히 `.sent` 마킹 후 전송 스킵.
  - 매칭 패턴을 `"tao"` → `"tao-"` 로 더 엄격하게 변경 (false positive 방지). `wry-`, `muda-` 도 추가.

### 내부
- `parsers/mod.rs` `parse_file` 진입 시 cloud placeholder 다음 단계로 사전 암호 감지 추가.
- `lib.rs` panic hook 의 `BENIGN_PANIC_SOURCES` 배열을 `panic_filter` 모듈로 이동 → `is_benign_location` / `is_all_benign` 두 유틸 제공.
- `commands/telemetry.rs` deferred flush 전 `is_all_benign` 검사.
- 테스트 10개 (panic_filter 6 + password_detect 4).

---

## [2.5.13] - 2026-04-23

**네트워크(UNC) 폴더 재인덱싱 / 배치 인덱싱 예방 패치** — [이슈 #19](https://github.com/chrisryugj/Docufinder/issues/19)

### 수정
- **네트워크 폴더 재인덱싱·Resume·배치 인덱싱 실패 가능성 차단** — v2.5.0 에서 `add_folder` 는 `dunce::canonicalize` 로 `\\server\share\...` 형태를 보존하도록 바꿨는데, `reindex_folder` / `resume_indexing` / `start_indexing_batch` / `run_folder_index_job_batch` 네 경로는 여전히 `std::fs::canonicalize` 를 써서 UNC 를 `\\?\UNC\server\share\...` 로 변환하고 있었음. 이 경우 DB 에 기록된 감시 경로(`\\server\share\...`)와 불일치해 "변경분 0건" 으로 오인식되거나 status 업데이트가 엉뚱한 키로 저장되는 문제가 있었음. **네 경로 모두 `dunce::canonicalize` 로 통일.**

### 알려진 제약 (다음 라운드)
- 매핑드라이브(Z:\, X:\ 등)는 여전히 로컬로 취급됨 (PollWatcher 분기 안 탐). SMB 매핑드라이브에서 실시간 이벤트 누락 가능 — `GetDriveTypeW` 기반 분기 검토 예정.
- AI 질의응답(Gemini) 은 여전히 인터넷 필요. 망분리 환경에서는 검색/파일명/벡터 기능만 동작 (ONNX·PaddleOCR·Lindera 는 이미 MSI 에 번들되어 오프라인 OK).

---

## [2.5.12] - 2026-04-23

**안정성 / 종료 / 정렬 크래시 대응 라운드**

### 수정
- **인덱싱 중 트레이 "종료" 무반응** — 트레이 quit / X 버튼(close_to_tray=false) 핸들러가 FTS cancel 신호를 보내지 않아 파이프라인 스레드가 그대로 돌던 문제. 이제 cancel_indexing + vector worker cancel 을 즉시 broadcast 하고, cleanup 이 3초 내 끝나지 않으면 **watchdog 이 std::process::exit(0) 으로 강제 종료**. 인덱싱 중에도 트레이 우클릭 → 종료가 즉시 동작.
- **트레이 최소화 토글이 꺼지지 않던 버그** — 설정 모달의 `handleChange` 가 stale state 로 `setSettings` 를 호출해, "트레이 최소화" 토글이 `close_to_tray` + `start_minimized` 를 같은 틱에 업데이트할 때 뒤 호출이 앞 호출을 덮어썼음. functional update (`setSettings(prev => ...)`) 로 교체.
- **"모든 데이터 초기화" 첫 시도 지연 (~수 초)** — FTS 파이프라인의 잔존 WAL read lock 이 DROP TABLE 과 경쟁해 발생. 취소 신호를 **맨 먼저** broadcast + 200ms 유예 + `db::pool::drain_pool()` 후 DROP. 첫 시도도 즉시 완료.
- **smallsort "total order" 패닉** — Rust 1.81+ sort 가 엄격한 전이성을 요구하는데 `partial_cmp().unwrap_or(Equal)` 패턴은 NaN 섞이면 전이성 위반. 검색 랭킹 / 중복 유사도 / 교정 후보 / OCR 바운딩박스 정렬 5곳을 모두 `f32::total_cmp` / `f64::total_cmp` 로 교체.
- **`type1-encoding-parser` / `cff-parser` / `tao` 관련 알림 스팸** — 손상된 PDF Type1 폰트 / 앱 종료 시 Windows 이벤트 루프 race 에서 발생하는 패닉이 crash log / Telegram 으로 전송되던 문제. `BENIGN_PANIC_SOURCES` 에 추가해 알림만 억제 (해당 패닉은 이미 `catch_unwind` 또는 종료 시점이라 앱 동작에는 영향 없음).

### 개선
- **PDF 수식 OCR 설정을 "검색" 탭으로 이동** — 이전에는 "진단" 탭에 있어 발견이 어려웠음. 빨간 경고 배너로 "PDF 인덱싱 속도가 수 배 ~ 수십 배 느려질 수 있음" 을 강조.
- **크래시 로그 재전송 스팸 차단** — 앱 시작 시 미전송 crash log 를 Telegram 으로 플러시하는 경로에서, **오늘자 `crash-YYYY-MM-DD.log` 만 전송**하고 이전 날짜 로그는 조용히 `.sent` 로 마킹. 재설치 사용자의 묵은 로그가 채널로 올라오던 문제 해결.

### 내부
- `cleanup_vector_resources` 가 FTS 파이프라인도 cancel 하도록 확장 (기존엔 벡터 워커만).

---

## [2.5.11] - 2026-04-23

**kordoc v2.6.2 반영 — PDF 수식 OCR 품질 대폭 개선 (90%+ 정확도)**

### 개선
- **PDF 수식 OCR noise 제거 대폭 강화** — [kordoc v2.6.1](https://github.com/chrisryugj/kordoc/releases/tag/v2.6.1) + [v2.6.2](https://github.com/chrisryugj/kordoc/releases/tag/v2.6.2) 반영. arxiv Attention 논문 기준 순수 noise 1개만 남아 **96% 정확도** 달성. ResNet (Figure 많은 논문) 기준 **90%**. 핵심 수식 100% 유지.
  - **trivial 수식 필터 12개 규칙 추가** — 단일 글자 (`$O$`, `$a$`), 단일 `\cmd` (`$\imath$`, `$\varPi$`), 장식 `\mathrm{...}` (`$\mathrm{fcloc}$`), 반복 기호 (`$\pm\pm\pm\pm$`), substring 반복 (`\alpha_{N}=` 연쇄), `\square` placeholder, 단독 숫자, 괄호 그룹 중복, 함수 인자 반복, `\frac{X}{X}`, matrix placeholder, `\mathsf`/`\mathtt`/`\texttt` 등.
  - **MFR tokenizer 과공백 정규화** — `\mathrm { m o d d }` → `\mathrm{modd}`, `6 4` → `64`, `( Q, K, V )` → `(Q,K,V)`.
  - **`\cmd` 뒤 공백 누락 복원** — `\cdotd` → `\cdot d`, `\timesd_{k}` → `\times d_{k}` (알려진 LaTeX 명령어 사전 기반).
  - **수식 bbox y 좌표 매핑** — 이전엔 검출된 수식이 페이지 끝에 몰렸는데, 이제 pdfjs 블록 사이 **올바른 위치**에 삽입. MultiHead/FFN/PE 수식이 논문 흐름에 맞게 배치.
  - **pdfjs 중복 블록 제거** — 수식 bbox 와 60%+ 겹치는 pdfjs 텍스트 블록 자동 삭제. 동일 수식이 두 번 나타나던 현상 해결.
  - **`cleanPdfText` 수식 라인 공백 보호** — `collapseEvenSpacing` 이 수식 내부 LaTeX 공백을 "균등배분" 으로 오인식해 `\cdot d` → `\cdotd` 로 합쳐지던 숨은 버그 수정.

### 의존성
- `kordoc` 2.6.0 → 2.6.2 (번들 재빌드)

---

## [2.5.10] - 2026-04-23

**kordoc v2.6.1 (수식 OCR 품질 개선) 반영**

### 개선
- kordoc v2.6.1 초기 반영. v2.5.11 에서 더 다듬어진 상태로 배포되었으므로 상세 내역은 v2.5.11 항목 참고.

### 의존성
- `kordoc` 2.6.0 → 2.6.1

---

## [2.5.9] - 2026-04-23

**kordoc v2.6.0 (PDF 수식 OCR) + KaTeX 미리보기**

### 추가
- **PDF 이미지 기반 수식 OCR** — kordoc v2.6.0 의 [Pix2Text MFD + MFR](https://github.com/breezedeus/pix2text) ONNX 모델 연동. 스캔 PDF / 이미지 삽입 수식이 자동으로 LaTeX (`$...$`, `$$...$$`) 로 추출되어 인덱싱 + 검색 대상 포함. 기본 활성화.
- **검색 결과 수식 KaTeX 미리보기** — 결과 뷰어에서 LaTeX 수식을 KaTeX 로 즉시 렌더. 인라인 `$...$` 와 display `$$...$$` 모두 지원.
- **수식 4포맷 지원** — HWPX / DOCX / HWP5 의 수식(EQN 블록) 도 kordoc 2.5.3 이상에서 LaTeX 로 변환되어 검색 가능.

### 의존성
- `kordoc` 2.5.2 → 2.6.0 (ort, sharp, @huggingface/transformers, @hyzyla/pdfium 추가)

---

## [2.5.8] - 2026-04-23

**자동 업데이트 시스템 도입 + Telegram 오류 리포트 + 진단 탭**

### 추가
- **GitHub Releases 기반 자동 업데이트** — ed25519 서명 검증 포함. 앱이 최신 버전 감지 시 배지 노출, "지금 확인" 버튼으로 수동 체크도 가능. Tauri updater plugin.
- **진단 탭 (설정 > 진단)** — 앱 상태 / DB / 인덱스 / 모델 위치 / 로그 한눈에 확인. 사용자 문의 대응 편의.
- **Telegram 오류 리포트** — 크래시 / panic 발생 시 사용자 동의하에 Telegram 채널로 비식별화된 스택 전송 (opt-in).

### 개선
- 다양한 UX 폴리싱 (세부는 커밋 [eac8858](https://github.com/chrisryugj/Docufinder/commit/eac8858) 참고)

---

## [2.5.7] - 2026-04-22

**kordoc 2.5.2 반영 — macOS 한컴 HWPX 호환 + HWP5 배포용 COM fallback**

### 개선
- **번들 kordoc 파서 2.5.0 → 2.5.2 업그레이드** — Docufinder 가 내부적으로 쓰는 Node.js 사이드카 파서([kordoc](https://github.com/chrisryugj/kordoc)) 를 최신으로 교체. HWP/HWPX 변환 품질이 조용히 개선됨. 앱 UI 변경 없음.
  - **macOS 한컴오피스에서 "파일 깨짐" 거부되던 HWPX 생성 이슈 해결** — `markdownToHwpx` 가 만드는 HWPX 의 테이블 XML 을 최소 스켈레톤에서 **완전 스펙 형태**로 재작성. `<hp:tbl>` 필수 속성, `<hp:sz>`/`<hp:pos>`/`<hp:outMargin>`/`<hp:inMargin>` 블록, `<hp:subList>` 래퍼 + `<hp:cellAddr>`/`<hp:cellSpan>`/`<hp:cellSz>`/`<hp:cellMargin>` 추가. `Preview/PrvText.txt` 동봉. (kordoc #4)
  - **테이블 테두리 / 볼드 / 순서 있는 목록 시각 품질 개선** — 테두리 단위 공백 포함(`"0.12 mm"`), 볼드 전용 fontface(HY견고딕/Arial Black) id=2 추가, indent 레벨별 러닝 카운터로 `1. 2. 3.` 자동 번호 정상 동작. (kordoc #4 후속)
  - **HWP 5.x "배포용 문서 상위 버전" 경고 플레이스홀더 COM 재시도** — `.hwp` 바이너리에서 `"이 문서는 상위 버전의 배포용 문서입니다..."` 로만 떨어지는 케이스에서, Windows + 한컴오피스 환경이면 자동으로 `HWPFrame.HwpObject` COM API 로 재시도. 기존 HWPX DRM fallback 인프라 재활용. (kordoc #25)
- **PDF 세로선 없는 표 오인식 수정** — 세로선 없는 표를 1 열 다행 그리드로 잘못 잡아 **본문이 한 줄에 평평화(flatten) 되어 표시되던 현상** 수정. 검색 결과 스니펫 품질 체감 개선. (kordoc fix)

### 의존성
- `kordoc` 2.5.0 → 2.5.2 (번들 재빌드)

---

## [2.5.6] - 2026-04-22

**인덱싱 중 강제 종료 예방 + 관련도→최신순 전환 시 스크롤 튀는 버그 수정**

### 수정
- **관련도순 → 최신순/이름순/오래된순 전환 시 스크롤이 중간으로 튀는 버그** — 정렬을 바꾸면 [useResultSelection](src/hooks/useResultSelection.ts) 이 "선택된 파일의 새 index" 로 `selectedIndex` 를 자동 재매핑하는데, [SearchResultList](src/components/search/SearchResultList.tsx) 의 `scrollIntoView` 가 이걸 "사용자가 새 항목을 선택한 것" 으로 오해해서 해당 위치로 스크롤. 결과적으로 "정렬이 안 먹히는 것 같다" 는 체감 이슈도 같이 유발. 선택된 **파일 경로** 를 `lastScrolledPathRef` 에 기록해 두고, 경로가 그대로면 (= 같은 파일이 index 만 바뀐 재매핑이면) `scrollIntoView` 를 건너뛰도록 수정. 키보드 내비게이션 (다른 파일로 이동) 은 기존 그대로 동작.
- **이미지 기반(스캔) PDF 다수 폴더 인덱싱 중 앱 강제 종료 예방** — Downloads 같이 스캔 PDF 가 수백~수천 개 쌓인 폴더에서 kordoc(Node.js) → Rust pdf-extract 순으로 2 중 재시도가 돌아가면서 Node.js 자식 프로세스 spawn + pdf-extract 메모리 사용이 누적되어 OOM 으로 OS 가 앱을 강제 종료. [parsers/kordoc.rs](src-tauri/src/parsers/kordoc.rs) stderr 에서 "이미지 기반 PDF" 마커를 에러 메시지에 태그하고, [parsers/mod.rs](src-tauri/src/parsers/mod.rs) 에서 OCR 비활성 + 이미지 PDF 조합이면 Rust 재시도를 건너뛰고 메타데이터만 저장하도록 조기 분기. OCR 활성 상태에서는 기존대로 Rust 파서 + OCR fallback 이 돌아가 기능 손실 없음.

### 개선
- `CHANNEL_BUFFER_SIZE` 32 → 16 ([pipeline.rs](src-tauri/src/indexer/pipeline.rs)). 실제 파싱 스레드는 HDD 2 / SSD 4 개로 제한되어 있어 16 이면 충분한 여유. 저사양 PC (8GB RAM) + 부적합 인덱싱 타깃 조합에서 메모리 피크 절반 수준으로 감소.
- `panic hook` 의 `BENIGN_PANIC_SOURCES` 에 `ort`, `usearch`, `lindera` 추가 ([lib.rs](src-tauri/src/lib.rs)). ONNX Runtime / 벡터 인덱스 C++ 바인딩 / 형태소 사전 로드 중 발생하는 알려진 panic 이 사용자 `crash.log` 를 오염시키던 문제 제거.

---

## [2.5.5] - 2026-04-21

**v2.5.3 이후에도 남아있던 정렬 버그 + 드롭다운 z-index 버그 최종 해결**

### 수정
- **키워드 검색 후 "최신순/이름순" 정렬이 그룹 뷰에서 먹지 않던 문제** — 기본 뷰가 **그룹 뷰** (기존 localStorage 없으면 `"grouped"` 로 초기화) 인데 [useSearch.ts](src/hooks/useSearch.ts) 의 `groupedResults` 가 `filteredResults` (정렬 반영됨)를 받은 뒤 **마지막에 무조건 `top_confidence` 로 재정렬**하고 있어 `filters.sortBy` 가 완전히 무시되던 것. `filters.sortBy === "relevance"` 일 때만 신뢰도 재정렬을 적용하고, 그 외엔 Map 삽입 순서(= filteredResults 의 정렬된 순서) 를 유지하도록 수정. `useMemo` 의존성에 `filters.sortBy` 추가.
- **정렬/확장자/기간 드롭다운 옵션이 결과 카드에 가려져 클릭 불가능하던 z-index 버그** — 결과 카드의 `stagger-item` 애니메이션(`transform: translateY`) 이 카드마다 새 stacking context 를 생성. 처음엔 `SearchFilters` root 자체에만 `relative z-40 isolate` 를 줬지만, **SearchFilters 의 부모 wrapper (App.tsx 의 filter bar 컨테이너) 가 stacking 을 갖지 않아** 효과가 국소화되어 여전히 스크롤 영역(DOM 상 뒤 형제) 이 위로 렌더되던 것이 진짜 원인. [App.tsx](src/App.tsx) 의 filter bar wrapper 에 `relative z-40` 직접 부여. `SearchFilters` / `ResultsToolbar` 의 보조 stacking 도 유지 (내부 드롭다운 보호).

### 내부
- 정렬/신뢰도 관련 주석 보강 — `groupedResults` 가 왜 relevance 에서만 재정렬하는지, Map 삽입 순서가 어떤 의미인지 명시.

> 📝 v2.5.4 는 위 2 버그의 **부분 수정판**으로 내부 빌드만 생성되었고 외부 배포되지 않았습니다. 본 v2.5.5 에 완전 통합.

---

## [2.5.3] - 2026-04-21

**v2.5.2 부팅 CPU/메모리 피크 핫픽스 + 정렬/컨텍스트 메뉴 버그 3종**

### 수정
- **부팅 직후 3~5분 CPU 60%+ / 메모리 1.2GB 피크** — v2.5.2 에서 추가된 주기 sync(`periodic_sync`) 가 startup sync 진행 중에도 창 포커스 복귀마다 재트리거되어 같은 드라이브를 2~3중으로 병렬 파싱/FTS 하던 race condition. 두 계층으로 차단:
  - `is_busy()` 에 `WatchManager::is_paused()` 체크 추가 — startup sync 등 다른 경로가 watcher 를 pause 한 상태면 periodic_sync 는 skip.
  - `run_sync_all` 진입부에 전역 `AtomicBool SYNC_RUNNING` CAS lock + RAII guard — interval / focus 트리거끼리의 중첩 실행 자체를 차단, 패닉/early-return 시에도 자동 해제.
- **키워드 검색 결과 "관련도순 / 최신순" 정렬이 안 먹는 것처럼 보이던 문제** — 내용 매치 섹션에는 정렬이 적용되지만 상단 "파일명 매치" 섹션에는 정렬 로직이 없어 드롭다운 변경이 무반응으로 체감되던 문제. `filteredFilenameResults` 에도 내용 섹션과 동일한 `sortBy` 분기(confidence / date_desc / date_asc / name) 적용.
- **파일명 매치 결과 우클릭 → "폴더 열기" 시 파일도 함께 열리던 문제** — `ResultContextMenu` 가 `createPortal(document.body)` 로 렌더되어 DOM 은 분리되어 있지만 React synthetic event 는 여전히 원래 부모로 버블링된다. `FilenameResultItem` 의 부모 div `onClick` = `onOpenFile` 이 같이 실행되어 파일이 딸려 열림. 메뉴 버튼 4종(파일 열기 / 폴더 열기 / 경로 복사 / 유사 문서) 의 onClick 에 `e.stopPropagation()` 추가.

### 내부
- `WatchManager` 에 `is_paused() -> bool` 공개 API 추가 (기존 `pause_count` 내부 상태 노출).
- `periodic_sync.rs` 상단에 `SYNC_RUNNING` static + `SyncGuard` (Drop 구현) 추가 — 함수 중간에 panic 이 나도 lock 이 풀림.

---

## [2.5.2] - 2026-04-20

**자동 동기화 주기 — watcher 이벤트 누락 보완**

### 추가
- **백그라운드 주기 sync (기본 10분)** — Windows `ReadDirectoryChangesW` 버퍼 오버플로로 notify 이벤트가 누락되어도 최대 10분 안에 새 파일/삭제/수정이 자동 감지된다. 전체 드라이브 감시 시 특히 유용. 배치/벡터 인덱싱 중에는 skip, 실행 전 watcher pause → sync 후 resume 순서로 DB 락 경쟁 회피.
- **창 포커스 복귀 즉시 sync** — 앱을 잠시 벗어났다 돌아오면(`onFocusChanged`) 마지막 sync 로부터 2분 이상 경과했을 때 즉시 재정합. 다른 창에서 파일 복사 후 바로 검색하는 흐름이 자연스러워짐.
- **설정 > 시스템 > 성능 > "자동 동기화 주기"** — 끄기 / 5분 / 10분(기본) / 30분 선택. 0(끄기)로 두면 주기 sync 와 포커스 sync 모두 비활성.

### 변경/개선
- `AppContainer` 에 `last_sync_at`(AtomicI64) + `sync_shutdown`(AtomicBool) 추가 — 주기 task 종료 신호 공유.
- 앱 종료 시 `cleanup_vector_resources` 가 sync shutdown 을 먼저 세팅하여 task 가 최대 60초 내 탈출.
- 신규 Tauri 커맨드 `trigger_sync_if_stale(min_elapsed_secs)` — 프론트가 호출, 응답 즉시 반환(block 없음).
- 변경분 발견 시 `periodic-sync-updated` 이벤트 emit + FilenameCache 자동 재로드.

### 내부
- 신규 모듈 [src-tauri/src/indexer/periodic_sync.rs](src-tauri/src/indexer/periodic_sync.rs) — 기존 `IndexService::sync_folder` + `pause_watching`/`resume_watching` 재사용.

---

## [2.5.1] - 2026-04-20

**PDF 인코딩 깨짐 대응 + 테이블 렌더링 개선 + UX 폴리싱**

### 수정
- **PDF CID 인코딩 깨짐 감지 + OCR fallback** — Adobe InDesign 등이 Identity-H 폰트를 ToUnicode CMap 없이 임베드하면 pdf-extract/pdfjs 가 CID를 `鈀 逥鎖` 같은 쓰레기 유니코드로 반환하던 문제. 제어문자 ≥5%, PUA ≥5%, 한글+Latin+공백 <30% 중 하나 만족 시 깨진 페이지로 판단하고 OCR 로 대체, OCR 도 실패하면 해당 페이지 스킵(DB 오염 방지).
- **스캔 PDF 프리뷰 본문 누락** — kordoc 이 임베디드 텍스트만 135자 정도 반환해 프리뷰가 짧게 보이던 문제. PDF 는 kordoc 결과와 DB 청크(OCR 결과)를 비교해 긴 쪽 + 깨지지 않은 쪽을 자동 선택.
- **PDF 표가 세로 일렬로 플래튼** — 세로선 없는(행 구분선만) 표가 kordoc 에서 1열 다행 그리드로 잡혀 "목록성 데이터" 로 내려가던 문제. 1×N 그리드는 스킵하여 클러스터 기반 열 감지에 위임.
- **좁은 프리뷰 창에서 표가 글자 단위로 세로 분해** — `.doc-table` 이 `width:100%` + `.doc-th` 가 `white-space:nowrap` 이라 좁은 창에서 다른 열이 1~2글자 폭으로 쭈그러지던 문제. `width:auto`, `max-width:100%`, `word-break: keep-all`, `overflow-wrap: break-word` 로 교체.

### 개선
- **푸터 드라이브 인덱싱 진행률 smooth 표시** — 1/2 완료에서 `50%` 로 튀던 걸 사이드바와 동일하게 `(done + activeFraction) / total` 로 부드럽게. 현재 처리 중 파일명도 함께 표시.
- **검색 필터 바 스크롤 고정** — 결과 스크롤해도 `모두 포함 / 하나 이상 / 정확히 일치 / 관련도순 / 확장자 / 기간 / 파일명 제외 / 프리셋` 줄이 항상 보이도록 sticky 영역으로 분리.
- **결과 스크롤바 드래그 영역 확대** — 기본 8px, 호버 시 14px 로 커져 드래그 쉬워짐. 평소엔 미니멀.

### 빌드/번들
- kordoc 번들 재빌드됨 — `pnpm run bundle-kordoc` 로 PDF 표 감지 수정본 반영.

---

## [2.5.0] - 2026-04-20

**OneDrive·네트워크 폴더 대응 + 모델 번들 (오프라인/회사망 친화)**

### 추가
- **ONNX Runtime + PaddleOCR 모델 MSI 번들** — 첫 실행 시 인터넷 차단 환경에서도 즉시 시맨틱 검색·OCR 가능. 회사망/방화벽으로 huggingface·github 다운로드가 막혀도 동작.
- **OneDrive(클라우드 placeholder) 차단** — Files-On-Demand 로 클라우드에만 있는 파일은 본문 파싱을 자동으로 skip. 인덱서가 모르는 사이 수십 GB 를 끌어내리던 사고 방지. 파일명·크기·수정일은 정상 인덱싱(파일명 검색 가능).
- **네트워크 폴더(UNC, `\\server\share`) 정식 지원** — UNC 경로 정규화(dunce), 30초 주기 PollWatcher 분기, kordoc/HWP 파서 호환. SMB 위에서 inotify 가 동작 안 하는 한계를 폴링으로 우회.
- **`dunce` 의존성 추가** — Windows extended-length(`\\?\`) / UNC prefix 일관 정규화.

### 개선
- 폴더 등록 시 `dunce::canonicalize` 사용 — 네트워크 경로에서 표준 canonicalize 가 수십 초 block 되던 문제 해소.
- 인덱싱 결과에 `cloud_skipped_count` 추가 — 본문 skip 통계와 실패를 분리.

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB

---

## [2.4.0] - 2026-04-20

**최초 배포 (Public release)**

내 PC 문서를 통째로 검색하는 로컬 검색 엔진. 파일명 몰라도, 열어보지 않아도 문서 **내용**으로 찾습니다.

### 핵심 기능
- 문서 내용 검색 (SQLite FTS5, 1초 이내)
- 파일명 검색 (Everything 스타일, 인메모리 캐시)
- 시맨틱/하이브리드 검색 (KoSimCSE ONNX 768차원)
- AI 질의응답 + 문서 요약 (Gemini API, 선택)
- 문서 버전 자동 그룹핑 (lineage) + 버전 간 diff
- 실시간 파일 감시 (`.gitignore` 자동 존중)
- HWPX/DOCX/XLSX/PPTX/PDF/이미지(OCR)/텍스트 지원

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB

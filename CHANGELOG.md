# Changelog

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

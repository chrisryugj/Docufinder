# Docufinder macOS 포팅 계획서

**작성일**: 2026-05-06
**기반 버전**: v2.5.17 (main 브랜치)
**목적**: Windows 전용 Anything 앱을 macOS arm64로 포팅 — 별도 레포 분기 없이 동일 코드베이스에서 cfg 분기로 처리

---

## 1. 결정 사항 (확정)

| 항목 | 선택 | 이유 |
|------|------|------|
| 타겟 아키텍처 | **arm64 (Apple Silicon) 단일** | 빌드시간 절감, Intel Mac 비중 낮음. 추후 Universal 확장 가능 |
| 코드 서명 | **Ad-hoc (`codesign --sign -`)** | $99/년 절약. 자가/지인 배포 수준 |
| Notarization | **하지 않음** | Apple Developer ID 미보유 전제 |
| 자동 업데이트 (macOS) | **비활성화** | Notarization 없이 macOS updater는 불안정. 수동 다운로드로 대체 |
| 번들 타겟 | **dmg** | 표준 배포 형식. app만 따로 zip할 필요 없음 |
| HWP/HWPX 처리 | **kordoc 단일 경로** | 이미 한컴 COM 미사용. 변경 없음 |
| Conf 분리 | **`tauri.macos.conf.json` 별도 파일** | 플랫폼별 resources/bundle 깔끔히 분리 |

### 사용자 안내 정책
- 첫 실행 시: **우클릭 → 열기** 1회만 안내 (이후 자동)
- 만약 "손상된 앱" 표시되면: `xattr -dr com.apple.quarantine /Applications/Anything.app` 한 줄

---

## 2. Windows 종속성 처리 매트릭스

| 위치 | 현재 상태 | 조치 |
|------|----------|------|
| `Cargo.toml` `[target.'cfg(windows)']` windows-sys | ✅ 격리됨 | 없음 |
| `utils/idle_detector.rs` | ✅ non-windows fallback 있음 | 없음 |
| `utils/cloud_detect.rs` | ✅ cfg(windows) 격리 | non-windows 함수에 빈 stub 추가 검증 필요 |
| `utils/disk_info.rs` | ⚠️ WMI/PowerShell 호출 | macOS에선 항상 `DiskType::Ssd` 반환하는 cfg 분기 추가 |
| `parsers/kordoc.rs` `which_node()` | ❌ `node.exe` 하드코딩 | cfg(target_os = "windows") → `node.exe` / 그 외 → `node` |
| `parsers/kordoc.rs:336-339` `CREATE_NO_WINDOW` | ✅ cfg(windows) | 없음 |
| `model_downloader.rs` `onnxruntime.dll` | ❌ Windows 한정 다운로드 | cfg로 `libonnxruntime.dylib` 분기. macOS arm64 1.23.0 zip URL/SHA 추가 |
| `lib.rs:520` ORT_DYLIB_PATH | ❌ `onnxruntime.dll` 하드코딩 | cfg 분기 |
| `tauri.conf.json` resources `node.exe`, `onnxruntime.dll` | ❌ Windows 한정 | `tauri.macos.conf.json`로 macOS 전용 resources 분리 |
| `tauri.conf.json` `bundle.targets: ["nsis"]` | ❌ Windows | macOS conf에 `["dmg"]` |
| `tauri.conf.json` `webviewInstallMode` | ❌ Windows 전용 | macOS conf에서 제거 (WKWebView 자동) |
| `commands/file.rs`/`preview.rs` 경로 처리 | 일부 `\\?\` 가정 | dunce::simplified 경유 — macOS는 no-op |

---

## 3. 작업 순서 (Phase 별)

### Phase 0 — Mac 환경 셋업 (Mac 머신 도착 후 첫 30분)

```bash
# Xcode CLI tools
xcode-select --install

# Rust + arm64 타겟
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-darwin

# Node.js (Homebrew 권장)
brew install node@20 pnpm

# 레포 클론
git clone https://github.com/chrisryugj/Docufinder.git
cd Docufinder
pnpm install
```

**검증**: `cargo --version`, `node --version`, `pnpm --version` 모두 출력되면 OK.

---

### Phase 1 — 빌드 통과 (Hello World)

목표: `pnpm tauri:dev`로 창 띄우기까지. 검색/인덱싱은 아직 동작 안 해도 됨.

#### 1-1. 코드 분기 추가

**파일별 변경 위치**

1. [src-tauri/src/utils/disk_info.rs](src-tauri/src/utils/disk_info.rs) — 비-Windows에서 Ssd 반환
   ```rust
   #[cfg(not(windows))]
   pub fn detect_disk_type(_path: &Path) -> DiskType {
       DiskType::Ssd  // 모던 macOS는 모두 SSD 가정
   }
   ```
   기존 windows 구현은 `#[cfg(windows)]` wrap.

2. [src-tauri/src/parsers/kordoc.rs:240](src-tauri/src/parsers/kordoc.rs#L240) — node 바이너리 이름 분기
   ```rust
   #[cfg(target_os = "windows")]
   const NODE_BIN: &str = "node.exe";
   #[cfg(not(target_os = "windows"))]
   const NODE_BIN: &str = "node";
   ```
   `which_node()`의 모든 `"node.exe"` 리터럴을 `NODE_BIN`으로 치환.

3. [src-tauri/src/lib.rs:520](src-tauri/src/lib.rs#L520) ORT 동적 라이브러리 경로
   ```rust
   #[cfg(target_os = "windows")]
   let dylib = "onnxruntime.dll";
   #[cfg(target_os = "macos")]
   let dylib = "libonnxruntime.dylib";
   ```

4. [src-tauri/src/model_downloader.rs](src-tauri/src/model_downloader.rs) — macOS arm64용 ONNX Runtime 다운로드 분기
   - macOS arm64: `https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-osx-arm64-1.23.0.tgz`
   - 압축 해제 후 `libonnxruntime.1.23.0.dylib` 추출 → `libonnxruntime.dylib`로 심볼릭 링크 또는 rename
   - SHA-256 검증 추가 (다운로드 후 macOS에서 측정해서 코드에 박음)

#### 1-2. tauri.macos.conf.json 작성

```json
{
  "bundle": {
    "targets": ["dmg"],
    "resources": [
      "resources/node",
      "resources/kordoc/**/*",
      "resources/paddleocr/det.onnx",
      "resources/paddleocr/rec.onnx",
      "resources/paddleocr/dict.txt",
      "resources/onnxruntime/libonnxruntime.dylib",
      "icons/tray-icon.png"
    ],
    "macOS": {
      "minimumSystemVersion": "11.0",
      "signingIdentity": "-",
      "entitlements": null,
      "exceptionDomain": null
    }
  },
  "plugins": {
    "updater": {
      "active": false
    }
  }
}
```
> Tauri는 빌드 시 `tauri.<platform>.conf.json`을 자동 머지함.

#### 1-3. 리소스 디렉터리 준비 (Mac에서)

```bash
# Node.js arm64 바이너리
curl -O https://nodejs.org/dist/v20.18.0/node-v20.18.0-darwin-arm64.tar.gz
tar xzf node-v20.18.0-darwin-arm64.tar.gz
cp node-v20.18.0-darwin-arm64/bin/node src-tauri/resources/node
chmod +x src-tauri/resources/node

# kordoc 네이티브 의존성 재설치 (darwin canvas)
cd src-tauri/resources/kordoc
rm -rf node_modules
pnpm install --force --prod
cd -

# ONNX Runtime arm64 dylib
mkdir -p src-tauri/resources/onnxruntime
curl -L -o ort.tgz https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-osx-arm64-1.23.0.tgz
tar xzf ort.tgz
cp onnxruntime-osx-arm64-1.23.0/lib/libonnxruntime.1.23.0.dylib src-tauri/resources/onnxruntime/libonnxruntime.dylib
rm -rf ort.tgz onnxruntime-osx-arm64-1.23.0
```

> 이 과정은 `scripts/setup-macos-resources.sh`로 자동화 권장 (Phase 1 끝에 작성).

**검증**: `pnpm tauri:dev` — 창 뜨고 검색바 보이면 OK.

---

### Phase 2 — 핵심 기능 동작 검증 (각 1~2시간)

| 검증 항목 | 방법 |
|----------|------|
| 폴더 추가 | 작은 테스트 폴더(.txt 5~10개) 추가 → 인덱싱 완료 |
| .hwpx 파싱 | 샘플 .hwpx 검색 → 본문 검색 결과 |
| .hwp 파싱 | 샘플 .hwp 검색 (kordoc 단일 경로) |
| .pdf 파싱 | 일반 PDF + 스캔 PDF (OCR fallback) |
| .docx/.xlsx | 표준 Office 파일 |
| 시맨틱 검색 | 의미 검색 토글 ON → 결과 차이 |
| 실시간 감시 | 폴더에 파일 추가 → 자동 인덱싱 |
| 종료 | 작업 중 종료 → 재시작 시 정상 복구 |

각 항목 실패 시 로그 → fix → 재검증.

---

### Phase 3 — DMG 빌드 + Ad-hoc 서명

```bash
pnpm tauri:build --target aarch64-apple-darwin
# 산출물: src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Anything_2.5.17_aarch64.dmg

# Ad-hoc 서명 (Tauri의 signingIdentity: "-"로 자동 적용되지만 명시적 재서명)
codesign --force --deep --sign - \
  src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Anything.app
```

**검증**: dmg 더블클릭 → Applications 드래그 → 우클릭 → 열기 → 정상 실행.

---

### Phase 4 — CI 추가

[.github/workflows/publish.yml](.github/workflows/publish.yml)에 macOS job 추가:

```yaml
build-macos:
  runs-on: macos-14  # M1 runner
  steps:
    - uses: actions/checkout@v4
    - uses: pnpm/action-setup@v3
      with: { version: 9 }
    - uses: actions/setup-node@v4
      with: { node-version: 20, cache: pnpm }
    - run: rustup target add aarch64-apple-darwin
    - run: pnpm install --frozen-lockfile
    - run: bash scripts/setup-macos-resources.sh  # node + kordoc + ort 다운로드
    - run: pnpm tauri:build --target aarch64-apple-darwin
    - name: Ad-hoc sign
      run: codesign --force --deep --sign - src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Anything.app
    - uses: actions/upload-artifact@v4
      with:
        name: macos-arm64
        path: src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/*.dmg
```

> `latest.json`에는 macOS 채널을 **추가하지 않음** (자동 업데이트 미지원).

---

### Phase 5 — README 갱신

[README.md](README.md)에 macOS 설치 섹션 추가:

```markdown
## macOS (Apple Silicon) 설치
1. [Releases](https://github.com/chrisryugj/Docufinder/releases)에서 최신 `.dmg` 다운로드
2. dmg 더블클릭 → Applications 폴더로 드래그
3. **첫 실행만**: Applications 폴더에서 Anything 우클릭 → 열기
4. 만약 "손상된 앱" 메시지가 뜨면:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Anything.app
   ```
5. 자동 업데이트 미지원 — 새 버전은 수동 다운로드
```

---

## 4. 알려진 리스크 & 완화

| 리스크 | 가능성 | 완화 |
|--------|--------|------|
| ort 2.0.0-rc.11 + macOS arm64 호환성 | 낮음 | 1.23.0 dylib 직접 번들. 문제시 ort 2.0.1 정식 대기 |
| @napi-rs/canvas darwin-arm64 prebuild 부재 | 매우 낮음 | npm registry에 prebuild 있음 (확인됨) |
| pdfjs-dist Node 환경 차이 | 낮음 | 이미 cross-platform pure JS |
| Tauri WKWebView 렌더링 차이 | 중간 | Phase 2 시각 검증 필수. CSS quirks fix 가능 |
| `notify` crate FSEvents 백엔드 | 낮음 | 공식 지원, 자동 활성화 |
| dmg 빌드 시 codesign 실패 | 중간 | `signingIdentity: "-"` 명시. 그래도 실패 시 수동 codesign |
| HWPX 일부 한글 폰트 미렌더링 | 낮음 | 텍스트 추출만 사용 — 폰트 무관 |

---

## 5. 작업량 추정

| Phase | 추정 |
|-------|------|
| 0. 환경 셋업 | 30분 |
| 1. 빌드 통과 + 코드 분기 + 리소스 | 4~6시간 |
| 2. 기능 검증 + 버그 fix | 1~2일 |
| 3. dmg 빌드 + 서명 | 2시간 |
| 4. CI 추가 | 2~3시간 |
| 5. README + 릴리즈 | 1시간 |

**총 5~7일 (실제 작업 기준)**

---

## 6. Mac에서 시작할 때 첫 명령

```bash
# 1. 클론
git clone https://github.com/chrisryugj/Docufinder.git
cd Docufinder
git pull origin main

# 2. 이 계획서 다시 열기
open .claude/plans/mac-port-plan.md
# 또는 Claude Code에서 /memory-start 후 이 파일 읽으라고 지시

# 3. Phase 0부터 순서대로 진행
```

### Claude Code 세션 시작 프롬프트 (Mac에서)

```
docs/MAC_PORT_PLAN.md를 Read 도구로 읽어줘.
거기 적힌 Phase 0부터 순서대로 진행하자. Phase 1의 코드 분기 작업부터
실제 파일을 읽어 현재 상태를 확인한 뒤 변경하기 시작해.
```

---

## 7. 진행 상태 트래킹

> Phase 완료 시 이 섹션의 체크박스를 갱신하고 커밋.

- [x] Phase 0: Mac 환경 셋업 (Rust 1.95, pnpm 11, aarch64-apple-darwin 타겟)
- [x] Phase 1-1: 코드 분기 (disk_info / kordoc / lib.rs / model_downloader)
- [x] Phase 1-2: tauri.macos.conf.json 작성
- [x] Phase 1-3: setup-macos-resources.sh 작성 + 실행
- [x] Phase 1: pnpm tauri:dev:mac 정상 실행 — 윈도우 표시 + DB 초기화 + 시스템 트레이 OK
- [ ] Phase 2: 기능 검증 (8항목) — 사용자 GUI 테스트 진행 중
- [ ] Phase 3: dmg 빌드 + ad-hoc 서명 — 진행 중
- [x] Phase 4: CI macOS job 추가 (publish.yml `publish-macos` job)
- [x] Phase 5: README macOS 섹션 추가
- [ ] Phase 6: 첫 macOS 릴리즈

---

## 8. 참고

- ONNX Runtime 릴리즈: https://github.com/microsoft/onnxruntime/releases/tag/v1.23.0
- Tauri macOS 가이드: https://v2.tauri.app/distribute/sign/macos/
- Ad-hoc 서명: https://developer.apple.com/library/archive/technotes/tn2206/_index.html
- Apple Silicon Rust 타겟: `aarch64-apple-darwin`

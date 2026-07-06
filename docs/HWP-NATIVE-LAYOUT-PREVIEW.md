# HWP(.hwp 바이너리) 원본 레이아웃 미리보기 — 구현 컨텍스트 / 핸드오프

**상태**: 구현 완료 · 정적 검증 통과(컴파일·유닛테스트·clippy·tsc) · **실앱 런타임 검증 대기**
**작성**: 2026-07-06 · **브랜치**: `feat/hwp-native-layout-preview`

---

## 1. 무엇을 했나

`.hwp`(HWP5 OLE2 바이너리)를 한컴 설치 없이 **원본 조판 SVG**로 렌더해 미리보기 "원본 레이아웃"
뷰에 연결했다. 기존에 `.hwp`는 마크다운 텍스트 미리보기만 가능했고 원본 레이아웃은 **전무**했다.

- HWPX = kordoc render SVG (기존), PDF = pdfium 페이지 이미지(기존), **HWP = rhwp 네이티브 SVG (신규)**
- 셋 다 프론트 `LayoutView`(HWPX) / `PdfLayoutView`(PDF)로 소비. HWP는 **HWPX와 동일하게 `LayoutView` 재사용**.

## 2. 핵심 결정: rhwp를 **네이티브 Rust 크레이트**로 (exe 사이드카 아님)

[rhwp](https://github.com/edwardkim/rhwp) = 순수 Rust HWP/HWPX 뷰어(MIT, Edward Kim). `DocumentCore→SVG`.

| 후보 | 결정 | 근거 |
|------|------|------|
| 프리빌트 `rhwp.exe` 사이드카 (kordoc 방식) | ❌ | **Windows 전용** — 맥 지원 불가 (플랫폼별 exe 번들은 유지보수 부담) |
| **git Cargo 의존성 (네이티브 링크)** | ✅ | 단일 코드베이스가 **Win/macOS 둘 다 컴파일**. exe 번들·다운로드 불필요. `render_page_svg_native`가 순수 Rust(`tiny-skia`) 경로라 **C++ skia 링크 없음** |

**실측 근거**: stable Rust **1.92.0**에서 rhwp가 네이티브 의존성으로 컴파일+실행 성공(격리 테스트,
`pages=2`, 각 페이지 SVG 생성 확인). rhwp의 `rust-toolchain.toml`은 1.93.1을 핀하지만 그건 rhwp
개발자용이고 **실제 코드는 1.92.0에서 빌드됨** → Docufinder 툴체인 유지(버전 상향 불필요).

## 3. 변경 파일

| 파일 | 변경 |
|------|------|
| `src-tauri/Cargo.toml` | `rhwp` git 의존성 추가 (`rev` = v0.7.17 태그 커밋 SHA `03351190…` 고정, `default-features = false`) |
| `src-tauri/Cargo.lock` | rhwp 의존성 트리 + `zip` 8.3.1→8.6.0 (rhwp `^8.5` 요구, Docufinder `^8` 범위 내라 안전) |
| `src-tauri/src/parsers/rhwp.rs` | **신규** — `render_svg(path)`: 파일 읽기 → `DocumentCore::from_bytes` → 페이지별 `render_page_svg_native` → **ID 네임스페이스 + `data-page` 세로스택 합성** → 단일 SVG. `catch_unwind`로 파서 패닉 봉쇄. 유닛테스트 4개 |
| `src-tauri/src/parsers/mod.rs` | `pub mod rhwp;` 등록 (인덱싱 디스패치 `parse_file`는 **무수정** — .hwp 인덱싱/마크다운은 계속 kordoc) |
| `src-tauri/src/commands/preview.rs` | `render_layout_svg`가 ext로 분기: `hwp`→`rhwp::render_svg`, 그 외→`kordoc::render_svg`(기존) |
| `src/components/search/PreviewPanel.tsx` | `isHwp` 게이트 추가 4곳: (1) `isHwp` const, (2) 원본레이아웃 세그먼트 노출, (3) 뷰전환 단축키(1/2), (4) 선호뷰 자동진입. **`LayoutView`는 무수정** |

### 멀티페이지 stitch (rhwp.rs의 핵심)

rhwp는 **페이지당 SVG 파일 1개**를 만든다(kordoc은 `data-page` 세로스택 단일 SVG). 그래서 백엔드에서
페이지들을 합친다:

- 각 페이지 SVG의 `viewBox`에서 W·H 추출 → `<g data-page="N" transform="translate(0,yOffset)">`로 세로 스택
- **clipPath ID 충돌 방지**: 각 페이지의 `id="X"`·`url(#X)`를 `pN-` 접두로 네임스페이스
  (rhwp SVG의 유일한 id 참조는 `clip-path="url(#…)"` — gradient/mask/use/href 없음을 실측 확인)
- 각 페이지가 자체 흰 배경 rect 포함 → 페이지 간 `PAGE_GAP`(12px)에 회색 컨테이너 배경이 비쳐 경계로 보임

## 4. 검증 상태

**완료(정적 + 렌더 품질)**:
- ✅ `cargo check --lib` 통과 (rhwp 의존성 + 신규 모듈 컴파일)
- ✅ `cargo test --lib parsers::rhwp` 4/4 (viewbox·wrapper·namespace·stitch)
- ✅ `cargo clippy --lib` 경고 0 (rhwp.rs·preview.rs)
- ✅ `tsc --noEmit` 통과
- ✅ **실제 .hwp 렌더 품질** — `재활용센터현황.hwp`/`재활용센터현황2.hwp`(kordoc fixtures)를 rhwp로
  렌더 → Edge headless(=WebView2 동일엔진)로 스크린샷 육안 검증. 표 병합·헤더음영·per-run 폰트·
  각주 near-한컴 수준. 멀티페이지 stitch도 WebView2에서 각 페이지 클리핑 독립 정상 확인.

**대기(다른 PC에서 할 것)**:
- ⬜ **실앱 런타임 E2E** — `pnpm tauri:dev`로 앱 실행 → .hwp 포함 폴더 인덱싱 → 결과 클릭 →
  "원본 레이아웃"(단축키 2) 클릭 → rhwp 렌더가 `LayoutView`에 뜨는지. 팝업 크게보기·줌·찾기(Ctrl+F)도.
- ✅ 릴리즈 빌드 thin-LTO rhwp 컴파일 — **v3.2.0 태그 CI로 검증 완료(2026-07-06)**: Windows setup.exe + macOS dmg 빌드·Release 게시 성공
- ✅ macOS(`tauri:build:mac`) 실빌드 확인 — **2026-07-06 맥미니(Apple Silicon) 검증**: cargo test 248/248(rhwp 유닛 포함),
  릴리스 컴파일 3m36s, Anything.app 번들·ad-hoc 서명 성공 (DMG 스크립트만 SSH 셸 Finder 스크립팅 불가로 실패 — 환경 문제, CI 무관)

## 5. ⚠️ 함정 / 주의

- **`zip` lock 상향**: rhwp가 `zip ^8.5`를 요구해 Docufinder lock을 8.3.1→8.6.0으로 올림
  (`cargo update -p zip@8.3.1 --precise 8.6.0`). Docufinder `zip="8"` 범위 내라 안전. 되돌리지 말 것.
- **`LAYOUT_OVERFLOW` 로그**: rhwp가 조판 오버플로 시 stdout/stderr로 진단 로그를 뿜음(in-process라
  앱 콘솔로). 릴리즈 windowed 앱에선 안 보이나 존재. SVG 반환값은 오염 안 됨(로그는 별개 스트림).
  억제하려면 rhwp 포크/feature 필요 — 저우선.
- **밀집 표 셀 clip/겹침**: 좁은 셀의 긴 수식·다행 헤더가 셀 경계에서 clip되거나 살짝 겹침
  (`재활용센터현황2.hwp` p1 수식 셀에서 관찰). kordoc의 "결재란 겹침"과 **동일한 하드케이스**(셀
  content extent 과소측정). 대다수 문서는 완벽 — 현재 원본레이아웃 전무 대비 압도적 개선.
- **HWP는 검색어 형광펜 미지원**: rhwp에 `--highlight` 없음 → `highlight_query` 무시. 단 `LayoutView`의
  인앱 찾기(Ctrl+F, SVG `<text>` 매칭)는 정상 동작.
- **rhwp 1.93.1 핀은 비구속**: rhwp의 `rust-toolchain.toml`(1.93.1)은 의존성으로 쓸 땐 무시됨
  (Docufinder 툴체인으로 빌드). CI Rust는 **≥1.92** 필요(실측). CI가 더 낮으면 상향.
- **파서 패닉 봉쇄**: rhwp 호출은 `catch_unwind`로 감쌈. Docufinder `panic="unwind"`라 유효 —
  `panic="abort"`로 바꾸면 손상 .hwp가 앱을 죽일 수 있음(전역 규칙과 동일).

## 6. 다른 PC에서 이어가기

1. `git fetch && git checkout feat/hwp-native-layout-preview`
2. `cd src-tauri && cargo build` — rhwp git 의존성이 **자동 fetch+컴파일**(별도 exe/다운로드 없음).
   최초 clean 빌드 시 rhwp+deps 컴파일로 수 분 추가.
3. 프론트: 루트에서 `pnpm install` (변경 없음)
4. 실행: `pnpm tauri:dev` → §4의 런타임 E2E 수행
5. 검증 방법론(참고): SVG 육안검증은 **Edge headless**(`msedge --headless=new --screenshot`, WebView2
   동일엔진) — librsvg/sharp는 `textLength` 무시해 겹침 과장. (메모리 `project_hwpx_pdf_preview.md` 참조)

## 7. 릴리즈 고려사항

- **공급망**: rhwp는 `rev`(커밋 SHA) 고정 — kordoc SHA-pin 정책과 동일. 버전 올릴 땐 새 태그의 커밋
  SHA로 `rev` 교체 + Cargo.lock 갱신.
- **빌드 시간**: 네이티브 크레이트라 clean 빌드마다 rhwp(438 소스파일 + usvg/tiny-skia 등) 컴파일.
  incremental은 영향 적음. CI 캐시 권장.
- **크레이트 규모**: rhwp는 `[lib] crate-type=["cdylib","rlib"]` 단일 크레이트. `native-skia`(C++ skia)
  feature는 **켜지 않음**(SVG는 순수 Rust 경로). 켜면 skia-safe 빌드로 폭증하니 주의.

## 8. rhwp 참고

- Repo: https://github.com/edwardkim/rhwp · Ver: **v0.7.17** (`rev 03351190ec35436e58cbfee0aa9278a8fdc04a59`)
- 네이티브 API(크레이트 루트 re-export): `rhwp::DocumentCore::from_bytes(&[u8]) -> Result<_, HwpError>`,
  `.page_count() -> u32`, `.render_page_svg_native(page: u32) -> Result<String, HwpError>`,
  `.populate_external_images_from_dir(&Path)` (같은 폴더 이미지 로드)
- `.hwp`·`.hwpx` 둘 다 `from_bytes` 자동판별. **본 작업은 .hwp 전용**(HWPX는 kordoc이 이미 양호 — surgical).
- CLI(`rhwp export-svg`)도 있으나 미사용(네이티브 API 직접 호출).

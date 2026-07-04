# 다음 세션 — 팝업 뷰어 줌 UX 3종 (+ 잔여: 발행·OCR·검증)

> **트리거**: "뷰어 이어서" / "줌 개선" / "docufinder 발행" → 이 파일로 시작.
> **직전 세션(2026-07-04, ultracode)**: 크롬 통합·문서 팝업 뷰어·OCR Phase 0·네이티브
> 드래그아웃·docling 재판단까지 완료(전부 커밋). 아래 §1이 사용자가 다음으로 지시한 것.

---

## 0. 직전 세션 완료 (미푸시 4 커밋 — origin/main 대비 ahead 4)

| 커밋 | 내용 |
|---|---|
| `1225a90` | 문서 크게 보기 **팝업 뷰어** — 레이아웃 렌더를 전체화면 오버레이(role=dialog)로. LayoutView 재사용(onClose·onExpand). 이미지별 라이트박스는 폐기(오해였음). |
| `b204a84` | OCR 설계 **재판단** — docling이 레이아웃 모델을 ONNX 직배포(heron-onnx 171MB, Apache-2.0). 기본은 PP-DocLayout-M 유지, heron은 벤치 오라클+상위티어. `docs/OCR-LAYOUT-DESIGN.md §3.4`. |
| `a270260` | 검색 결과 **네이티브 드래그아웃** — 파일명 잡아 다른 앱/웹에 드롭. tauri-plugin-drag + `drag_preview_icon` 커맨드 + `drag:default` 권한 + SearchResultItem draggable. |
| `b809d9b` | fix — ① 선호 뷰 재적용(prefAppliedRef 파일 재오픈마다 리셋) ② 팝업 휠=스크롤/Ctrl·⌘휠(핀치)=줌 전환(freeZoom 제거). |

앞선 세션에 이미 **푸시됨**: 크롬 통합(`f86ee17`), OCR Phase 0(`710be6f`), 세그먼트 UX 등.

**★발행 미결**: 위 4개 push 안 함(사용자가 직접 히스토리 관리 중). 발행 지시 대기.

---

## 1. 지금 할 것 — 팝업 뷰어 줌 UX 3종 (사용자 확정)

**전부 `src/components/search/LayoutView.tsx` 한 파일, 프론트 only.** PreviewPanel 무변경.

**★모델 주의**: LayoutView 줌은 **width-% + overflow 스크롤** 모델이다(`transform: scale` 아님).
`widthStyle = fitWidth ? "100%" : \`${zoom*100}%\``, 호스트는 `mx-auto`, 팬은 `scrollLeft/Top` 조작.
따라서 커서 기준 줌은 "줌 후 스크롤 보정"으로 구현한다.

공용 준비물:
- `import { useLayoutEffect } from "react"` 추가.
- 현재값 ref(스테일 클로저 방지): 렌더마다 `zoomRef.current = zoom; fitWidthRef.current = fitWidth;`
- `const clampZoom = (z) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z))`
- `const zoomAnchorRef = useRef<{fx,fy,cx,cy}|null>(null)`

### ① 커서 기준 줌 (코어 — ②가 재사용)
`zoomToPoint(nextZoom, clientX, clientY)`:
1. `sc=scrollRef.current, host=hostRef.current`; `rect=sc.getBoundingClientRect()`.
2. `cx=clientX-rect.left, cy=clientY-rect.top` (뷰포트 내 커서).
3. 줌 전 커서 아래 지점의 **분율** 캡처: `fx=(sc.scrollLeft+cx)/host.offsetWidth`, `fy=(sc.scrollTop+cy)/host.offsetHeight`.
4. `zoomAnchorRef.current={fx,fy,cx,cy}; setFitWidth(false); setZoom(clampZoom(nextZoom))`.
5. **보정** `useLayoutEffect(…, [zoom, fitWidth])`: anchor 있으면 새 host 치수로
   `sc.scrollLeft = a.fx*host.offsetWidth - a.cx; sc.scrollTop = a.fy*host.offsetHeight - a.cy;` 후 anchor=null.
   (분율은 스케일 불변이라 옛 fx×새 offsetWidth 가 정확. fitWidth≡zoom1 시각적 동치.)
- `onWheel`: Ctrl/⌘일 때 `zoomToPoint(cur - e.deltaY*0.0015, e.clientX, e.clientY)`, `cur = fitWidthRef.current ? 1 : zoomRef.current`.
- `zoomBy(d)`(±버튼): 컨테이너 중앙 기준 `zoomToPoint(cur+d, rect.left+rect.width/2, rect.top+rect.height/2)`.

### ② 더블클릭 줌 토글 (뷰어 모드 = onClose 있을 때만)
스크롤 컨테이너에 `onDoubleClick={onClose ? onDoubleClick : undefined}`:
- 확대상태(`!fitWidthRef.current && zoomRef.current>1.01`)면 → `setFitWidth(true); setZoom(1); zoomAnchorRef.current=null` (너비맞춤 원복).
- 아니면 → `zoomToPoint(2, e.clientX, e.clientY)` (그 지점 2× 확대).
- 인라인(작은 패널)엔 미부착 — 단어 더블클릭 선택 보존.

### ③ 맞춤 모드 순환 — 너비맞춤 ↔ 페이지맞춤
`fitPage()`:
- `pageEl = host.querySelector("[data-page]") ?? host`; `pageH = pageEl.offsetHeight`.
- `contH = sc.clientHeight - 24` (py-3 여백); `cur = fitWidthRef.current?1:zoomRef.current`.
- `setFitWidth(false); setZoom(clampZoom(cur*(contH/pageH)))`; rAF로 `sc.scrollTop=0`; anchor=null.

기존 "너비 맞춤" 버튼(Maximize2)을 **토글**로: `fitWidth ? fitPage() : (setFitWidth(true), setZoom(1))`,
`title={fitWidth ? "페이지 맞춤" : "너비 맞춤"}`.
- **"실제 크기(1:1)"는 벡터 렌더(kordoc SVG, `svg{width:100%}` 강제)라 의미가 약해 제외** — width↔page 2모드가
  문서 뷰어에 실질적. 진짜 1:1 필요하면 `.layout-svg-host svg`의 width 오버라이드 해제가 필요(별도 논의).

**검증**: `npx tsc --noEmit && npx vite build` (프론트 only, Rust 무관).
**주의**: 세 기능 모두 팝업뿐 아니라 인라인 LayoutView 에도 적용됨(①③은 무해, ②만 뷰어 한정 게이트).

---

## 2. 잔여 열린 항목 (우선순위)

- **A. 발행** (지시 대기) — 미푸시 4커밋 push. docufinder 버전 bump+CHANGELOG 여부 결정.
  kordoc v3.15.0 npm publish+태그도 여전히 미결(직전 세션들 이월).
- **B. OCR Phase 1** (레이아웃 분류) — 설계·모델 확정됨(`OCR-LAYOUT-DESIGN.md`). 기본 = PP-DocLayout-M(22MB,
  oar-ocr 릴리스, Rust `ort` 참조코드 있음) + XY-Cut(Phase 0 완료). heron-onnx는 벤치 오라클로.
  착수: `ensure_layout_model` + `ocr/layout.rs`(전처리·NMS·class 매핑) + 융합(IoU 귀속). fmt/clippy 게이트 필수.
- **C. 태그 발견성** (직전 UX 리뷰 Med) — 빈 태그바 제거 대가로 태그 추가가 `⋯>태그추가` 2클릭.
  원하면 더 가벼운 진입점(예: 파일명 옆 태그 칩 hover 어포던스). 사용자 판단 대기.
- **D. 드래그아웃 런타임 검증** — 헤드리스 불가. 앱에서 결과 파일명→Finder/메일/웹 업로드칸 드롭 확인.
  실패 시: `drag:default` 권한, `drag_preview_icon` 캐시 아이콘 경로, startDrag 호출 타이밍 순으로 진단.
- **E. 앱 시각 검증** — 통합 툴바, 팝업 뷰어, 선호 뷰 재적용, 휠=스크롤/핀치=줌.
  `KORDOC_CLI_PATH=~/workspace/kordoc/dist/cli.js pnpm tauri:dev:mac`
- **F. reflow 미세개선**(kordoc, 이월) — dyMax≈176 세로offset·81% 셀 줄나눔. `bench/verify-reflow.mjs` 게이트.

## 3. 게이트/주의
- 프론트: `tsc && vite build`. Rust: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test`
  (커밋마다 CI가 강제 — lesson-docufinder-fmt-gate).
- 커밋 신원 chrisryugj/ryuseungin@naver.com. main 직접(브랜치 만들지 말 것). **push는 지시받고**.
- 첫 액션: `git pull`(다른 PC 반영 가능성) → 이 파일 §1 정독 → LayoutView 3종 구현 → 검증 → 커밋.

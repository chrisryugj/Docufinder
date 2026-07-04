# 다음 세션 — Anything 미리보기 뷰 전환 UX 획기적 개선 + 추가 검증

> **트리거**: "미리보기 뷰 개선" 또는 "뷰 전환 UX"라고 하면 이 파일로 시작.
> **1순위 = 미리보기 뷰 전환 UX 획기적 개선(사용자 명시 요청).** 계획 확정 후 착수.

---

## 0. 이번 세션 완성 (배경 — 2026-07-04, ultracode)

**kordoc v3.15.0** (커밋 `7fb9885`, **npm 미배포·태그 없음**)
- Tier-2 reflow 엔진 — 조판 캐시 없는 HWPX(생성·편집본)를 순수 TS 조판.
  자기일관성 9/10(match 91~100%·세로 0.1~1pt), 무회귀(681 테스트·score.mjs PASS).
  `src/render/reflow.ts` + `head-styles.paraGeom` + `renderHwpxToSvg({reflow:true})`.
  세로모델 실측 박제 = `.claude/plans/render-poc/findings.md`(gitignore, 로컬).
- 그리기 도형 — rect/ellipse/line/polygon/curv/arc SVG 렌더(`svg-render.drawShape`).
- render-worker — `kordoc render-worker`, stdin NDJSON persistent 워커(콜드스타트 제거).

**docufinder** (커밋 `c31bcd2`)
- `LayoutView.tsx` 신설 — `<img data:>`→인라인 SVG. 줌/팬(Ctrl+휠·드래그), 페이지
  네비게이션, 레이아웃 뷰 내 찾기 매치 이동(SVG `<text>` 검색·강조), 텍스트 선택.
- `kordoc.rs` — persistent render-worker 연동(백그라운드 리더·id매칭·타임아웃·재시작·
  oneshot 폴백) + reflow 옵션(캐시 없는 HWPX 미리보기). 번들 재복사 완료.

**보류**: kordoc npm publish·태그 · docufinder 앱 시각 검증(tauri:dev).

---

## 1. 1순위 — 미리보기 뷰 전환 UX 획기적 개선 (사용자 명시)

**현재 문제**: 레이아웃/마크다운 전환이 툴바 아이콘 버튼 — **툴팁이 늦게 뜨고 위치가
불편**하며 지금 어느 뷰인지·뭘 누르는지 불명확하다.

**목표**: 마크다운(문서 텍스트) 뷰 ↔ 원본(레이아웃) 뷰를 **쉽고 즉각적으로 선택·전환**.

**방안 후보 (세션 시작 시 사용자와 확정 — 조합 가능)**:
1. **세그먼트 컨트롤** — 미리보기 상단에 `[ 문서 텍스트 | 원본 레이아웃 ]` 큰 세그먼트
   토글. 현재 뷰가 눈에 명확, 한 번에 전환. (에디토리얼 미니멀, hairline)
2. **나란히(split) 뷰** — 좌 마크다운 | 우 레이아웃, 스크롤 동기. 대화면에서 원본 대조
   읽기. (사용자가 원했던 "스크롤 동기"의 실질 구현처)
3. **단축키** — `Tab`/`1`·`2`로 즉시 전환(검색 결과 이동 중에도). 마우스 없이.
4. **기본 뷰 기억 + 파일타입 우선** — HWPX는 레이아웃 뷰 기본, 선호 뷰를 설정에 저장
   (현재는 파일 전환마다 markdown 리셋 — PreviewPanel.tsx:675).
5. **호버/썸네일** — 검색 결과 카드에 레이아웃 썸네일 프리뷰(레이아웃 뷰 진입 유도).

**가드레일**: 스크롤 로직 불변 · DESIGN.md 토큰 · 안티슬롭(에디토리얼 미니멀) · 기존
찾기(Ctrl+F)·AI 인용 점프와 정합 · 레이아웃 렌더 실패(HWP·비HWPX) 시 우아한 폴백.

**관련 파일**: `PreviewPanel.tsx`(뷰 토글 `handleToggleLayout`·`viewMode`·뷰 전환 버튼
1020·1090 근처·`requestLayoutRender`), `LayoutView.tsx`(뷰어 본체).

---

## 2. 2순위 — 추가 검증 (앱 시각)

`KORDOC_CLI_PATH=~/workspace/kordoc/dist/cli.js pnpm tauri:dev` 로 레이아웃 뷰 실측:
- reflow(AI 생성/편집 HWPX 미리보기가 실제로 뜨는지), 도형 렌더 충실도, 다페이지
  네비게이션, 줌/팬 체감, 레이아웃 뷰 내 매치 이동, worker 콜드스타트 제거 체감.

**reflow 미세개선(kordoc)** — `bench/verify-reflow.mjs` 게이트 기준:
- 자기일관성 1건 match 81%(긴 셀 텍스트 줄나눔 위치 초과) · dyMax≈176HWPUNIT(1.76pt)
  systematic 세로 offset · lineSpacing 비-PERCENT(FIXED/BETWEEN_LINES) pitch 분기 ·
  intent hanging(내어쓰기) horzpos/contWidth 정밀화 · 표 페이지 중간 분할(현재 문단 단위만).
**도형(kordoc)**: 회전(`rotationInfo` angle), `connectLine`, arc start/sweep 각 미해석.
**worker**: 동시 요청/부하, 강제 재시작 실측.
**bench:gate 전체**: `corpus/review`(gitignore 로컬 코퍼스)가 있는 환경/맥미니에서 roundtrip 포함 5체인.

---

## 3. 3순위 — 릴리스 발행 (이번 세션 보류분)

- **kordoc v3.15.0**: `npm publish` + `git tag v3.15.0 && git push --tags`
  (이번 세션은 커밋 `7fb9885`만, 발행 보류). ★fly deploy 금지 규칙 등 CLAUDE.md 준수.
- **docufinder**: 버전 bump + CHANGELOG(뷰어 UX). tauri:build 전 `bundle-kordoc`.

---

## 4. 첫 액션
1. `git pull`(양쪽) → 이 파일 + `LayoutView.tsx` + `PreviewPanel.tsx` 뷰 토글부 정독.
2. **뷰 전환 UX 방안 확정**(세그먼트/split/단축키/기본뷰 조합) → 착수.
3. 여력 시 앱 시각 검증 → reflow 미세개선.

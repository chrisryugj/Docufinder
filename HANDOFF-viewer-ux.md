# 다음 세션 — 뷰 전환 UX 후속(발행·시각검증) + reflow 미세개선

> **트리거**: "뷰어 이어서" / "reflow 개선" / "docufinder 발행" → 이 파일로 시작.
> **직전 세션(2026-07-04, ultracode)에서 1순위 UX는 구현·검증 완료.** 아래는 남은 것.

---

## 0. 직전 세션 완성 (2026-07-04)

### docufinder — 미리보기 뷰 전환 UX 획기적 개선 ✅ (워킹트리, **미커밋**)
액션바에 묻혀있던 아이콘 토글 → 본문 위 **세그먼트 컨트롤**로 교체.
- `PreviewPanel.tsx` (+100/-23): 세그먼트(`문서 텍스트 | 원본 레이아웃`, HWPX 전용, raised pill·활성 명확)
  + 단축키 `1`/`2`(입력창·모달 밖) + 선호 뷰 **localStorage 전역 기억**(`docufinder:preview-view`)
  + 파일당 1회 자동 진입(`prefAppliedRef`). 액션바 `LayoutTemplate` 아이콘·`handleToggleLayout` 제거.
- `VersionDiffModal.tsx` (+2): 오버레이에 `role="dialog" aria-modal="true"` — 앱 전역 bare-key 가드 갭 수복.
- **적대적 검증 2라운드**(워크플로우, 13에이전트): 1R에서 5 CONFIRMED 결함 발견→전부 수정
  (①`desiredViewRef` 경쟁수정 = in-flight 렌더가 사용자 전환/인용점프를 덮지 않게, 성공 시 캐시만 저장·의도가 layout일 때만 setViewMode ②모달 가드 ③빈텍스트 `markdown!==null` ④`var(--shadow-sm)` ⑤`role=radiogroup/radio`).
  2R에서 경쟁 완전·new_defect 0 확인. tsc·vite 빌드 클린.
- **핵심 불변식**: `viewMode==="layout"` ⇔ `desiredViewRef.current==="layout"`. 이 정합을 깨지 말 것.

### kordoc — bench:gate 맥북 이식 ✅
맥미니 전용이라던 게 이제 이 맥북에서 완주. `bench/corpus/`(gitignore)에 맥미니(`ssh sm`)에서
review 45·hwp5 30·formats 27·pairs rsync 완료. **5체인 전부 PASS**(score/roundtrip/pdf-table/formats/fuzz).
reflow 자기일관성 11/12(seoul). ★맥 기본 rsync 구버전이라 `--info` 플래그 쓰지 말 것(usage 에러).

---

## 1. 남은 작업 (우선순위)

### A. 앱 시각 검증 (직전 세션 못함 — 화면 확인 불가)
`KORDOC_CLI_PATH=~/workspace/kordoc/dist/cli.js pnpm tauri:dev:mac` (★mac은 `:mac` — 기본 `tauri:dev`는 Windows용 `set` 문법).
확인: 세그먼트 전환 즉시성·활성 명확도, 단축키 1/2, 선호 기억(파일 바꿔도 유지), pref=layout 자동진입,
렌더 실패(HWP·조판캐시無) 폴백 토스트, 빈텍스트 HWPX에서 세그먼트 노출, Ctrl+F·AI인용점프 정합.

### B. 발행 (지시 대기 — main 직접 푸시 룰)
- **docufinder**: 버전 bump + CHANGELOG(뷰어 UX). 커밋 후 push. tauri:build는 `bundle-kordoc` 선행.
- **kordoc v3.15.0**: `npm publish` + `git tag v3.15.0 && git push --tags` (직전은 커밋 `7fb9885`만).
  ★fly deploy 금지 규칙 등 CLAUDE.md 준수.

### C. reflow 미세개선 (kordoc — `bench/verify-reflow.mjs` 게이트, 맥북에서 검증 가능)
직전 세션에 재현 확인된 타깃:
- **systematic 세로 offset** `dyMax≈176HWPUNIT(1.76pt)` 다수 파일 반복 — 계통 오차 추적.
- 자기일관성 1건 `match 81%`(`응답소 민원 처리 결과 회신`) = 긴 셀 텍스트 줄나눔 위치 초과.
- `lineSpacing` 비-PERCENT(FIXED/BETWEEN_LINES) pitch 분기 · intent hanging(내어쓰기) horzpos/contWidth 정밀화 · 표 페이지 중간 분할(현재 문단 단위만).
- 도형: 회전(`rotationInfo` angle)·`connectLine`·arc start/sweep 각 미해석.
- 세로모델 실측 박제 = `.claude/plans/render-poc/findings.md`(gitignore, 로컬).

### D. 백로그
- worker 동시요청/부하·강제재시작 실측 · ODL Phase3 · HWP3 표.
- docufinder LayoutView: 대화면 split(나란히) 뷰는 이번에 **의도적 제외**(사이드패널 320px엔 부적합). 풀뷰 생기면 재검토.

---

## 2. 첫 액션
1. `git pull`(양쪽) → 이 파일 정독.
2. 앱 띄워 A 시각 검증 → 이상 없으면 사용자 확인 후 **B 발행**.
3. 여력 시 C reflow 개선(verify-reflow 게이트로 회귀 방지).

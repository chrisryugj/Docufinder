# UI 하니스 (뷰어 + 검색 결과 리스트)

LayoutView(HWPX SVG)·PdfLayoutView·SearchResultList 를 Tauri 없이 브라우저 단독으로
띄워 헤드리스 WebKit(Tauri 맥 엔진 계열)에서 실구동 검증한다.
v3.2.3 의 "pgclip 정규식이 릴리스 빌드에서 전혀 매치되지 않던" 부류 — 코드만 봐서는
릴리스된 픽스가 실제로 안 도는 — 회귀를 잡기 위한 도구.

## 실행

```bash
# 1. 하니스 빌드 + 서버 (Tauri invoke 는 tauri-core-stub.ts 로 대체)
#    검증은 반드시 build+preview 로 — build 산출물에만 패키징 앱과 같은 CSP(meta) 가
#    주입된다. dev 서버는 vite 가 CSS 를 런타임 <style> 로 꽂아 CSP 를 걸 수 없어
#    "dev 에선 돌고 앱에서만 죽는" 부류(CSP 의 런타임 <style> 차단 → 줌·맞춤 무동작)를 놓친다.
pnpm exec vite build --config vite.harness.config.ts
pnpm exec vite preview --config vite.harness.config.ts

# (인터랙티브 디버깅용 dev 서버 — CSP 없음, 검증엔 쓰지 말 것)
pnpm exec vite --config vite.harness.config.ts

# 2. 검증 (playwright 필요 — 임의 위치에 npm i playwright && npx playwright install webkit)
node harness/verify-ui.mjs        # 뷰어 33항목
node harness/verify-results.mjs   # 검색 결과 클릭 UX 26항목 (?view=results 마운트)
```

## 검증 항목 (33)

- **LayoutView(팝업)**: 초기 페이지 맞춤 폭 정확도(첫 프레임), **svg 폭=host 폭(스케일
  CSS 실효 — CSP 가 런타임 <style> 을 차단하면 여기서 잡힌다)**, Ctrl/⌘+휠 줌 +
  `preventDefault` 실효(웹뷰 전체 줌 누수 차단), 일반 휠 스크롤 보존, ± 버튼,
  맞춤↔너비 토글, 더블클릭 2× 토글, Cmd/Ctrl +/−/0 키보드 줌, 페이지 네비 정렬, Esc 닫기
- **LayoutView(인라인)**: PreviewPanel 식 flex-col 형제 배치에서 뷰어가 패널 안에
  들어오고(하단 미절단) 맞춤 페이지가 보이는 높이에 들어옴
- **PdfLayoutView**: 초기 렌더 요청 1회(page 0), 페이지 맞춤 폭, 휠/키보드/더블클릭 줌
  (v3.2.4 에서 SVG 뷰와 동일 계약으로 정합), 페이지 네비, **파일 전환 시 이전 페이지
  번호로의 낭비 요청·stale pageCount 없음**, Esc 닫기

## 검증 항목 — 검색 결과 리스트 (26, verify-results.mjs)

v3.2.7 열기 방식 설정(`open_on_single_click`)·저장 위치 표시(`show_result_path`)의
실브라우저 실효 검증. `?view=results` 로 가짜 결과(플랫 2건·파일명 매치 동명 3곳·그룹
2건)를 마운트하고 콜백을 `window.__calls` 로 기록해 확인한다.

- **두 번 클릭 모드(기본)**: 파일명 한 번 클릭=선택·미리보기만(외부 열기 0), 두 번
  클릭=열기 1회, 스니펫·그룹 청크 행(role=button)·복사본("N곳") 배지 더블클릭 제외,
  카드 빈 영역 더블클릭 열기, 액션 버튼 오발 없음 — 플랫·그룹·파일명 매치 3면 동일 계약
- **저장 위치**: 기본 보기 브레드크럼(세그먼트 클릭 = 해당 폴더 열기 — 유닉스/윈도우
  구분자·절대경로 접두 보존), 컴팩트 보기도 경로 줄 유지 + 꼬리 우선 축약(말단 폴더
  보임), 경로 클릭=위치 열기(reveal), 표시 끔 동작. 파일명 매치는 브레드크럼 이식 +
  말단 세그먼트=reveal(탐색기에서 파일 선택), 중간 세그먼트=해당 폴더 열기
- **한 번 클릭 모드**: 구버전 즉시 열기 복원 + 카드 선택 버블 차단

알려진 관찰(비수정): 줌 변경 직후 페이지 표시가 다음 스크롤 이벤트까지 이전 값일 수
있음 — 표시만의 문제로 네비 자체는 정상.

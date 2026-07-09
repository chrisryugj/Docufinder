# 원본 레이아웃 뷰어 UI 하니스

LayoutView(HWPX SVG)·PdfLayoutView 를 Tauri 없이 브라우저 단독으로 띄워, 실kordoc
SVG(14페이지)로 확대축소 UI 를 헤드리스 WebKit(Tauri 맥 엔진 계열)에서 실구동 검증한다.
v3.2.3 의 "pgclip 정규식이 릴리스 빌드에서 전혀 매치되지 않던" 부류 — 코드만 봐서는
릴리스된 픽스가 실제로 안 도는 — 회귀를 잡기 위한 도구.

## 실행

```bash
# 1. 하니스 서버 (Tauri invoke 는 tauri-core-stub.ts 로 대체)
pnpm exec vite --config vite.harness.config.ts

# 2. 검증 (playwright 필요 — 임의 위치에 npm i playwright && npx playwright install webkit)
node harness/verify-ui.mjs
```

## 검증 항목 (30)

- **LayoutView**: 초기 페이지 맞춤 폭 정확도(첫 프레임), Ctrl/⌘+휠 줌 + `preventDefault`
  실효(웹뷰 전체 줌 누수 차단), 일반 휠 스크롤 보존, ± 버튼, 맞춤↔너비 토글,
  더블클릭 2× 토글, Cmd/Ctrl +/−/0 키보드 줌, 페이지 네비 정렬, Esc 닫기
- **PdfLayoutView**: 초기 렌더 요청 1회(page 0), 페이지 맞춤 폭, 휠/키보드/더블클릭 줌
  (v3.2.4 에서 SVG 뷰와 동일 계약으로 정합), 페이지 네비, **파일 전환 시 이전 페이지
  번호로의 낭비 요청·stale pageCount 없음**, Esc 닫기

알려진 관찰(비수정): 줌 변경 직후 페이지 표시가 다음 스크롤 이벤트까지 이전 값일 수
있음 — 표시만의 문제로 네비 자체는 정상.

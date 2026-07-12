/**
 * docufinder SVG/PDF 원본 레이아웃 뷰어 런타임 UI 검증 — 헤드리스 WebKit(Tauri 맥 엔진 계열).
 * 실kordoc SVG(14페이지)를 물려 초기 맞춤·휠/버튼/키보드 줌·맞춤 토글·더블클릭·페이지 네비·
 * preventDefault 실효·파일 전환 리셋을 실구동으로 확인한다.
 */
import { webkit } from "playwright";

const BASE = "http://127.0.0.1:5199";
const RATIO = 841.88 / 595.28; // 실SVG 페이지 비
let pass = 0, fail = 0;
const t = (name, ok, detail = "") => {
  console.log(`${ok ? "✅" : "❌"} ${name}${detail ? ` — ${detail}` : ""}`);
  ok ? pass++ : fail++;
};

const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1000, height: 720 } });
page.on("pageerror", (e) => console.log("PAGEERROR:", e.message));
await page.goto(BASE, { waitUntil: "networkidle" });

// ─── LayoutView (SVG) ───────────────────────────────
const host = page.locator(".layout-svg-host");
await host.waitFor({ state: "visible" });
await page.waitForTimeout(200);

// L1: 초기 페이지 맞춤 — 라벨 "맞춤" + host 폭 = min(availW, availH/ratio)
{
  const label = await page.locator("#stage span.w-9").innerText();
  const m = await page.evaluate(() => {
    const sc = document.querySelector("#stage .overflow-auto");
    const host = document.querySelector(".layout-svg-host");
    return {
      availW: sc.clientWidth - 32, availH: sc.clientHeight - 24,
      hostW: host.getBoundingClientRect().width,
      pg1H: document.querySelector('[data-page="1"]').getBoundingClientRect().height,
    };
  });
  const expected = Math.min(m.availW, m.availH / RATIO);
  t("L1 초기 페이지 맞춤 라벨", label === "맞춤", `label=${label}`);
  t("L1 초기 맞춤 폭 정확", Math.abs(m.hostW - expected) < 3, `host=${m.hostW.toFixed(1)} 기대=${expected.toFixed(1)}`);
  t("L1 첫 페이지가 컨테이너 높이에 들어옴", m.pg1H <= m.availH + 3, `pg1H=${m.pg1H.toFixed(1)} availH=${m.availH}`);
}

// L1.5: SVG 스케일 CSS 실효 — svg 렌더 폭이 host 폭을 따른다. 이 CSS 가 죽으면(패키징 앱
// CSP 가 런타임 <style> 차단하던 v3.2.x 부류) svg 는 고유 pt 크기로 굳어 줌·맞춤이 라벨만
// 바뀌고 그림은 불변이 된다. 하니스는 build+preview(CSP 주입)로 돌 때 이 부류를 잡는다.
{
  const m = await page.evaluate(() => {
    const host = document.querySelector(".layout-svg-host");
    const svg = host.querySelector("svg");
    return { hostW: host.getBoundingClientRect().width, svgW: svg.getBoundingClientRect().width };
  });
  t("L1.5 svg 폭 = host 폭 (스케일 CSS 실효)", Math.abs(m.svgW - m.hostW) < 2, `svg=${m.svgW.toFixed(1)} host=${m.hostW.toFixed(1)}`);
}

// L2: Ctrl+휠 줌 — 컴포넌트 줌 작동 + preventDefault 실효(브라우저 줌 누수 차단)
{
  const before = await host.evaluate((el) => el.getBoundingClientRect().width);
  await page.evaluate(() => (window.__wheelPrevented = []));
  const box = await host.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + 100);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -240);
  await page.keyboard.up("Control");
  await page.waitForTimeout(150);
  const after = await host.evaluate((el) => el.getBoundingClientRect().width);
  const prevented = await page.evaluate(() => window.__wheelPrevented);
  const label = await page.locator("#stage span.w-9").innerText();
  t("L2 Ctrl+휠 확대", after > before + 5, `${before.toFixed(0)}→${after.toFixed(0)}px, label=${label}`);
  t("L2 preventDefault 실효(웹뷰 줌 누수 차단)", prevented.length > 0 && prevented.every(Boolean), `records=${JSON.stringify(prevented)}`);
}

// L3: 일반 휠 = 스크롤(줌 아님) + preventDefault 안 함
{
  await page.evaluate(() => (window.__wheelPrevented = []));
  const before = await host.evaluate((el) => el.getBoundingClientRect().width);
  await page.mouse.wheel(0, 300);
  await page.waitForTimeout(150);
  const after = await host.evaluate((el) => el.getBoundingClientRect().width);
  const scrolled = await page.evaluate(() => document.querySelector("#stage .overflow-auto").scrollTop);
  const prevented = await page.evaluate(() => window.__wheelPrevented);
  t("L3 일반 휠 스크롤(폭 불변)", Math.abs(after - before) < 1 && scrolled > 0, `scrollTop=${scrolled}`);
  t("L3 일반 휠 기본동작 보존", prevented.every((p) => !p));
}

// L4: ± 버튼
{
  const labelBefore = await page.locator("#stage span.w-9").innerText();
  await page.locator('button[title="확대"]').click();
  await page.waitForTimeout(100);
  const labelAfter = await page.locator("#stage span.w-9").innerText();
  t("L4 확대 버튼 배율 증가", parseInt(labelAfter) > parseInt(labelBefore), `${labelBefore}→${labelAfter}`);
}

// L5: 맞춤 토글 — 수동 줌 → 맞춤 복귀 → 너비 맞춤 순환
{
  const fitBtn = page.locator('button[title*="맞춤"]');
  await fitBtn.click(); // 수동 줌 상태에서 → 페이지 맞춤
  await page.waitForTimeout(100);
  const l1 = await page.locator("#stage span.w-9").innerText();
  await fitBtn.click(); // → 너비 맞춤
  await page.waitForTimeout(100);
  const l2 = await page.locator("#stage span.w-9").innerText();
  const m = await page.evaluate(() => {
    const sc = document.querySelector("#stage .overflow-auto");
    const host = document.querySelector(".layout-svg-host");
    return { availW: sc.clientWidth - 32, hostW: host.getBoundingClientRect().width };
  });
  t("L5 맞춤 복귀", l1 === "맞춤", `label=${l1}`);
  t("L5 너비 맞춤 = 100%", l2 === "너비" && Math.abs(m.hostW - m.availW) < 3, `label=${l2}, host=${m.hostW.toFixed(0)} avail=${m.availW}`);
  await fitBtn.click(); // 다시 페이지 맞춤으로 복원
  await page.waitForTimeout(100);
}

// L6: 더블클릭 줌 토글 (팝업 모드) — 스크롤과 무관하게 '보이는' 컨테이너 중앙을 찍는다
{
  const scBox = await page.locator("#stage .overflow-auto").boundingBox();
  const cx = scBox.x + scBox.width / 2, cy = scBox.y + scBox.height / 2;
  await page.mouse.dblclick(cx, cy);
  await page.waitForTimeout(120);
  const l1 = await page.locator("#stage span.w-9").innerText();
  await page.mouse.dblclick(cx, cy);
  await page.waitForTimeout(120);
  const l2 = await page.locator("#stage span.w-9").innerText();
  t("L6 더블클릭 2× 확대", l1 === "200%", `label=${l1}`);
  t("L6 더블클릭 맞춤 원복", l2 === "맞춤", `label=${l2}`);
}

// L7: 키보드 줌 Cmd/Ctrl +/-/0 (팝업 모드)
{
  await page.keyboard.press("Control+=");
  await page.waitForTimeout(100);
  const l1 = await page.locator("#stage span.w-9").innerText();
  await page.keyboard.press("Control+0");
  await page.waitForTimeout(100);
  const l2 = await page.locator("#stage span.w-9").innerText();
  t("L7 Ctrl+= 확대", /%$/.test(l1), `label=${l1}`);
  t("L7 Ctrl+0 맞춤 리셋", l2 === "맞춤", `label=${l2}`);
}

// L8: 페이지 네비 — 현재 +1 로 이동하고, 타깃 페이지 상단이 컨테이너 상단에 정렬(block:start)
{
  const before = parseInt(await page.locator("#stage .tabular-nums").first().innerText());
  await page.locator('button[title="다음 페이지"]').click();
  await page.waitForTimeout(700); // smooth 스크롤
  const label = await page.locator("#stage .tabular-nums").first().innerText();
  const align = await page.evaluate((target) => {
    const sc = document.querySelector("#stage .overflow-auto");
    const pg = document.querySelector(`[data-page="${target}"]`);
    return Math.abs(pg.getBoundingClientRect().top - sc.getBoundingClientRect().top);
  }, before + 1);
  t("L8 다음 페이지 네비+정렬", label.startsWith(`${before + 1}/14`) && align < 20, `${before}→${label}, 상단 오차=${align.toFixed(0)}px`);
}

// L9: Esc → onClose
{
  await page.keyboard.press("Escape");
  await page.waitForTimeout(100);
  const closed = await page.evaluate(() => window.__closed);
  t("L9 Esc 닫기 콜백", closed === true);
}

// ─── LayoutView 인라인 모드 (PreviewPanel flex-col 형제 배치) ──────
// 팝업(#stage 직속)과 달리 헤더·툴바·푸터가 세로 공간을 나눠 갖는다. 루트가 h-full 단독이면
// 패널 전체 높이를 먹고 크롬 높이만큼 아래로 밀려 맞춤 페이지 하단·푸터가 잘린다(v3.2.4 결함).
{
  await page.evaluate(() => window.__setMode("inline"));
  await host.waitFor({ state: "visible" });
  await page.waitForTimeout(300);
  const m = await page.evaluate(() => {
    const stage = document.querySelector("#stage");
    const sc = document.querySelector("#stage .overflow-auto");
    const pg = document.querySelector('[data-page="1"]');
    return {
      stageBottom: stage.getBoundingClientRect().bottom,
      scBottom: sc.getBoundingClientRect().bottom,
      scH: sc.clientHeight,
      pg1H: pg.getBoundingClientRect().height,
    };
  });
  // 푸터(27px)가 스크롤 영역 아래 보여야 하므로 뷰어 하단은 스테이지 하단보다 위
  t("I1 인라인 뷰어가 패널 안에 들어옴(하단 미절단)", m.scBottom <= m.stageBottom - 25, `scBottom=${m.scBottom.toFixed(0)} stageBottom=${m.stageBottom.toFixed(0)}`);
  t("I1 인라인 맞춤 페이지가 보이는 높이에 들어옴", m.pg1H <= m.scH - 24 + 3, `pg1H=${m.pg1H.toFixed(1)} 가용=${m.scH - 24}`);
}

// ─── PdfLayoutView ──────────────────────────────────
await page.evaluate(() => { window.__closed = false; window.__invokeCalls = []; window.__setMode("pdf"); });
const pdfImg = page.locator("#stage img");
await pdfImg.waitFor({ state: "visible", timeout: 5000 });
await page.waitForTimeout(200);

// P1: 초기 로드 — invoke 1회(page 0), 라벨 1/3, 페이지 맞춤 폭
{
  const calls = await page.evaluate(() => window.__invokeCalls);
  const label = await page.locator("#stage .tabular-nums").first().innerText();
  const m = await page.evaluate(() => {
    const sc = document.querySelector("#stage .overflow-auto");
    const img = document.querySelector("#stage img");
    return { availW: sc.clientWidth - 32, availH: sc.clientHeight - 24, imgW: img.getBoundingClientRect().width };
  });
  const expected = Math.min(m.availW, m.availH * (595 / 842));
  t("P1 초기 렌더 요청 1회(page 0)", calls.length === 1 && calls[0].args.page === 0, JSON.stringify(calls.map((c) => c.args.page)));
  t("P1 페이지 라벨 1/3", label === "1/3", `label=${label}`);
  t("P1 페이지 맞춤 폭", Math.abs(m.imgW - expected) < 3, `img=${m.imgW.toFixed(1)} 기대=${expected.toFixed(1)}`);
}

// P2: Ctrl+휠 줌 + preventDefault 실효 (이번 수정 핵심)
{
  await page.evaluate(() => (window.__wheelPrevented = []));
  const before = await pdfImg.evaluate((el) => el.getBoundingClientRect().width);
  const box = await pdfImg.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + 100);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -240);
  await page.keyboard.up("Control");
  await page.waitForTimeout(150);
  const after = await pdfImg.evaluate((el) => el.getBoundingClientRect().width);
  const prevented = await page.evaluate(() => window.__wheelPrevented);
  t("P2 PDF Ctrl+휠 확대", after > before + 5, `${before.toFixed(0)}→${after.toFixed(0)}px`);
  t("P2 PDF preventDefault 실효 (v3.2.4 수정)", prevented.length > 0 && prevented.every(Boolean), `records=${JSON.stringify(prevented)}`);
}

// P3: 키보드 줌 (이번 추가)
{
  await page.keyboard.press("Control+0");
  await page.waitForTimeout(100);
  const l0 = await page.locator("#stage span.w-9").innerText();
  await page.keyboard.press("Control+=");
  await page.waitForTimeout(100);
  const l1 = await page.locator("#stage span.w-9").innerText();
  t("P3 PDF Ctrl+0 맞춤", l0 === "맞춤", `label=${l0}`);
  t("P3 PDF Ctrl+= 확대", /%$/.test(l1), `label=${l1}`);
}

// P4: 더블클릭 토글 (이번 추가)
{
  const box = await pdfImg.boundingBox();
  await page.mouse.dblclick(box.x + 60, box.y + 60);
  await page.waitForTimeout(120);
  const l1 = await page.locator("#stage span.w-9").innerText();
  await page.mouse.dblclick(box.x + 60, box.y + 60);
  await page.waitForTimeout(120);
  const l2 = await page.locator("#stage span.w-9").innerText();
  t("P4 PDF 더블클릭 2×", l1 === "200%", `label=${l1}`);
  t("P4 PDF 더블클릭 원복", l2 === "맞춤", `label=${l2}`);
}

// P5: 페이지 네비 → page 1 요청 + 라벨 2/3
{
  await page.evaluate(() => (window.__invokeCalls = []));
  await page.locator('button[title="다음 페이지"]').click();
  await page.waitForTimeout(300);
  const calls = await page.evaluate(() => window.__invokeCalls);
  const label = await page.locator("#stage .tabular-nums").first().innerText();
  t("P5 다음 페이지 렌더 요청", calls.length === 1 && calls[0].args.page === 1, JSON.stringify(calls.map((c) => c.args.page)));
  t("P5 라벨 2/3", label === "2/3", `label=${label}`);
}

// P6: 파일 전환 — 이전 page(2)로 낭비 요청 없이 page 0 한 번만 (v3.2.4 수정)
{
  await page.locator('button[title="다음 페이지"]').click(); // → page 3 (index 2)
  await page.waitForTimeout(300);
  await page.evaluate(() => (window.__invokeCalls = []));
  await page.evaluate(() => window.__setPdfFile("/fake/b.pdf"));
  await page.waitForTimeout(400);
  const calls = await page.evaluate(() => window.__invokeCalls);
  const label = await page.locator("#stage .tabular-nums").first().innerText();
  const errVisible = await page.locator("#stage").getByText("실패").count();
  t("P6 파일 전환 요청 = page 0 단 1회 (낭비/stale 없음)", calls.length === 1 && calls[0].args.page === 0, JSON.stringify(calls.map((c) => c.args.page)));
  t("P6 라벨 1/3 리셋 + 에러 없음", label === "1/3" && errVisible === 0, `label=${label}`);
}

// P8: 휠 페이지 넘김 — 경계에서 계속 굴리면 다음/이전 페이지 (v3.2.9).
// 현재 상태: b.pdf, 1/3, 페이지 맞춤(전체가 화면에 들어옴 → 항상 상·하단 경계).
{
  const sc = await page.locator("#stage .overflow-auto").boundingBox();
  await page.mouse.move(sc.x + sc.width / 2, sc.y + sc.height / 2);
  await page.evaluate(() => (window.__invokeCalls = []));
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(350);
  let label = await page.locator("#stage .tabular-nums").first().innerText();
  const calls = await page.evaluate(() => window.__invokeCalls);
  t("P8 경계 휠다운 → 다음 페이지", label === "2/3" && calls.some((c) => c.args.page === 1), `label=${label}`);
  await page.waitForTimeout(250); // 재무장 갭(180ms) 경과
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(350);
  label = await page.locator("#stage .tabular-nums").first().innerText();
  t("P8 재무장 후 휠다운 → 3/3", label === "3/3", `label=${label}`);
  await page.waitForTimeout(250);
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(300);
  label = await page.locator("#stage .tabular-nums").first().innerText();
  t("P8 마지막 페이지에서 휠다운 → 유지", label === "3/3", `label=${label}`);
}

// P9: 휠업 ← 이전 페이지 + 확대 상태에선 하단 정렬로 진입 (읽던 흐름 유지)
{
  // 확대해 세로 오버플로 생성 → 이전 페이지 진입 시 하단 정렬이 관측 가능해짐
  for (let i = 0; i < 4; i++) await page.locator('button[title="확대"]').click();
  await page.waitForTimeout(300);
  // 확대 버튼 클릭이 커서를 툴바로 옮김 — 휠 표적을 스크롤러로 복귀
  const scBox = await page.locator("#stage .overflow-auto").boundingBox();
  await page.mouse.move(scBox.x + scBox.width / 2, scBox.y + scBox.height / 2);
  const overflow = await page.evaluate(() => {
    const sc = document.querySelector("#stage .overflow-auto");
    return sc.scrollHeight > sc.clientHeight + 10;
  });
  await page.evaluate(() => document.querySelector("#stage .overflow-auto").scrollTo(0, 0));
  await page.waitForTimeout(250);
  await page.mouse.wheel(0, -120);
  await page.waitForTimeout(500); // 렌더 + 이미지 onLoad 하단 정렬
  const label = await page.locator("#stage .tabular-nums").first().innerText();
  const nearBottom = await page.evaluate(() => {
    const sc = document.querySelector("#stage .overflow-auto");
    return sc.scrollHeight - sc.clientHeight - sc.scrollTop < 3;
  });
  t("P9 상단 경계 휠업 → 이전 페이지", label === "2/3", `label=${label}`);
  t("P9 이전 페이지는 하단 정렬 진입", overflow && nearBottom, `overflow=${overflow} nearBottom=${nearBottom}`);
}

// P7: Esc 닫기
{
  await page.keyboard.press("Escape");
  await page.waitForTimeout(100);
  const closed = await page.evaluate(() => window.__closed);
  t("P7 PDF Esc 닫기 콜백", closed === true);
}

console.log(`\n결과: ${pass} PASS / ${fail} FAIL`);
await browser.close();
process.exit(fail > 0 ? 1 : 0);

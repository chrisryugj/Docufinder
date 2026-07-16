/**
 * 파일명 검색 컬럼 뷰(Everything식) 레이아웃 검증 — 헤드리스 WebKit.
 * 창 폭 축소 시 컬럼 겹침(v3.3.4 버그)·재확대 시 선호 폭 복원·유형 셀 뱃지/액션
 * 공간 분리를 실브라우저에서 계측한다. 실행 전 build+preview 필요 (README 참조),
 * /filename.html 로 마운트.
 */
import { webkit } from "playwright";

const BASE = "http://127.0.0.1:5199/filename.html";
let pass = 0, fail = 0;
const t = (name, ok, detail = "") => {
  console.log(`${ok ? "✅" : "❌"} ${name}${detail ? ` — ${detail}` : ""}`);
  ok ? pass++ : fail++;
};

const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.on("pageerror", (e) => console.log("PAGEERROR:", e.message));
await page.addInitScript(() => localStorage.clear()); // 폭 기억 초기화 — 결정적 실행
await page.goto(BASE, { waitUntil: "networkidle" });
await page.locator("#search-result-0").waitFor({ state: "visible" });
await page.waitForTimeout(400); // stagger-item 애니메이션 안정화

const setStage = async (px) => {
  await page.evaluate((w) => window.__setStageWidth(w), px);
  await page.waitForTimeout(150); // ResizeObserver → setState 반영 대기
};

/** 헤더가 렌더한 grid-template-columns 실효값(px 목록) */
const trackSizes = () =>
  page.evaluate(() =>
    getComputedStyle(document.querySelector('[role="row"]'))
      .gridTemplateColumns.split(" ")
      .map((v) => Math.round(parseFloat(v)))
  );

/** 각 행의 "칠해지는" 콘텐츠(경로 버튼·크기·시간 텍스트·뱃지)의 bounding box 목록 */
const rowContentBoxes = (idx) =>
  page.evaluate((i) => {
    const row = document.querySelector(`#search-result-${i} > .grid`);
    const cells = [...row.children];
    const pick = (el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
    };
    // 셀 안의 실제 텍스트/뱃지 요소 (셀 div 가 아니라 칠해지는 내용물)
    const content = cells.map((c) => {
      const inner = c.querySelector("button, span") || c;
      return pick(inner);
    });
    return { cells: cells.map(pick), content, container: pick(row) };
  }, idx);

const overlapX = (a, b) => Math.min(a.right, b.right) - Math.max(a.left, b.left);

/** 한 행에서 인접 콘텐츠끼리 수평 겹침이 있는지 (0.5px 톨러런스) */
async function assertNoOverlap(label, rowIdx, { checkSpill = true } = {}) {
  const { content, cells, container } = await rowContentBoxes(rowIdx);
  let worst = 0, worstPair = "";
  for (let i = 0; i < content.length - 1; i++) {
    for (let j = i + 1; j < content.length; j++) {
      const ov = overlapX(content[i], content[j]);
      if (ov > worst) { worst = ov; worstPair = `${i}↔${j}`; }
    }
  }
  t(`${label}: 셀 콘텐츠 비중첩`, worst <= 0.5, worst > 0.5 ? `${worstPair} ${worst.toFixed(1)}px 겹침` : "");
  if (checkSpill) {
    const spill = Math.max(...cells.map((c) => c.right)) - container.right;
    t(`${label}: 셀이 컨테이너 밖으로 안 나감`, spill <= 0.5, spill > 0.5 ? `${spill.toFixed(1)}px 초과` : "");
  }
}

// ── 1. 기준 폭(1000px)에서 선호 폭 = 기본값 렌더 ──────────────────────
const wide = await trackSizes();
t("기준(1000px): 트랙 5개", wide.length === 5, `[${wide}]`);
t("기준(1000px): 기본 선호 폭 렌더 (260/360/96/150)",
  wide[0] === 260 && wide[1] === 360 && wide[2] === 96 && wide[3] === 150, `[${wide}]`);

// ── 2. 좁힘(540px ≥ MIN 총합 532px) — 겹침·오버플로 없음 ───────────────
await setStage(540);
await assertNoOverlap("좁힘(540px) 행0", 0);
await assertNoOverlap("좁힘(540px) 행1(긴 이름)", 1);
{
  const narrow = await trackSizes();
  const total = narrow.reduce((a, b) => a + b, 0) + 8 * 4 + 20; // gap 4 + px-2.5
  t("좁힘(540px): 트랙+gap+pad 총합이 컨테이너 안", total <= 540 + 0.5, `합 ${total}px`);
}

// ── 3. 한계 밑(420px < MIN 총합) — 겹침 없음 + 래퍼가 페인팅 clip ──────
await setStage(420);
await assertNoOverlap("한계 밑(420px) 행0", 0, { checkSpill: false });
await assertNoOverlap("한계 밑(420px) 행1", 1, { checkSpill: false });
{
  const clip = await page.evaluate(() => {
    const wrapper = document.querySelector('[role="row"]').parentElement;
    return getComputedStyle(wrapper).overflowX;
  });
  t("한계 밑(420px): 래퍼 overflow-x=clip (트랙 오버플로 페인팅 차단)", clip === "clip", `overflow-x=${clip}`);
}

// ── 4. 재확대(1000px) — 선호 폭 복원 ──────────────────────────────────
await setStage(1000);
const restored = await trackSizes();
t("재확대(1000px): 선호 폭 복원",
  restored[0] === 260 && restored[1] === 360 && restored[2] === 96 && restored[3] === 150,
  `[${restored}] (기대 260/360/96/150)`);
// 영속 상태(localStorage)가 축소로 오염되지 않았는지
const stored = await page.evaluate(() => localStorage.getItem("docufinder_filename_columns"));
const storedW = stored ? JSON.parse(stored) : null;
t("재확대(1000px): 영속 폭 미오염",
  !storedW || (storedW.name === 260 && storedW.path === 360 && storedW.size === 96 && storedW.time === 150),
  stored ?? "(미저장=기본값)");

// ── 5. 드래그 리사이즈한 선호 폭도 축소→재확대 왕복 후 복원 ────────────
{
  await setStage(1200); // 1000px에선 예산 clamp(max 282)에 걸려 +60 드래그이 안 들어간다
  // 이름 컬럼 경계를 +60px 드래그
  const sep = page.locator('[role="row"] [role="separator"]').first();
  const box = await sep.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 5 });
  await page.mouse.up();
  await page.waitForTimeout(350); // localStorage debounce(250ms) 반영
  const dragged = await trackSizes();
  t("드래그(1200px): 이름 폭 260→320", dragged[0] === 320, `[${dragged}]`);
  await setStage(420);
  await setStage(1200);
  const back = await trackSizes();
  t("드래그 후 축소→재확대: 선호 폭(320) 복원", back[0] === 320, `[${back}]`);
  const stored2 = JSON.parse(await page.evaluate(() => localStorage.getItem("docufinder_filename_columns")));
  t("드래그 후 축소→재확대: 영속 폭=320 유지", stored2.name === 320, JSON.stringify(stored2));
}

// ── 6. 유형 셀 — hover 액션 표시 상태에서 뱃지 미침범 ──────────────────
async function typeCellBoxes() {
  return page.evaluate(() => {
    const row = document.querySelector("#search-result-0 > .grid");
    const cell = row.lastElementChild; // 유형 셀
    const badge = cell.querySelector("span[aria-label^='파일 형식']");
    const overlay = cell.querySelector("div");
    const btns = [...cell.querySelectorAll("button")];
    const rect = (el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left, right: r.right, width: r.width };
    };
    return {
      cell: rect(cell),
      badge: rect(badge),
      btns: btns.map(rect),
      overlayOpacity: getComputedStyle(overlay).opacity,
      overlayBg: getComputedStyle(overlay).backgroundColor,
    };
  });
}
async function assertTypeCell(label, { expectNoCover }) {
  // hover 전 — 뱃지가 셀 안에 온전 (액션이 흐름에서 빠져 공간을 안 밀어야 함)
  await page.mouse.move(5, 5);
  await page.waitForTimeout(250);
  const idle = await typeCellBoxes();
  t(`${label}: hover 전 액션 숨김`, parseFloat(idle.overlayOpacity) === 0, `opacity=${idle.overlayOpacity}`);
  t(`${label}: hover 전 뱃지 셀 안에 온전`,
    idle.badge.width > 40 && idle.badge.left >= idle.cell.left - 0.5 && idle.badge.right <= idle.cell.right + 0.5,
    `badge(${idle.badge.left.toFixed(0)}-${idle.badge.right.toFixed(0)}) cell(${idle.cell.left.toFixed(0)}-${idle.cell.right.toFixed(0)})`);
  // hover — 액션 표시
  await page.hover("#search-result-0");
  await page.waitForTimeout(250); // opacity 트랜지션
  const r = await typeCellBoxes();
  t(`${label}: hover 시 액션 표시`, parseFloat(r.overlayOpacity) === 1, `opacity=${r.overlayOpacity}`);
  const inCell = r.btns.every((b) => b.right <= r.cell.right + 0.5 && b.left >= r.cell.left - 0.5);
  t(`${label}: 액션이 유형 셀 영역 안 (옆 컬럼 미침범)`, inCell,
    r.btns.map((b) => `btn(${b.left.toFixed(0)}-${b.right.toFixed(0)})`).join(",") + ` cell(${r.cell.left.toFixed(0)}-${r.cell.right.toFixed(0)})`);
  const worst = Math.max(...r.btns.map((b) => overlapX(r.badge, b)));
  if (expectNoCover) {
    t(`${label}: 아이콘이 뱃지 미침범`, worst <= 0.5, worst > 0.5 ? `${worst.toFixed(1)}px 겹침` : "");
  } else {
    // 극단적으로 좁으면 덮는 것이 사양(VS Code식) — 대신 불투명 배경으로 깨끗하게 덮어야 함
    const opaque = !/rgba\(.*,\s*0\)$/.test(r.overlayBg) && r.overlayBg !== "transparent";
    t(`${label}: 덮을 땐 불투명 배경`, opaque, `bg=${r.overlayBg}, 겹침 ${worst.toFixed(1)}px`);
  }
}
// 넓은 폭: 유형 컬럼 여유(1fr 잔여 ≥ 100px) — 뱃지·아이콘 완전 분리
await setStage(1200);
await assertTypeCell("유형 셀(1200px)", { expectNoCover: true });
// 유형 컬럼이 최소폭(60px) 근처 — 오버레이가 뱃지를 불투명 배경으로 덮는 것까지 허용
await setStage(560);
await assertTypeCell("유형 셀(560px, TYPE_MIN 근처)", { expectNoCover: false });

await browser.close();
console.log(`\n${pass} pass / ${fail} fail`);
process.exit(fail ? 1 : 0);

/**
 * 창 불러오기 단축키 '직접 지정' 녹화 검증 (#52) — 헤드리스 Chromium(WebView2 엔진)·WebKit(맥 엔진).
 * 녹화 시작·수식키 안내·맨 글자키 거부·Ctrl+Alt+K 저장·녹화 중 앱 단축키 차단·Esc 취소를 실구동으로 확인한다.
 */
import { chromium, webkit } from "playwright";

const BASE = "http://127.0.0.1:5199/shortcut.html";
let pass = 0, fail = 0;
const t = (name, ok, detail = "") => {
  console.log(`${ok ? "✅" : "❌"} ${name}${detail ? ` — ${detail}` : ""}`);
  ok ? pass++ : fail++;
};

for (const [label, engine] of [["chromium", chromium], ["webkit", webkit]]) {
  console.log(`\n── ${label}`);
  const browser = await engine.launch();
  const page = await browser.newPage();
  page.on("pageerror", (e) => console.log("PAGEERROR:", e.message));
  await page.goto(BASE, { waitUntil: "networkidle" });
  const value = () => page.evaluate(() => window.__value);
  const appKeys = () => page.evaluate(() => window.__appKeys);
  const status = async () => ((await page.locator('[role="status"]').count()) ? page.locator('[role="status"]').innerText() : null);
  const recBtn = page.locator("button[aria-pressed]");

  t("프리셋 값 표시", await page.getByText("Ctrl + Alt + Space").isVisible());

  await recBtn.click();
  t("녹화 시작 안내", (await status())?.includes("쓰고 싶은 조합을 누르세요") ?? false, await status());
  const before = await appKeys();

  await page.keyboard.press("KeyK");
  t("맨 글자키 거부", (await status()) === "Ctrl 이나 Alt 를 함께 눌러 주세요", await status());
  await page.keyboard.press("Shift+KeyK");
  t("Shift+글자 거부", (await status()) === "Ctrl 이나 Alt 를 함께 눌러 주세요", await status());

  await page.keyboard.down("Control");
  await page.keyboard.down("Alt");
  t("수식키 누르는 중 안내", ((await status()) ?? "").includes("+ …"), await status());
  await page.keyboard.press("KeyK");
  await page.keyboard.up("Alt");
  await page.keyboard.up("Control");
  t("Ctrl+Alt+K 저장", (await value()) === "Ctrl+Alt+KeyK", await value());
  t("녹화 끝남", (await status()) === null && (await recBtn.innerText()) === "직접 지정");
  t("녹화 중 앱 키 처리 0회", (await appKeys()) === before, `${before} → ${await appKeys()}`);
  // 맥 UA 에선 Alt 를 Option 으로 표시한다
  t("직접 지정 값 표시", await page.getByText(/^Ctrl \+ (Alt|Option) \+ K \(직접 지정\)$/).isVisible());

  await page.keyboard.press("KeyJ");
  t("녹화 끝난 뒤 앱 키 처리 복구", (await appKeys()) === before + 1);

  await recBtn.click();
  await page.keyboard.press("Escape");
  t("Esc 로 취소", (await status()) === null && (await value()) === "Ctrl+Alt+KeyK");

  await recBtn.click();
  await page.keyboard.press("Control+Shift+Digit5");
  t("Ctrl+Shift+5 저장", (await value()) === "Ctrl+Shift+Digit5", await value());

  await recBtn.click();
  await recBtn.click();
  t("버튼으로 취소", (await status()) === null);

  await browser.close();
}

console.log(`\n${pass} 통과 / ${fail} 실패`);
process.exit(fail ? 1 : 0);

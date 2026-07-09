/**
 * @tauri-apps/api/core 스텁 — 하니스에서 PdfLayoutView 의 render_pdf_page 를
 * 브라우저 단독으로 흉내낸다. 호출 기록은 window.__invokeCalls 로 검증용 노출.
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const w = window as unknown as { __invokeCalls?: Array<{ cmd: string; args: unknown }> };
  (w.__invokeCalls ||= []).push({ cmd, args });
  if (cmd === "render_pdf_page") {
    const page = (args as { page: number }).page;
    if (page < 0 || page >= 3) throw `페이지 범위 밖: ${page}`;
    const c = document.createElement("canvas");
    c.width = 595;
    c.height = 842;
    const ctx = c.getContext("2d")!;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, 595, 842);
    ctx.fillStyle = "#000000";
    ctx.font = "24px sans-serif";
    ctx.fillText(`page ${page + 1}`, 40, 60);
    return {
      data_url: c.toDataURL("image/png"),
      page_count: 3,
      width: 595,
      height: 842,
    } as T;
  }
  throw new Error(`unmocked invoke: ${cmd}`);
}

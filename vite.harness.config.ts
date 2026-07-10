// UI 하니스 전용 vite 설정 — 실컴포넌트를 브라우저 단독으로 띄운다 (Tauri invoke 스텁).
// 사용: pnpm exec vite --config vite.harness.config.ts   (임시 파일 — 검증 후 삭제)
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

// 패키징 앱의 실효 CSP 재현 — Tauri 는 index.html <style> 에 nonce 를 주입하고 style-src 에
// nonce/hash 를 추가한다. CSP3 규칙상 nonce/hash 가 있으면 'unsafe-inline' 이 무효화되어
// JS 가 런타임에 삽입하는 <style> 태그가 패키징 앱에서만 차단된다. 하니스도 같은 조건으로
// 검증해야 "dev 에선 돌고 앱에서만 죽는" 부류(v3.2.x 줌·맞춤 무동작)를 잡는다.
// vite dev 서버는 CSS 를 런타임 <style> 로 주입하므로 build 산출물에만 적용한다.
const APP_LIKE_CSP =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' 'sha256-0000000000000000000000000000000000000000000='; font-src 'self'; img-src 'self' data:";

export default defineConfig({
  root: "harness",
  plugins: [
    react(),
    tailwindcss(),
    {
      name: "harness-app-like-csp",
      apply: "build",
      transformIndexHtml(html) {
        return html.replace(
          "<head>",
          `<head>\n    <meta http-equiv="Content-Security-Policy" content="${APP_LIKE_CSP}">`,
        );
      },
    },
  ],
  resolve: {
    alias: {
      "@tauri-apps/api/core": fileURLToPath(new URL("./harness/tauri-core-stub.ts", import.meta.url)),
    },
  },
  server: { host: "127.0.0.1", port: 5199, strictPort: true },
  preview: { host: "127.0.0.1", port: 5199, strictPort: true },
});

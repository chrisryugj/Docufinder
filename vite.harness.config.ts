// UI 하니스 전용 vite 설정 — 실컴포넌트를 브라우저 단독으로 띄운다 (Tauri invoke 스텁).
// 사용: pnpm exec vite --config vite.harness.config.ts   (임시 파일 — 검증 후 삭제)
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  root: "harness",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@tauri-apps/api/core": fileURLToPath(new URL("./harness/tauri-core-stub.ts", import.meta.url)),
    },
  },
  server: { host: "127.0.0.1", port: 5199, strictPort: true },
});

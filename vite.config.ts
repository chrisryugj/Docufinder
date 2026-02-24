import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

  // Tauri expects a fixed port
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
    // WebView2 초기 로딩 최적화: 소스 파일 사전 변환
    warmup: {
      clientFiles: ["./src/main.tsx", "./src/App.tsx", "./src/components/**/*.tsx", "./src/hooks/**/*.ts"],
    },
  },

  // 의존성 사전 번들링 최적화 (HTTP 요청 수 감소)
  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react/jsx-runtime",
      "@tauri-apps/api/core",
      "@tauri-apps/api/window",
      "@tauri-apps/api/event",
      "@tauri-apps/plugin-dialog",
      "@tauri-apps/plugin-process",
      "lucide-react",
    ],
  },

  // Build settings for Tauri
  build: {
    target: "esnext",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },

  // 프로덕션 빌드에서 console.log 제거 (보안: 디버그 정보 노출 방지)
  esbuild: {
    drop: process.env.TAURI_DEBUG ? [] : ["console", "debugger"],
  },

  // Prevent Vite from obscuring Rust errors
  clearScreen: false,
});

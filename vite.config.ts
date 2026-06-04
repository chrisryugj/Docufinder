import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

  // Tauri expects a fixed port
  server: {
    host: "127.0.0.1",
    port: 5174,
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
    chunkSizeWarningLimit: 600,
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      output: {
        manualChunks: {
          // 마크다운 + 수식 플러그인 (react-markdown + remark/rehype). PreviewPanel/
          // AiAnswerPanel 이 lazy 라 이 청크도 async 분리 → 초기 번들에서 빠진다 (P2-1).
          markdown: ["react-markdown", "remark-gfm", "remark-math", "rehype-katex"],
          // katex 본체(~267kB)만 leaf 청크로 분리 — rehype-katex 를 여기 넣으면
          // markdown↔katex 순환 청크가 생기므로 leaf 인 katex 만 둔다.
          katex: ["katex"],
          // 아이콘 라이브러리
          icons: ["lucide-react"],
        },
      },
    },
  },

  // 프로덕션 빌드에서 console.log 제거 (보안: 디버그 정보 노출 방지)
  esbuild: {
    drop: process.env.TAURI_DEBUG ? [] : ["console", "debugger"],
  },

  // Prevent Vite from obscuring Rust errors
  clearScreen: false,
});

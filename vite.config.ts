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
        // 함수형: pnpm 은 실제 모듈이 node_modules/.pnpm/<pkg>@<ver>/node_modules/<pkg>/… 에 있어
        // 객체형 목록이 하위 모듈(react/cjs/…)을 못 잡는다. 마지막 node_modules 뒤 패키지 이름으로 가른다.
        manualChunks(id) {
          const tail = id.split(/node_modules[\\/]/).pop();
          if (!tail || tail === id) return undefined;
          const parts = tail.split(/[\\/]/);
          const pkg = parts[0].startsWith("@") ? `${parts[0]}/${parts[1]}` : parts[0];
          // React 런타임은 따로 둔다. 안 그러면 react/jsx-runtime 이 react-markdown 쪽 청크에 묶여
          // 진입 청크가 markdown 청크를(그 청크가 다시 katex 를) 정적으로 끌어와, 시작할 때마다
          // 미리보기 전용 코드 ~430kB 를 받고 파싱했다. 마크다운·수식 라이브러리는 lazy 인
          // PreviewPanel/AiAnswerPanel 에서만 쓰여 Rollup 이 비동기 공유 청크로 알아서 뗀다.
          if (pkg === "react" || pkg === "react-dom" || pkg === "scheduler") return "vendor";
          // katex 본체(~267kB)는 leaf 청크로
          if (pkg === "katex") return "katex";
          // 아이콘 라이브러리
          if (pkg === "lucide-react") return "icons";
          return undefined;
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

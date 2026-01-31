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

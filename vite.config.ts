/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    host: host || false,
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": { target: process.env.ORBIT_API_TARGET || "http://127.0.0.1:1430" },
      "/healthz": { target: process.env.ORBIT_API_TARGET || "http://127.0.0.1:1430" },
    },
    // Native build outputs can be locked by the Windows linker. They do not
    // participate in frontend HMR and must not be watched by Vite.
    watch: {
      ignored: ["**/target/**", "**/src-tauri/**", "**/cloud/**", "**/crates/**", "**/audit-results/**", "**/competition/**", "**/.orbit-server/**", "**/*.tmpdir/**", "**/.*.tmpdir/**"],
    },
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: { target: "es2022", minify: false, sourcemap: true },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});

import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";

const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));

// The dev server (the Tauri window in development, and the browser preview) listens on this PC
// only. To reach the preview from another device, opt in: KEYLUME_DEV_HOST=0.0.0.0 npm run dev
const host = process.env.KEYLUME_DEV_HOST || "127.0.0.1";

// Tauri expects a fixed port; the dev server also serves the browser-only mock mode.
export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(version) }, // shown by the browser preview's Settings
  clearScreen: false,
  server: { port: 1420, strictPort: true, host },
  preview: { host },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  // public/ only holds the browser preview's mock library: keep it out of the app
  build: { target: "es2021", sourcemap: false, chunkSizeWarningLimit: 1500, copyPublicDir: false },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});

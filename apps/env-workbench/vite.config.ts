import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      ...(command === "serve"
        ? {
            "@keeldesk/react": fileURLToPath(
              new URL("../../packages/react/src/index.ts", import.meta.url),
            ),
          }
        : {}),
    },
  },
  optimizeDeps: {
    include: [
      "react",
      "react-dom/client",
      "lucide-react",
      "radix-ui",
      "class-variance-authority",
      "clsx",
      "tailwind-merge",
      "@tauri-apps/api/core",
      "@tauri-apps/api/event",
    ],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1520,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1521,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));

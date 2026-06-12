import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ command }) => ({
  base: process.env.VITE_BASE ?? "/",
  plugins: [react()],
  resolve: {
    alias: {
      "@graph/ui":
        command === "serve"
          ? path.resolve(__dirname, "../../packages/graph-ui/src/index.ts")
          : path.resolve(__dirname, "../../packages/graph-ui/dist/graph-ui.js"),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
}));

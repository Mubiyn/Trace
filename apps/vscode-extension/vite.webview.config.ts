import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@graph/ui": path.resolve(__dirname, "../../packages/graph-ui/src/index.ts"),
    },
  },
  root: path.resolve(__dirname, "src/webview"),
  build: {
    outDir: path.resolve(__dirname, "media"),
    emptyOutDir: true,
    rollupOptions: {
      input: path.resolve(__dirname, "src/webview/index.html"),
      output: {
        entryFileNames: "assets/index.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: "assets/index.[ext]",
      },
    },
  },
});

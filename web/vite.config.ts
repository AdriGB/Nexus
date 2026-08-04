import { defineConfig } from "vite";

export default defineConfig({
  // FIX #10: Relative base — works locally and on GitHub Pages
  base: "./",
  publicDir: "public",
  build: {
    target: "esnext",
    outDir: "dist",
  },
  server: {
    open: true,
  },
});

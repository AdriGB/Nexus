import { defineConfig } from "vite";

export default defineConfig({
  publicDir: "public",
  build: {
    target: "esnext",
    outDir: "dist",
  },
  server: {
    open: true,
  },
});

import { defineConfig } from "vite";

// The workspace libraries ship TypeScript source (main → ./src/index.ts). Exclude
// them from dep pre-bundling so Vite processes them from source (resolving the
// `.js` import specifiers to their `.ts` siblings, same as Vitest does), and allow
// serving files from the monorepo root.
export default defineConfig({
  optimizeDeps: { exclude: ["@cyclevania/core", "@cyclevania/examples"] },
  server: { fs: { allow: ["../.."] }, open: false },
  build: { target: "es2022" },
});

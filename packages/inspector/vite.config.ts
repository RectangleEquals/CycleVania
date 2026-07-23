import { defineConfig } from "vite";

// The inspector consumes @cyclevania/core + /examples as TS source (workspace deps),
// so no pre-build is needed; Vite resolves `.js` specifiers to their `.ts` sources.
export default defineConfig({
  root: ".",
  build: { outDir: "dist", target: "es2022", chunkSizeWarningLimit: 2000 },
  server: { port: 5173, strictPort: false },
});

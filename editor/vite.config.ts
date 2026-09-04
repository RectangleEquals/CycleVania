import { defineConfig } from "vite";

// ⚠ **No framework plugin, deliberately.** `10-editor.md` §10 says the layout needs mockups rather than
// prose, and choosing a component model before the layout is choosing in the dark. Plain TypeScript
// until there is something to lay out.
export default defineConfig({
  server: {
    // The API lives in the editor's own service; vite proxies to it so the browser sees one origin.
    proxy: { "/api": "http://127.0.0.1:5174" },
  },
  build: { target: "es2023" },
});

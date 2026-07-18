import { defineConfig } from "vitest/config";

// Each package with tests is a project so `vitest run` at the root executes the
// whole suite (core unit + golden parity, examples soak).
export default defineConfig({
  test: {
    projects: ["packages/core", "packages/examples"],
  },
});

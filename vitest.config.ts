import { defineConfig } from "vitest/config";

// One root config: the whole workspace test suite runs from the repo root
// (`pnpm test`). Never run vitest from inside a package directory — that
// duplicates the workspace path and fails with a bogus "non-existing file".
export default defineConfig({
  test: {
    include: ["packages/*/src/**/*.test.ts"],
    testTimeout: 15000,
  },
});

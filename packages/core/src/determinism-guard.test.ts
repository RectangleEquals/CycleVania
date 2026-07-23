import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * The determinism guard (redesign 02 rule 8): no `Math.random` and no host trig
 * (`Math.sin/cos/tan/atan`) anywhere in shipped generation source, and no
 * Node-only APIs (`node:` imports, `require(`) in non-test core/examples source
 * (core is engine/host-agnostic). Test files are exempt (they run on Node and may
 * reference the forbidden tokens deliberately, as this file does).
 */

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../../.."); // packages/core/src -> repo root

const SCAN_ROOTS = ["packages/core/src", "packages/examples/src"];

function tsFiles(dir: string): string[] {
  const out: string[] = [];
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, ent.name);
    if (ent.isDirectory()) out.push(...tsFiles(full));
    else if (ent.name.endsWith(".ts") && !ent.name.endsWith(".test.ts")) out.push(full);
  }
  return out;
}

function stripComments(src: string): string {
  return src.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/\/\/[^\n]*/g, "");
}

describe("determinism guard", () => {
  const files = SCAN_ROOTS.flatMap((r) => {
    const abs = join(repoRoot, r);
    return existsSync(abs) ? tsFiles(abs) : [];
  });

  it("scans at least the core math + diagnostics source", () => {
    expect(files.length).toBeGreaterThan(5);
  });

  it("contains no Math.random", () => {
    const offenders = files.filter((f) => /Math\.random/.test(stripComments(readFileSync(f, "utf8"))));
    expect(offenders.map((f) => relative(repoRoot, f))).toEqual([]);
  });

  it("contains no host trigonometry (Math.sin/cos/tan/atan)", () => {
    const offenders = files.filter((f) => /Math\.(sin|cos|tan|atan)\b/.test(stripComments(readFileSync(f, "utf8"))));
    expect(offenders.map((f) => relative(repoRoot, f))).toEqual([]);
  });

  it("contains no Node-only APIs in shipped source (node: imports / require)", () => {
    const offenders = files.filter((f) => {
      const s = stripComments(readFileSync(f, "utf8"));
      return /from\s+["']node:/.test(s) || /\brequire\s*\(/.test(s);
    });
    expect(offenders.map((f) => relative(repoRoot, f))).toEqual([]);
  });
});

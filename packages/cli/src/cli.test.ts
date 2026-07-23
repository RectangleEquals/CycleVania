import { describe, it, expect } from "vitest";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "./main.js";

const tmp = (): string => mkdtempSync(join(tmpdir(), "cvcli-"));

const bracketsBalanced = (s: string): boolean => {
  const pairs: Record<string, string> = { ")": "(", "]": "[", "}": "{" };
  const opens = new Set(["(", "[", "{"]);
  const stack: string[] = [];
  for (const ch of s) {
    if (opens.has(ch)) stack.push(ch);
    else if (ch in pairs) {
      if (stack.pop() !== pairs[ch]) return false;
    }
  }
  return stack.length === 0;
};

describe("cli — help + dispatch", () => {
  it("--help lists every subcommand", async () => {
    const r = await runCli(["--help"]);
    expect(r.code).toBe(0);
    for (const c of ["generate", "validate", "soak", "report", "diff", "export-diagram"]) expect(r.stdout).toContain(c);
  });

  it("no command prints help and exits 1", async () => {
    const r = await runCli([]);
    expect(r.code).toBe(1);
    expect(r.stdout).toContain("Usage");
  });

  it("unknown command exits 1", async () => {
    const r = await runCli(["frobnicate"]);
    expect(r.code).toBe(1);
    expect(r.stderr).toContain("unknown command");
  });
});

describe("cli — validate", () => {
  it("accepts a preset and reports its fingerprint", async () => {
    const r = await runCli(["validate", "classic"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("valid");
    expect(r.stdout).toContain("fp_");
  });

  it("rejects a broken dataset naming the offending entry", async () => {
    const r = await runCli(["validate", "./cli.fixtures.js#BROKEN"]);
    expect(r.code).toBe(1);
    expect(r.stderr).toContain("registry.duplicate-id");
    expect(r.stderr).toContain("dup");
  });
});

describe("cli — generate", () => {
  it("emits a stable world descriptor to stdout", async () => {
    const a = await runCli(["generate", "classic", "--seed", "gen-1"]);
    const b = await runCli(["generate", "classic", "--seed", "gen-1"]);
    expect(a.code).toBe(0);
    expect(a.stdout).toBe(b.stdout); // byte-stable across runs
    const world = JSON.parse(a.stdout);
    expect(world.meta.worldSeed).toBe("gen-1");
    expect(world.reaches.length).toBe(1);
  });

  it("round-trips through a reproduction bundle byte-identically", async () => {
    const dir = tmp();
    const worldA = join(dir, "worldA.json");
    const bundle = join(dir, "bundle.json");
    const worldB = join(dir, "worldB.json");

    const g1 = await runCli(["generate", "classic", "--seed", "rt", "-o", worldA, "--save-bundle", bundle]);
    expect(g1.code).toBe(0);
    const g2 = await runCli(["generate", bundle, "-o", worldB]);
    expect(g2.code).toBe(0);

    expect(readFileSync(worldB, "utf8")).toBe(readFileSync(worldA, "utf8"));
  });

  it("fails a bundle whose expected fingerprint no longer matches", async () => {
    const dir = tmp();
    const bad = join(dir, "bad-bundle.json");
    writeFileSync(
      bad,
      JSON.stringify({
        kind: "cyclevania.reproduction-bundle",
        formatVersion: 1,
        registry: { module: "@cyclevania/examples", export: "CLASSIC" },
        seed: "x",
        requestLog: [],
        expected: { registryFingerprint: "fp_deadbeef", generationVersion: "1.0.0" },
      }),
      "utf8",
    );
    const r = await runCli(["generate", bad, "--reaches", "1"]);
    expect(r.code).toBe(1);
    expect(r.stderr).toContain("reproduction check failed");
  });
});

describe("cli — soak", () => {
  it("runs 25 seeds green on classic", async () => {
    const r = await runCli(["soak", "classic", "--seeds", "25"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("25 pass");
  });

  it("passes with autosolve too", async () => {
    const r = await runCli(["soak", "crawler", "--seeds", "10", "--autosolve"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("0 fail");
  });
});

describe("cli — report", () => {
  it("includes a mermaid fence, the sphere table, and the seed", async () => {
    const r = await runCli(["report", "classic", "--seed", "report-xyz"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("```mermaid");
    expect(r.stdout).toContain("| Sphere | Regions |");
    expect(r.stdout).toContain("report-xyz");
  });
});

describe("cli — diff", () => {
  it("reports identical for the same source + seed", async () => {
    const r = await runCli(["diff", "classic", "classic", "--seedA", "1", "--seedB", "1"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("identical");
  });

  it("reports differ for two seeds", async () => {
    const r = await runCli(["diff", "classic", "classic", "--seedA", "1", "--seedB", "2"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("differ");
  });
});

describe("cli — export-diagram", () => {
  it("emits balanced, parseable mermaid", async () => {
    const r = await runCli(["export-diagram", "classic", "--seed", "d", "--view", "mission"]);
    expect(r.code).toBe(0);
    expect(r.stdout).toContain("flowchart");
    expect(bracketsBalanced(r.stdout)).toBe(true);
  });

  it("rejects an unknown view", async () => {
    const r = await runCli(["export-diagram", "classic", "--view", "nope"]);
    expect(r.code).toBe(1);
    expect(r.stderr).toContain("unsupported view");
  });
});

/**
 * **The State view's drawing.**
 *
 * ⚠ **Every bug this file guards was invisible to a passing test suite and obvious in a screenshot.**
 * The data was right each time — the *picture* was wrong. That is the whole argument for looking:
 *
 * | Defect | What the data said | What the screen showed |
 * |---|---|---|
 * | two-way arrows collapsed onto one line | both transitions present | one arrow — *"there is a way back"* invisible |
 * | `orient="auto-start-end"` | valid SVG to a linter | console errors, no arrowheads |
 * | the gate label at the wire midpoint | correct coordinates | overprinting the state's name |
 */

import { describe, expect, it } from "vitest";
import { checkLine, checkTone, drawStateGraph, type StateGraphView } from "../src/state-view.ts";

const WATER: StateGraphView = {
  variable: "water_level",
  satisfiesP15: true,
  states: [
    { name: "low", x: -170, y: 0, initial: true, outDegree: 1 },
    { name: "mid", x: 0, y: 0, initial: false, outDegree: 2 },
    { name: "high", x: 170, y: 0, initial: false, outDegree: 1 },
  ],
  transitions: [
    { from: "low", to: "mid", via: "", requires: [] },
    { from: "mid", to: "low", via: "", requires: [] },
    { from: "mid", to: "high", via: "", requires: [] },
    { from: "high", to: "mid", via: "", requires: ["IronBoots"] },
  ],
  findings: [
    {
      kind: "exit-gated",
      blocks: false,
      state: "high",
      message: "`high` is accessible but leaving it needs [IronBoots] — potential softlock",
    },
  ],
};

function lines(svg: string): string[] {
  return svg.match(/<line [^>]*>/g) ?? [];
}

describe("a two-way pair does not collapse into one line", () => {
  it("draws every transition", () => {
    expect(lines(drawStateGraph(WATER))).toHaveLength(4);
  });

  it("separates the two directions onto different rows", () => {
    // ⚠ **The bug.** The normal was taken from each edge's own direction, so reversing the edge flipped
    // it — and the flip cancelled the offset, putting both arrows on the same pixel row. Four lines
    // were emitted and two were drawn.
    const ys = lines(drawStateGraph(WATER))
      .map((l) => /y1="(-?[\d.]+)"/.exec(l)?.[1])
      .filter((y): y is string => y !== undefined);
    expect(new Set(ys).size).toBeGreaterThan(1);
  });

  it("keeps a single-direction pair on one row", () => {
    const oneWay: StateGraphView = { ...WATER, transitions: [WATER.transitions[0]!] };
    expect(lines(drawStateGraph(oneWay))).toHaveLength(1);
  });
});

describe("the drawing is valid SVG a browser accepts", () => {
  it("uses an orient value markers actually support", () => {
    // ⚠ `auto-start-end` is in the spec and Chrome rejects it — the arrowheads silently vanished and
    // the console filled with errors nobody was reading.
    const svg = drawStateGraph(WATER);
    expect(svg).toContain('orient="auto"');
    expect(svg).not.toContain("auto-start-end");
  });

  it("opens and closes", () => {
    const svg = drawStateGraph(WATER);
    expect(svg.startsWith("<svg")).toBe(true);
    expect(svg.endsWith("</svg>")).toBe(true);
  });

  it("fits every box inside the viewBox", () => {
    const svg = drawStateGraph(WATER);
    const [minX, minY, w, h] = (/viewBox="([^"]+)"/.exec(svg)?.[1] ?? "")
      .split(" ")
      .map(Number) as [number, number, number, number];
    for (const s of WATER.states) {
      expect(s.x).toBeGreaterThan(minX);
      expect(s.x).toBeLessThan(minX + w);
      expect(s.y).toBeGreaterThan(minY);
      expect(s.y).toBeLessThan(minY + h);
    }
  });
});

describe("a gate is readable", () => {
  it("labels the wire with what it costs", () => {
    expect(drawStateGraph(WATER)).toContain("IronBoots");
  });

  it("places the label clear of both boxes", () => {
    // ⚠ Authored positions can leave a 54px gap between 116-wide boxes — narrower than the word — so a
    // label near the midpoint overprints the state names. It must sit outside the boxes' band.
    const svg = drawStateGraph(WATER);
    const label = /<text x="(-?[\d.]+)" y="(-?[\d.]+)"[^>]*>IronBoots/.exec(svg);
    expect(label).not.toBeNull();
    const y = Number(label![2]);
    expect(Math.abs(y)).toBeGreaterThan(23); // half a box height
  });

  it("draws a gated transition differently from a free one", () => {
    const svg = drawStateGraph(WATER);
    expect(svg).toContain("stroke-dasharray");
    expect(svg).toContain("arrow-gated");
  });
});

describe("faults are drawn on the box they are about", () => {
  it("marks the state a finding names", () => {
    // ⚠ Telling a developer "nobody can get here" when the truth is "nobody can leave" sends them to
    // the wrong end of the graph, so the badge goes on the right box.
    //
    // ⚠ **Asserted as a token, not a hex.** These read `#b8860b` until the theme landed, and a test
    // that names a literal colour is a second palette — it goes stale the moment the first one moves,
    // and it fails on output that got *more* correct.
    expect(drawStateGraph(WATER)).toContain("var(--cv-warn)");
  });

  it("shows a blocking fault differently from a warning", () => {
    const blocking: StateGraphView = {
      ...WATER,
      satisfiesP15: false,
      findings: [{ kind: "dead-end", blocks: true, state: "high", message: "no way out" }],
    };
    // ⚠ **Assert on the box, not the whole document.** A first draft checked that amber was absent
    // entirely — but the gated *wire* is legitimately amber, so the test failed on correct output.
    const boxes = (svg: string) => svg.match(/<rect [^>]*>/g) ?? [];
    expect(boxes(drawStateGraph(blocking)).some((r) => r.includes("var(--cv-err)"))).toBe(true);
    expect(boxes(drawStateGraph(WATER)).some((r) => r.includes("var(--cv-warn)"))).toBe(true);
    expect(boxes(drawStateGraph(WATER)).some((r) => r.includes("var(--cv-err)"))).toBe(false);
  });

  it("marks the initial state on the box rather than in prose beside it", () => {
    expect(drawStateGraph(WATER)).toContain(">initial<");
  });
});

describe("the check line is a sentence, not a count", () => {
  it("says what is wrong", () => {
    expect(checkLine(WATER)).toContain("IronBoots");
    expect(checkLine(WATER)).not.toMatch(/^\d+ finding/);
  });

  it("says so plainly when there is nothing to report", () => {
    expect(checkLine({ ...WATER, findings: [] })).toContain("P15 satisfied");
  });
});

describe("the check line's colour follows the worst finding", () => {
  it("is ok only when there is nothing to say", () => {
    expect(checkTone({ ...WATER, satisfiesP15: true, findings: [] })).toBe("ok");
  });

  it("warns when a graph satisfies P15 but still carries a finding", () => {
    // ⚠ **Found by looking.** The line was coloured by `satisfiesP15` alone, so "⚠ … potential
    // softlock" printed green — and the colour is read before the words are.
    expect(checkTone(WATER)).toBe("warn");
    expect(WATER.satisfiesP15).toBe(true);
    expect(WATER.findings.length).toBeGreaterThan(0);
  });

  it("errors on a blocking finding, whatever P15 says", () => {
    const blocking = {
      ...WATER,
      satisfiesP15: true,
      findings: [{ kind: "dead-end" as const, blocks: true, state: "high", message: "no way out" }],
    };
    expect(checkTone(blocking)).toBe("err");
  });

  it("errors when P15 fails with no finding to point at", () => {
    // ⚠ A claim about the graph as a whole still has to be visible.
    expect(checkTone({ ...WATER, satisfiesP15: false, findings: [] })).toBe("err");
  });
});

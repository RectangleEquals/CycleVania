/**
 * **The Curve editor, Unlock table, Dials view and floor slider.**
 *
 * ⚠ **The assertions that matter are the ones about what the drawing must *distinguish*.** A view that
 * renders is not a view that works: a curve preview on a shared scale renders, and lies; a table that
 * prints a cycle message beside itself renders, and leaves the developer to find the rows by eye.
 */

import { describe, expect, it } from "vitest";
import {
  FloorSlider,
  curveThumbnail,
  drawCurveTable,
  drawDials,
  drawUnlockTable,
  type CurveRow,
  type CurveTableView,
  type DialRowView,
  type UnlockTableView,
} from "../src/table-views.ts";

const sample = (from: number, to: number, f: (x: number) => number, n = 24) =>
  Array.from({ length: n }, (_, i) => {
    const x = from + ((to - from) * i) / (n - 1);
    return [x, f(x)] as [number, number];
  });

/** A row, with the keys and interpolation the format carries. */
const row = (
  name: string,
  from: number,
  to: number,
  f: (x: number) => number,
  interpolation = "LINEAR",
): CurveRow => ({
  name,
  from,
  to,
  interpolation,
  keys: [
    [from, f(from)],
    [(from + to) / 2, f((from + to) / 2)],
    [to, f(to)],
  ],
  points: sample(from, to, f),
});

const CURVES: CurveTableView = {
  path: "/Content/Curves/progression.cvcurve",
  domain: "depth",
  yLabel: "multiplier",
  rows: [row("big", 0, 12, (x) => x * 10), row("small", 0, 12, (x) => x * 0.01, "CUBIC")],
};

describe("P03 — the Curve editor", () => {
  it("scales each row to its own extent", () => {
    // ⚠ **A shared scale would flatten `small` beside `big`** — and flat is what a broken curve looks
    // like, so a shared scale lies in the worst direction.
    const small = curveThumbnail(CURVES.rows[1]!);
    const points = /points="([^"]+)"/.exec(small)?.[1] ?? "";
    const ys = points.split(" ").map((p) => Number(p.split(",")[1]));
    expect(Math.max(...ys) - Math.min(...ys)).toBeGreaterThan(4);
  });

  it("draws over the row's own x range, not an assumed 0..1", () => {
    const shifted: CurveTableView = {
      ...CURVES,
      rows: [row("late", 100, 200, (x) => x)],
    };
    const points = /points="([^"]+)"/.exec(curveThumbnail(shifted.rows[0]!))?.[1] ?? "";
    const xs = points.split(" ").map((p) => Number(p.split(",")[0]));
    expect(Math.max(...xs) - Math.min(...xs)).toBeGreaterThan(10);
  });

  it("names both axes, because a shape with no units is every curve", () => {
    const svg = drawCurveTable(CURVES);
    expect(svg).toContain("depth");
    expect(svg).toContain("multiplier");
  });

  it("gives each row its own colour so a legend means something", () => {
    // ⚠ **Series colours stay literal on purpose.** They are data identity, not chrome — Unreal's
    // graphs fix them per series too — so they are the one thing here that is not a theme token.
    const svg = drawCurveTable(CURVES);
    expect(svg).toContain("#5aab55");
    expect(svg).toContain("#e0a33a");
  });

  it("draws the authored keys, not only the sampled line", () => {
    // ⚠ A polyline is a *preview*; a key is a thing a developer selects and moves.
    const svg = drawCurveTable(CURVES);
    expect((svg.match(/<rect [^>]*data-row=/g) ?? []).length).toBe(6);
  });

  it("shows tangent handles on a selected CUBIC key, and none on a LINEAR one", () => {
    // ⚠ A LINEAR row has no tangent to edit; an inert handle invites a drag that cannot move.
    const cubic = drawCurveTable(CURVES, 460, 190, { row: "small", key: 1 });
    const linear = drawCurveTable(CURVES, 460, 190, { row: "big", key: 1 });
    expect(cubic).toContain("#e8e8e8");
    expect(linear).not.toContain("#e8e8e8");
  });

  it("names each row's interpolation, because the format carries it per row", () => {
    expect(drawCurveTable(CURVES)).toContain("CUBIC");
  });
});

describe("P03a — the Unlock table", () => {
  const CYCLE: UnlockTableView = {
    rows: [
      { id: "u_a", name: "A", doc: "", supersedes: ["u_b"] },
      { id: "u_b", name: "B", doc: "", supersedes: ["u_a"] },
      { id: "u_c", name: "C", doc: "fine", supersedes: [] },
    ],
    fault: {
      kind: "supersedes-cycle",
      rows: ["u_a", "u_b", "u_a"],
      message: "supersedes cycle: u_a -> u_b -> u_a",
    },
  };

  it("shows the cycle on the rows that form it, not only in a message", () => {
    // ⚠ **The point of the view.** An error beside a table leaves the developer to find the rows by
    // eye, which is the work this replaces.
    const html = drawUnlockTable(CYCLE);
    const rows = html.split("<tr").slice(1);
    // ⚠ **Count the marked rows, not mentions of the token.** `--cv-err` also appears in the
    // supersedes *text* of a faulted row, so counting the token counts some rows twice and the
    // assertion drifts from what it claims to check.
    const marked = rows.filter((r) => r.includes("is-faulted"));
    expect(marked).toHaveLength(2);
    expect(marked.every((r) => r.includes("u_a") || r.includes("u_b"))).toBe(true);
  });

  it("does not mark the innocent row", () => {
    expect(drawUnlockTable(CYCLE).split("<tr").find((r) => r.includes("u_c"))).not.toContain("is-faulted");
  });

  it("dedupes a cycle path so a row is not marked twice", () => {
    // ⚠ The fault carries `a → b → a`; a naive pass marks `u_a` for arriving and again for closing.
    const html = drawUnlockTable(CYCLE);
    expect(html.split("is-faulted").length - 1).toBe(2);
  });

  it("marks id read-only as a column, not per row", () => {
    const html = drawUnlockTable(CYCLE);
    expect(html).toContain("read-only");
    // once, in the header — not on every row
    expect(html.split("read-only").length - 1).toBe(1);
  });

  it("still shows the message, because the rows alone do not say what is wrong", () => {
    expect(drawUnlockTable(CYCLE)).toContain("supersedes cycle");
  });
});

describe("P04 — the Dials view", () => {
  const DIALS: DialRowView[] = [
    {
      id: "Hookshot.rope_length",
      owner: "Hookshot",
      kind: "NUMBER",
      doc: "how far it reaches",
      default: "30",
      effective: "42",
      source: "HOST",
      overridden: true,
      outOfBounds: false,
    },
    {
      id: "Hookshot.charge",
      owner: "Hookshot",
      kind: "RANGE",
      doc: "",
      default: "1",
      effective: "9",
      source: "AUTHORED",
      overridden: false,
      outOfBounds: true,
    },
  ];

  it("shows default and effective both", () => {
    // ⚠ Neither is derivable from the other: only-effective makes *reset* impossible, only-default
    // makes the panel a lie about what the next generate uses.
    const html = drawDials(DIALS);
    expect(html).toContain("30");
    expect(html).toContain("42");
  });

  it("marks an out-of-bounds value as a warning rather than hiding it", () => {
    // ⚠ Content may have authored a default outside a range it later narrowed — hiding the dial would
    // hide the mistake.
    expect(drawDials(DIALS)).toContain("--cv-warn");
  });

  it("says where a changed value came from", () => {
    expect(drawDials(DIALS)).toContain("HOST");
    expect(drawDials(DIALS)).toContain("AUTHORED");
  });

  it("says plainly when a project declares none", () => {
    expect(drawDials([])).toContain("no dials");
  });
});

describe("P05 — the floor slider is shared state", () => {
  it("tells every watcher, not just the one that moved it", () => {
    // ⚠ Shared from the start, because M20 links it to the skeleton — lifting it later means finding
    // every place that read a private copy.
    const floor = new FloorSlider();
    const seen: number[] = [];
    floor.onChange((f) => seen.push(f));
    floor.onChange((f) => seen.push(f * 100));
    floor.set(3);
    expect(seen).toEqual([3, 300]);
    expect(floor.watchers).toBe(2);
  });

  it("does not fire when the value did not change", () => {
    const floor = new FloorSlider();
    let fired = 0;
    floor.onChange(() => (fired += 1));
    floor.set(2);
    floor.set(2);
    expect(fired).toBe(1);
  });

  it("reports the current floor to a view that arrives late", () => {
    const floor = new FloorSlider();
    floor.set(4);
    expect(floor.floor).toBe(4);
  });
});

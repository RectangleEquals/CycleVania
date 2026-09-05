/**
 * **The curve editor.**
 *
 * ⚠ **The assertions are about what M20a's thumbnail strip could not do**: compare channels on one
 * scale you control, land a key on an exact number, and see the interpolation mode as a *control*
 * rather than a caption.
 */

import { describe, expect, it } from "vitest";
import {
  MODES,
  channelColour,
  curveStyles,
  drawCanvas,
  drawChannels,
  drawCurveToolbar,
  drawGrid,
  extentOf,
  project,
  tangentsAt,
  ticks,
  type Channel,
  type CurveAsset,
  type CurveState,
} from "../src/curve-editor.ts";

const sample = (from: number, to: number, f: (x: number) => number, n = 20): [number, number][] =>
  Array.from({ length: n }, (_, i) => {
    const x = from + ((to - from) * i) / (n - 1);
    return [x, f(x)] as [number, number];
  });

const ch = (name: string, f: (x: number) => number, interpolation = "LINEAR", vis = true): Channel => ({
  name,
  interpolation,
  keys: [[0, f(0)], [6, f(6)], [12, f(12)]],
  points: sample(0, 12, f),
  visible: vis,
});

const ASSET: CurveAsset = {
  path: "/Content/Curves/progression.cvcurve",
  domain: "depth",
  yLabel: "multiplier",
  channels: [
    ch("complexity", (x) => x * 0.5 + 1, "CUBIC"),
    ch("hazard", (x) => x * 0.06, "LINEAR"),
    ch("tier", (x) => Math.floor(x / 4) + 1, "STEP"),
  ],
};

const st = (over: Partial<CurveState> = {}): CurveState => ({
  filter: "",
  selected: null,
  view: "curve",
  ...over,
});

describe("one canvas, and visibility is what makes a shared scale honest", () => {
  it("scales to the visible channels only", () => {
    // ⚠ **The answer to the shared-scale problem.** A developer chooses what to compare, so nothing
    // has to rescale per row and lie about the rest — which is what M20a's strip did.
    const all = extentOf(ASSET.channels);
    const one = extentOf(ASSET.channels.map((c) => ({ ...c, visible: c.name === "hazard" })));
    expect(one.y1).toBeLessThan(all.y1);
  });

  it("survives a channel that never changes value", () => {
    // ⚠ A zero-height extent divides by zero and draws a line through the middle of nothing.
    const flat = extentOf([ch("flat", () => 3)]);
    expect(flat.y1).toBeGreaterThan(flat.y0);
    expect(Number.isFinite(project(flat, 400, 200, 0, 3)[1])).toBe(true);
  });

  it("has something to draw when every channel is hidden", () => {
    const e = extentOf(ASSET.channels.map((c) => ({ ...c, visible: false })));
    expect(Number.isFinite(e.x1 - e.x0)).toBe(true);
  });

  it("puts the domain ruler along the top, not the bottom", () => {
    // ⚠ **An earlier draft put it at the bottom, which is neither engine.** The tick labels sit above
    // the plot; the value labels sit left of it.
    const svg = drawCanvas(ASSET, st(), 600, 300);
    const domain = [...svg.matchAll(/<text x="([\d.]+)" y="([\d.]+)" text-anchor="middle"/g)];
    expect(domain.length).toBeGreaterThan(0);
    for (const m of domain) expect(Number(m[2])).toBeLessThan(30);
  });

  it("emphasises the zero line", () => {
    // ▶ The one value a reader looks for first.
    expect(drawCanvas(ASSET, st(), 600, 300)).toMatch(/stroke="var\(--cv-muted\)" stroke-width="1"/);
  });

  it("labels each curve where it ends, not in a legend", () => {
    // ▶ **A legend steals canvas**; the label at the curve's end costs nothing.
    const svg = drawCanvas(ASSET, st(), 600, 300);
    for (const c of ASSET.channels) expect(svg).toContain(`>${c.name}</text>`);
  });

  it("draws only what is visible", () => {
    const hidden = { ...ASSET, channels: ASSET.channels.map((c) => ({ ...c, visible: c.name === "tier" })) };
    const svg = drawCanvas(hidden, st(), 600, 300);
    expect((svg.match(/<polyline/g) ?? []).length).toBe(1);
  });
});

describe("ticks are round numbers", () => {
  it("never offers a ruler of 0.3333", () => {
    // ⚠ **A ruler a reader cannot hold in their head teaches nothing.**
    for (const t of ticks(0, 1)) expect(String(t).replace("-", "").length).toBeLessThanOrEqual(4);
    expect(ticks(0, 12)).toContain(0);
  });

  it("does not loop forever on a degenerate range", () => {
    expect(ticks(5, 5)).toEqual([5]);
  });
});

describe("keys and tangents", () => {
  it("draws a key per authored key, not per sample", () => {
    // ⚠ A polyline is a *preview*; a key is a thing a developer selects and moves.
    const svg = drawCanvas(ASSET, st(), 600, 300);
    expect((svg.match(/class="cv-key"/g) ?? []).length).toBe(9);
  });

  it("fills the selected key in the accent, and only that one", () => {
    const svg = drawCanvas(ASSET, st({ selected: { channel: "complexity", key: 1 } }), 600, 300);
    expect((svg.match(/fill="var\(--cv-accent\)"/g) ?? []).length).toBe(1);
  });

  it("draws tangent handles as one line through the key", () => {
    // ▶ **The reference draws one line with a circle at each end**, not two separate arms.
    const t = tangentsAt(ASSET.channels[0]!, 1, extentOf(ASSET.channels), 600, 300);
    expect(t).toBeDefined();
    expect(t!.a[0]).toBeLessThan(t!.at[0]);
    expect(t!.b[0]).toBeGreaterThan(t!.at[0]);
  });

  it("offers no tangent on a LINEAR or STEP key", () => {
    // ⚠ **An inert handle invites a drag that cannot move.**
    expect(tangentsAt(ASSET.channels[1]!, 1, extentOf(ASSET.channels), 600, 300)).toBeUndefined();
    expect(tangentsAt(ASSET.channels[2]!, 1, extentOf(ASSET.channels), 600, 300)).toBeUndefined();
  });
});

describe("the toolbar is the control, not a caption", () => {
  it("lights the mode the selection already has", () => {
    // ⚠ **Stateful** — the group is the readout *and* the control. M20a printed the mode as text.
    const html = drawCurveToolbar(ASSET, st({ selected: { channel: "complexity", key: 0 } }));
    expect(html).toMatch(/class="cv-titem is-on" data-mode="CUBIC"/);
    expect(html).not.toMatch(/is-on" data-mode="LINEAR"/);
  });

  it("offers the three modes the format carries, and no fourth", () => {
    expect([...MODES]).toEqual(["LINEAR", "STEP", "CUBIC"]);
  });

  it("gives the selected key numeric time and value fields", () => {
    // ⚠ **A curve editor with no numeric entry is a drawing tool** — some keys have to land on an
    // exact number, and dragging cannot do that.
    const html = drawCurveToolbar(ASSET, st({ selected: { channel: "hazard", key: 2 } }));
    expect(html).toContain("data-keytime");
    expect(html).toMatch(/data-keytime value="12"/);
  });

  it("disables the key fields when nothing is selected, rather than showing a lie", () => {
    expect(drawCurveToolbar(ASSET, st())).toMatch(/data-keytime value=""\s*disabled/);
  });

  it("offers the grid view beside the curve view", () => {
    // ▶ **A `.cvcurve` is literally a table of curves.**
    const html = drawCurveToolbar(ASSET, st({ view: "grid" }));
    expect(html).toMatch(/class="cv-titem is-on" data-view="grid"/);
  });
});

describe("the channel list", () => {
  it("is how a curve table is authored", () => {
    // ⚠ **A new table has no rows at all** — create-from-scratch is the default path.
    expect(drawChannels(ASSET, st())).toContain("data-addcurve");
    expect(drawChannels({ ...ASSET, channels: [] }, st())).toContain("adds one");
  });

  it("carries the channel's colour on the row itself", () => {
    expect(drawChannels(ASSET, st())).toContain(channelColour(0));
  });

  it("filters", () => {
    const html = drawChannels(ASSET, st({ filter: "haz" }));
    expect(html).toContain("hazard");
    expect(html).not.toContain(">complexity<");
  });

  it("shows each channel's mode without opening anything", () => {
    expect(drawChannels(ASSET, st())).toContain("STEP");
  });

  it("offers a visibility toggle per channel", () => {
    expect(drawChannels(ASSET, st())).toContain('data-vis="tier"');
  });
});

describe("the grid view", () => {
  it("is the same rows, typed instead of dragged", () => {
    const html = drawGrid(ASSET, st());
    expect(html).toContain("depth");
    for (const c of ASSET.channels) expect(html).toContain(c.name);
  });

  it("marks a key a channel does not have, rather than inventing a value", () => {
    const sparse: CurveAsset = {
      ...ASSET,
      channels: [ch("a", (x) => x), { ...ch("b", (x) => x), keys: [[0, 0]] }],
    };
    expect(drawGrid(sparse, st())).toContain("—");
  });
});

describe("the editor keeps the theme's promise", () => {
  it("takes every colour from the four parameters, except the series", () => {
    // ⚠ **Series colours are data identity, not chrome** — Unreal fixes them per series too, and they
    // are the one thing here that is deliberately literal.
    const bare = curveStyles().replace(/var\(--cv-[a-z]+\)/g, "");
    expect(bare).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});

/**
 * **The curve editor** — a channel list and one canvas.
 *
 * ⚠ **A first draft of this section of the design described Unreal's *Sequencer*, which is the cinematic
 * editor and not a curve editor at all.** ▶ **The reference is the `Curve` / `Curve Table` asset
 * editor** — a standalone editor you get by opening a curve asset, which is exactly this case.
 *
 * ⚠ **And M20a's curve view was the right drawing in the wrong place.** It rendered every row of a table
 * onto one small canvas with a legend — ▶ **which is a thumbnail**, and a thumbnail belongs wherever a
 * curve is *referenced*, not where it is edited. `table-views.ts` keeps that drawing; this is the editor.
 *
 * # What the reference actually does — `10-editor.md` §9b
 *
 * - **the domain ruler runs along the *top***, the value ruler down the left, the zero line emphasised
 * - **channel labels sit at the right edge, where each curve ends** — so overlaid curves are told apart
 *   without a legend stealing canvas
 * - **keys are circles**: hollow idle, accent-filled selected, with **tangent handles drawn as one line
 *   through the key**, a hollow circle at each end
 * - ⚠ **the selected key's time and value are numeric fields in the toolbar** — *"Multiple Values"* on a
 *   mixed selection. **A curve editor with no numeric entry is a drawing tool**
 * - ⚠ **`LINEAR` · `STEP` · `CUBIC` are stateful toolbar buttons**, applied to the selection and lit for
 *   the selection's current mode
 */

import { icon } from "./icons.ts";

/** One curve in the table. */
export interface Channel {
  name: string;
  /** `LINEAR` · `STEP` · `CUBIC` — ⚠ **the file's spelling**, never the core's enum name. */
  interpolation: string;
  keys: [number, number][];
  /** Sampled by the core, for drawing between keys. */
  points: [number, number][];
  visible: boolean;
}

export interface CurveAsset {
  path: string;
  /** ⚠ Asset properties, and they belong in the Details panel — not invented onto the canvas. */
  domain: string;
  yLabel: string;
  channels: Channel[];
}

export interface Selection {
  channel: string;
  key: number;
}

export interface CurveState {
  filter: string;
  selected: Selection | null;
  /** ▶ **A `.cvcurve` is literally a table of curves** — a developer typing exact numbers wants the grid. */
  view: "curve" | "grid";
}

/** ⚠ Series colours are data identity, not chrome — Unreal fixes them per series too. */
export const SERIES = ["#5aab55", "#e0a33a", "#3d8bfd", "#b34fa0", "#48b0a8", "#c2603f"];
export const channelColour = (i: number): string => SERIES[i % SERIES.length]!;

/** ⚠ The three the format carries. A fourth would be a mode nobody declared. */
export const MODES = ["LINEAR", "STEP", "CUBIC"] as const;

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

// ---------------------------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------------------------

export interface Extent {
  x0: number;
  x1: number;
  y0: number;
  y1: number;
}

/**
 * The extent across every *visible* channel.
 *
 * ⚠ **Visibility is the answer to the shared-scale problem.** A developer chooses what to compare, so
 * nothing has to rescale per row and lie about the rest — ▶ which is what M20a's thumbnail strip did,
 * and why a flat line there meant nothing.
 */
export function extentOf(channels: Channel[]): Extent {
  const vis = channels.filter((c) => c.visible && c.points.length);
  if (!vis.length) return { x0: 0, x1: 1, y0: 0, y1: 1 };
  const xs = vis.flatMap((c) => c.points.map((p) => p[0]));
  const ys = vis.flatMap((c) => c.points.map((p) => p[1]));
  const y0 = Math.min(...ys);
  const y1 = Math.max(...ys);
  // ⚠ A zero-height extent divides by zero and draws a line through the middle of nothing.
  const pad = (y1 - y0) * 0.12 || 1;
  return { x0: Math.min(...xs), x1: Math.max(...xs), y0: y0 - pad, y1: y1 + pad };
}

/** "Nice" tick values across a range — ⚠ round numbers, because a ruler of 0.3333 teaches nothing. */
export function ticks(lo: number, hi: number, want = 5): number[] {
  const span = hi - lo;
  if (!(span > 0)) return [lo];
  const raw = span / want;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? mag * 10;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) {
    out.push(Math.abs(v) < step / 1e6 ? 0 : Number(v.toFixed(6)));
  }
  return out;
}

const PAD = { left: 46, top: 22, right: 76, bottom: 14 };

/** Project a point into canvas coordinates. */
export function project(e: Extent, w: number, h: number, x: number, y: number): [number, number] {
  const iw = w - PAD.left - PAD.right;
  const ih = h - PAD.top - PAD.bottom;
  return [
    PAD.left + ((x - e.x0) / (e.x1 - e.x0 || 1)) * iw,
    PAD.top + ih - ((y - e.y0) / (e.y1 - e.y0 || 1)) * ih,
  ];
}

/**
 * Tangent handles for a key.
 *
 * ▶ **One line drawn *through* the key**, with a hollow circle at each end — which is what the reference
 * draws, rather than two separate arms. ⚠ **`CUBIC` only**: a `LINEAR` or `STEP` key has no tangent, and
 * an inert handle invites a drag that cannot move.
 */
export function tangentsAt(
  c: Channel,
  i: number,
  e: Extent,
  w: number,
  h: number,
): { at: [number, number]; a: [number, number]; b: [number, number] } | undefined {
  if (c.interpolation.toUpperCase() !== "CUBIC") return undefined;
  const key = c.keys[i];
  if (!key) return undefined;
  const prev = c.keys[i - 1] ?? key;
  const next = c.keys[i + 1] ?? key;
  const at = project(e, w, h, key[0], key[1]);
  const p = project(e, w, h, prev[0], prev[1]);
  const n = project(e, w, h, next[0], next[1]);
  const third = (q: [number, number]): [number, number] => [
    at[0] + (q[0] - at[0]) / 3,
    at[1] + (q[1] - at[1]) / 3,
  ];
  return { at, a: third(p), b: third(n) };
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/** The canvas. */
export function drawCanvas(asset: CurveAsset, st: CurveState, w = 780, h = 430): string {
  const e = extentOf(asset.channels);
  const xs = ticks(e.x0, e.x1);
  const ys = ticks(e.y0, e.y1);
  const P = (x: number, y: number) => project(e, w, h, x, y);

  // ⚠ **The domain ruler runs along the top** — an earlier draft put it at the bottom, which is neither
  // engine. The value ruler runs down the left.
  const grid =
    xs
      .map((t) => {
        const [x] = P(t, 0);
        return (
          `<line x1="${x}" y1="${PAD.top}" x2="${x}" y2="${h - PAD.bottom}" ` +
          `stroke="var(--cv-line)" stroke-width=".7" opacity=".6"/>` +
          `<text x="${x}" y="${PAD.top - 7}" text-anchor="middle" font-size="9.5" ` +
          `fill="var(--cv-muted)">${t}</text>`
        );
      })
      .join("") +
    ys
      .map((t) => {
        const [, y] = P(0, t);
        // ▶ **The zero line is emphasised** — it is the one value a reader looks for first.
        const zero = Math.abs(t) < 1e-9;
        return (
          `<line x1="${PAD.left}" y1="${y}" x2="${w - PAD.right}" y2="${y}" ` +
          `stroke="var(--cv-${zero ? "muted" : "line"})" stroke-width="${zero ? 1 : 0.7}" ` +
          `opacity="${zero ? 0.8 : 0.6}"/>` +
          `<text x="${PAD.left - 7}" y="${y + 3.4}" text-anchor="end" font-size="9.5" ` +
          `fill="var(--cv-muted)">${t}</text>`
        );
      })
      .join("");

  const lines = asset.channels
    .map((c, i) => {
      if (!c.visible) return "";
      const pts = c.points.map(([x, y]) => P(x, y).map((v) => v.toFixed(1)).join(",")).join(" ");
      return `<polyline fill="none" stroke="${channelColour(i)}" stroke-width="1.8" points="${pts}"/>`;
    })
    .join("");

  // ▶ **Channel labels at the right edge, where each curve ends.**
  const labels = asset.channels
    .map((c, i) => {
      if (!c.visible || !c.points.length) return "";
      const last = c.points[c.points.length - 1]!;
      const [x, y] = P(last[0], last[1]);
      return (
        `<text x="${x + 6}" y="${y + 3.4}" font-size="10" fill="${channelColour(i)}">` +
        `${esc(c.name)}</text>`
      );
    })
    .join("");

  // ▶ **Keys are circles**: hollow idle, accent-filled selected.
  const keys = asset.channels
    .map((c, i) =>
      !c.visible
        ? ""
        : c.keys
            .map((k, j) => {
              const [x, y] = P(k[0], k[1]);
              const on = st.selected?.channel === c.name && st.selected.key === j;
              return (
                `<circle class="cv-key" data-ch="${esc(c.name)}" data-key="${j}" ` +
                `cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="${on ? 4.5 : 3.6}" ` +
                `fill="${on ? "var(--cv-accent)" : "var(--cv-bg)"}" ` +
                `stroke="${channelColour(i)}" stroke-width="1.5"/>`
              );
            })
            .join(""),
    )
    .join("");

  let handles = "";
  if (st.selected) {
    const c = asset.channels.find((x) => x.name === st.selected!.channel);
    const t = c && tangentsAt(c, st.selected.key, e, w, h);
    if (t) {
      handles =
        `<line x1="${t.a[0].toFixed(1)}" y1="${t.a[1].toFixed(1)}" ` +
        `x2="${t.b[0].toFixed(1)}" y2="${t.b[1].toFixed(1)}" ` +
        `stroke="var(--cv-text)" stroke-width="1.1" opacity=".75"/>` +
        [t.a, t.b]
          .map(
            (q) =>
              `<circle cx="${q[0].toFixed(1)}" cy="${q[1].toFixed(1)}" r="3" fill="var(--cv-bg)" ` +
              `stroke="var(--cv-text)" stroke-width="1.2"/>`,
          )
          .join("");
    }
  }

  return (
    `<svg class="cv-curve" width="100%" height="${h}" viewBox="0 0 ${w} ${h}" ` +
    `preserveAspectRatio="none" font-family="ui-sans-serif, system-ui, sans-serif">` +
    `<rect width="${w}" height="${h}" fill="var(--cv-bg)"/>` +
    grid +
    lines +
    keys +
    handles +
    labels +
    `</svg>`
  );
}

/** The channel list. ⚠ `⊕ Curve` is how a table is authored — a new one has no rows at all. */
export function drawChannels(asset: CurveAsset, st: CurveState): string {
  const rows = asset.channels
    .map((c, i) => ({ c, i }))
    .filter(({ c }) => !st.filter || c.name.toLowerCase().includes(st.filter.toLowerCase()))
    .map(
      ({ c, i }) =>
        `<div class="cv-chrow${st.selected?.channel === c.name ? " is-selected" : ""}" ` +
        `data-channel="${esc(c.name)}">` +
        `<button class="cv-vis${c.visible ? " is-on" : ""}" data-vis="${esc(c.name)}" ` +
        `title="${c.visible ? "Hide" : "Show"} ${esc(c.name)}">${c.visible ? "◉" : "○"}</button>` +
        `<span class="cv-chdot" style="background:${channelColour(i)}"></span>` +
        `<span class="cv-chname">${esc(c.name)}</span>` +
        `<span class="cv-chmode">${esc(c.interpolation)}</span></div>`,
    )
    .join("");
  return (
    `<div class="cv-channels">` +
    `<div class="cv-chhead">` +
    `<button class="cv-bbtn is-primary" data-addcurve="1">${icon("add", 12)}Curve</button>` +
    `<input class="cv-search cv-chfilter" placeholder="Filter" value="${esc(st.filter)}"/>` +
    `</div>` +
    (rows || `<div class="cv-empty">No curve yet. <b>⊕ Curve</b> adds one.</div>`) +
    `</div>`
  );
}

/**
 * The toolbar.
 *
 * ⚠ **The mode buttons are stateful** — the one matching the selection stays lit, so the group is the
 * readout *and* the control. ▶ M20a printed the mode as a text label beside the row: a readout of a
 * control that should have been there.
 */
export function drawCurveToolbar(asset: CurveAsset, st: CurveState): string {
  const c = st.selected && asset.channels.find((x) => x.name === st.selected!.channel);
  const key = c && st.selected ? c.keys[st.selected.key] : undefined;
  const mode = c?.interpolation.toUpperCase() ?? "";
  const dis = key ? "" : " disabled";
  return (
    `<div class="cv-toolbar cv-ctool">` +
    `<div class="cv-tgroup">` +
    MODES.map(
      (m) =>
        `<button class="cv-titem${mode === m ? " is-on" : ""}" data-mode="${m}"${c ? "" : " disabled"} ` +
        `title="Set the selected curve to ${m}">${m}</button>`,
    ).join("") +
    `</div>` +
    // ⚠ **Numeric entry, because some keys have to land on an exact number and dragging cannot do that.**
    `<div class="cv-tgroup">` +
    `<label class="cv-kfield">Time<input data-keytime value="${key ? key[0] : ""}"${dis}/></label>` +
    `<label class="cv-kfield">Value<input data-keyval value="${key ? key[1] : ""}"${dis}/></label>` +
    `</div>` +
    `<div class="cv-tgroup">` +
    `<button class="cv-titem" data-fit="1" title="Frame every visible curve (F)">Fit</button>` +
    `</div>` +
    `<div class="cv-tspacer"></div>` +
    // ▶ **The graph, or the rows as an editable grid** — the same rows, typed instead of dragged.
    `<div class="cv-tgroup">` +
    `<button class="cv-titem${st.view === "curve" ? " is-on" : ""}" data-view="curve" ` +
    `title="Curve view">${icon("curve", 13)}</button>` +
    `<button class="cv-titem${st.view === "grid" ? " is-on" : ""}" data-view="grid" ` +
    `title="Grid view — type exact numbers">${icon("content", 13)}</button>` +
    `</div></div>`
  );
}

/** The grid view. ⚠ **An in-editor grid, not a file** — the same rows, typed instead of dragged. */
export function drawGrid(asset: CurveAsset, st: CurveState): string {
  const cols = [...new Set(asset.channels.flatMap((c) => c.keys.map((k) => k[0])))].sort(
    (a, b) => a - b,
  );
  return (
    `<div class="cv-gridwrap"><table class="cv">` +
    `<thead><tr><th>${esc(asset.domain)}</th>` +
    cols.map((t) => `<th>${t}</th>`).join("") +
    `</tr></thead><tbody>` +
    asset.channels
      .map((c, i) => {
        const at = new Map(c.keys.map(([x, y]) => [x, y]));
        return (
          `<tr><td><span class="cv-chdot" style="background:${channelColour(i)}"></span>` +
          `${esc(c.name)}</td>` +
          cols
            .map((t) => {
              const v = at.get(t);
              const sel = st.selected?.channel === c.name && c.keys[st.selected.key]?.[0] === t;
              return (
                `<td class="cv-gcell${sel ? " is-selected" : ""}">` +
                (v === undefined ? `<span class="cv-dim">—</span>` : String(v)) +
                `</td>`
              );
            })
            .join("") +
          `</tr>`
        );
      })
      .join("") +
    `</tbody></table></div>`
  );
}

export function curveStyles(): string {
  const V = (n: string) => `var(--cv-${n})`;
  return `
.cv-curvewrap { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.cv-curvebody { display: flex; flex: 1 1 auto; min-height: 0; }
.cv-channels { width: 196px; flex: 0 0 auto; border-right: 1px solid ${V("line")}; overflow: auto; }
.cv-chhead { display: flex; gap: 5px; padding: 6px; border-bottom: 1px solid ${V("line")}; }
.cv-chfilter { flex: 1 1 auto; min-width: 0; }
.cv-chrow { display: flex; align-items: center; gap: 6px; padding: 4px 8px; cursor: pointer;
  font-size: 12px; }
.cv-chrow:hover { background: ${V("raised")}; }
.cv-chrow.is-selected { background: ${V("selected")}; box-shadow: inset 2px 0 0 ${V("accent")}; }
.cv-vis { background: none; border: 0; color: ${V("muted")}; cursor: pointer; font: inherit;
  padding: 0; line-height: 1; }
.cv-vis.is-on { color: ${V("accent")}; }
.cv-chdot { width: 9px; height: 9px; border-radius: 2px; flex: 0 0 auto; display: inline-block;
  vertical-align: middle; margin-right: 5px; }
.cv-chname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cv-chmode { margin-left: auto; color: ${V("muted")}; font-size: 9.5px; letter-spacing: .05em; }
.cv-canvas { flex: 1 1 auto; min-width: 0; overflow: auto; }
.cv-curve { display: block; }
.cv-key { cursor: pointer; }
.cv-ctool { border-bottom: 1px solid ${V("line")}; border-top: 0; }
.cv-kfield { display: inline-flex; align-items: center; gap: 5px; color: ${V("muted")};
  font-size: 11px; padding: 0 4px; }
.cv-kfield input { width: 62px; background: ${V("bg")}; color: ${V("text")};
  border: 1px solid ${V("line")}; border-radius: ${V("radius")}; padding: 2px 5px;
  font: inherit; font-size: 11px; font-family: ui-monospace, monospace; }
.cv-kfield input:disabled { opacity: .45; }
.cv-gridwrap { flex: 1 1 auto; overflow: auto; padding: 10px; }
.cv-gcell.is-selected { background: ${V("selected")}; box-shadow: inset 0 0 0 1px ${V("accent")}; }
`;
}

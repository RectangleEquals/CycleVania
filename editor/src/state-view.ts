/**
 * **The State graph view** — the first thing in this editor with pixels.
 *
 * ⚠ **It draws and computes nothing.** Every fact on screen — which states exist, where they sit, what
 * gates a transition, which of the four faults applies — arrives from `cv_core::state` through one
 * binding call. A view that recomputed any of it would be a second opinion about solvability.
 *
 * ▶ **No layout algorithm, because there is no layout decision.** Each `Begin State` carries `Pos=(x,y)`
 * from the document, so the author placed the boxes and this honours them. That is what lets this view
 * be drawn while *panel arrangement* is still waiting on mockups: node placement is authored data.
 *
 * ⚠ **Plain SVG, no framework.** M16 P01 deferred the component model deliberately — choosing one before
 * the layout is choosing in the dark — and nodes, arrows and labels do not need one.
 */

/** One state, as the check reports it. */
export interface StateBox {
  name: string;
  x: number;
  y: number;
  initial: boolean;
  outDegree: number;
}

/** One transition. */
export interface Edge {
  from: string;
  to: string;
  via: string;
  requires: string[];
}

/** One of the four faults. */
export interface Fault {
  kind: "inaccessible" | "dead-end" | "exit-gated" | "initial-unclear" | "unknown-state";
  blocks: boolean;
  state: string;
  message: string;
}

/** Everything the view draws. */
export interface StateGraphView {
  variable: string;
  satisfiesP15: boolean;
  states: StateBox[];
  transitions: Edge[];
  findings: Fault[];
}

const BOX_W = 116;
const BOX_H = 46;
const PAD = 56;

/** ⚠ **Blocking faults and warnings look different**, because one is a mistake and one is a decision. */
const FAULT_COLOUR: Record<Fault["kind"], string> = {
  inaccessible: "var(--cv-err)",
  "dead-end": "var(--cv-err)",
  "unknown-state": "var(--cv-err)",
  "initial-unclear": "var(--cv-err)",
  "exit-gated": "var(--cv-warn)",
};

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Where a box's centre is. */
function centre(s: StateBox): { cx: number; cy: number } {
  return { cx: s.x, cy: s.y };
}

/**
 * Draw the graph.
 *
 * ⚠ **Returns SVG text rather than touching the DOM**, so the drawing is testable without a browser —
 * the same reason every other rule in this editor is a function rather than an event handler.
 */
export function drawStateGraph(view: StateGraphView): string {
  const byName = new Map(view.states.map((s) => [s.name, s]));
  const worst = new Map<string, Fault>();
  for (const f of view.findings) {
    const held = worst.get(f.state);
    // A blocking fault outranks a warning on the same box.
    if (!held || (f.blocks && !held.blocks)) worst.set(f.state, f);
  }

  const xs = view.states.map((s) => s.x);
  const ys = view.states.map((s) => s.y);
  const minX = Math.min(...xs, 0) - BOX_W / 2 - PAD;
  const maxX = Math.max(...xs, 0) + BOX_W / 2 + PAD;
  const minY = Math.min(...ys, 0) - BOX_H / 2 - PAD;
  const maxY = Math.max(...ys, 0) + BOX_H / 2 + PAD;
  const w = maxX - minX;
  const h = maxY - minY;

  const parts: string[] = [];
  parts.push(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${minX} ${minY} ${w} ${h}" ` +
      `width="${w}" height="${h}" font-family="ui-sans-serif, system-ui, sans-serif">`,
  );
  parts.push(
    `<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" ` +
      `markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="var(--cv-muted)"/></marker>` +
      `<marker id="arrow-gated" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" ` +
      `markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="var(--cv-warn)"/></marker></defs>`,
  );

  // --- transitions first, so boxes sit on top -------------------------------------------
  const drawn = new Set<string>();
  for (const t of view.transitions) {
    const a = byName.get(t.from);
    const b = byName.get(t.to);
    if (!a || !b) continue;
    const { cx: x1, cy: y1 } = centre(a);
    const { cx: x2, cy: y2 } = centre(b);
    const gated = t.requires.length > 0;

    // ⚠ **Two states with edges both ways must not collapse into one line**, or *"there is a way
    // back"* becomes invisible — which is the one fact this view exists to show.
    //
    // ⚠ **The normal is taken from a canonical direction, not the edge's own.** Deriving it from the
    // edge reverses it for the return trip, and the sign flip then cancels the offset: both arrows land
    // on the same pixel row. It drew as a single line and every test still passed, because the tests
    // read the data and the data was right.
    const pairKey = [t.from, t.to].sort().join(" ");
    const offset = drawn.has(pairKey) ? -10 : 10;
    drawn.add(pairKey);

    const forward = t.from < t.to ? 1 : -1;
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.hypot(dx, dy) || 1;
    const nx = (-dy / len) * offset * forward;
    const ny = (dx / len) * offset * forward;
    const shrink = BOX_W / 2 + 6;
    const sx = x1 + (dx / len) * shrink + nx;
    const sy = y1 + (dy / len) * (BOX_H / 2 + 6) + ny;
    const ex = x2 - (dx / len) * shrink + nx;
    const ey = y2 - (dy / len) * (BOX_H / 2 + 6) + ny;

    parts.push(
      `<line x1="${sx.toFixed(1)}" y1="${sy.toFixed(1)}" x2="${ex.toFixed(1)}" y2="${ey.toFixed(1)}" ` +
        `stroke="${gated ? "var(--cv-warn)" : "var(--cv-muted)"}" stroke-width="1.6" ` +
        `${gated ? 'stroke-dasharray="6 4" ' : ""}marker-end="url(#${gated ? "arrow-gated" : "arrow"})"/>`,
    );
    if (gated) {
      // ⚠ The requirement is drawn **on the wire**, not in a legend. The whole point is that the
      // one-way arrow with a cost is visible at a glance.
      // ⚠ **Pushed clear of the boxes, not merely off the wire.** Authored positions can put two
      // states 170 apart with 116-wide boxes — a 54px gap, narrower than the word "IronBoots" — so a
      // label near the midpoint overprints both. Offsetting by half a box plus a margin puts it
      // outside them whatever the spacing.
      const mag = Math.hypot(nx, ny) || 1;
      const clear = BOX_H / 2 + 14;
      const lx = (x1 + x2) / 2 + (nx / mag) * clear;
      const ly = (y1 + y2) / 2 + (ny / mag) * clear + 4;
      parts.push(
        `<text x="${lx.toFixed(1)}" y="${ly.toFixed(1)}" text-anchor="middle" font-size="11" ` +
          `fill="var(--cv-warn)">${esc(t.requires.join(", "))}</text>`,
      );
    }
  }

  // --- states ----------------------------------------------------------------------------
  for (const s of view.states) {
    const fault = worst.get(s.name);
    const stroke = fault ? FAULT_COLOUR[fault.kind] : "var(--cv-line)";
    const fill = fault ? (fault.blocks ? "color-mix(in srgb, var(--cv-err) 18%, transparent)" : "color-mix(in srgb, var(--cv-warn) 18%, transparent)") : "var(--cv-raised)";
    const x = s.x - BOX_W / 2;
    const y = s.y - BOX_H / 2;
    parts.push(
      `<rect x="${x}" y="${y}" width="${BOX_W}" height="${BOX_H}" rx="7" fill="${fill}" ` +
        `stroke="${stroke}" stroke-width="${fault?.blocks ? 2.2 : 1.4}"/>`,
    );
    parts.push(
      `<text x="${s.x}" y="${s.y + 1}" text-anchor="middle" font-size="14" fill="var(--cv-text)">${esc(s.name)}</text>`,
    );
    if (s.initial) {
      // ⚠ Where the variable starts, marked on the box rather than stated in prose beside it.
      parts.push(
        `<text x="${s.x}" y="${s.y + 15}" text-anchor="middle" font-size="10" fill="var(--cv-muted)">initial</text>`,
      );
    }
    if (fault) {
      parts.push(
        `<circle cx="${x + BOX_W - 9}" cy="${y + 9}" r="5.5" fill="${stroke}"/>` +
          `<text x="${x + BOX_W - 9}" y="${y + 12.5}" text-anchor="middle" font-size="8" ` +
          `fill="#fff">${fault.blocks ? "!" : "?"}</text>`,
      );
    }
  }

  parts.push("</svg>");
  return parts.join("");
}

/**
 * The line under the graph.
 *
 * ⚠ **A sentence, not a count.** *"3 findings"* tells a developer to go looking; the sentence is the
 * thing they act on.
 */
/**
 * What colour the check line takes.
 *
 * ⚠ **Found by looking, not by a test.** The line was coloured by `satisfiesP15` alone, so a graph
 * that satisfies P15 *and carries a warning* printed "⚠ ... potential softlock" in the **ok** colour.
 * ▶ **The tone must follow the worst finding present**, because the colour is read before the words
 * are — and a warning painted green is a warning nobody reads.
 */
export function checkTone(view: StateGraphView): "ok" | "warn" | "err" {
  if (view.findings.some((f) => f.blocks)) return "err";
  if (view.findings.length > 0) return "warn";
  // ⚠ P15 can still fail with no finding to point at — that is a claim about the graph as a whole.
  return view.satisfiesP15 ? "ok" : "err";
}

export function checkLine(view: StateGraphView): string {
  if (view.findings.length === 0) {
    return "✓ every state re-enterable. No dead state. P15 satisfied on this graph.";
  }
  return view.findings.map((f) => `${f.blocks ? "⛔" : "⚠"} ${f.message}`).join("\n");
}

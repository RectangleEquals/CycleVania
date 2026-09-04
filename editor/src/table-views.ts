/**
 * **The Curve editor, the Unlock table, the Dials view, and the floor slider.**
 *
 * ⚠ **Every number here was computed by the core.** A curve's shape is what `Row::sample` says it is —
 * an editor that interpolated its own preview would draw a line the generator does not follow, and it
 * would be wrong in exactly the cases that matter: the ones where the interpolation mode is doing
 * something.
 *
 * ▶ **Still no layout.** These are the views' content, drawn; where the panels sit is `10-editor.md`
 * §10's mockups, which have not happened.
 */

// ---------------------------------------------------------------------------------------------
// P03 — the Curve editor, and thumbnails wherever a curve is referenced
// ---------------------------------------------------------------------------------------------

/** One sampled row of a `.cvcurve`. */
export interface CurveRow {
  name: string;
  from: number;
  to: number;
  points: [number, number][];
}

/** A whole curve table. */
export interface CurveTableView {
  path: string;
  domain: string;
  yLabel: string;
  rows: CurveRow[];
}

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/**
 * A curve as a polyline.
 *
 * ⚠ **Scaled to the row's own extent, in both axes.** A shared scale across rows would flatten a row
 * whose values are small beside one whose values are large — and *flat* is what a broken curve looks
 * like, so a shared scale lies in the worst direction.
 */
function polyline(row: CurveRow, w: number, h: number, pad: number): string {
  const ys = row.points.map(([, y]) => y);
  const yMin = Math.min(...ys);
  const yMax = Math.max(...ys);
  const ySpan = yMax - yMin || 1;
  const xSpan = row.to - row.from || 1;
  return row.points
    .map(([x, y]) => {
      const px = pad + ((x - row.from) / xSpan) * (w - pad * 2);
      const py = h - pad - ((y - yMin) / ySpan) * (h - pad * 2);
      return `${px.toFixed(1)},${py.toFixed(1)}`;
    })
    .join(" ");
}

/**
 * A small preview, for **wherever a curve is referenced** — not only in the editor.
 *
 * ⚠ **A thumbnail is how a curve-valued dial shows its value.** A number cannot represent one, and
 * showing the row's *name* tells a developer only that somebody named something.
 */
export function curveThumbnail(row: CurveRow, w = 76, h = 26): string {
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">` +
    `<rect width="${w}" height="${h}" rx="3" fill="#fbfbfb" stroke="#e3e3e3"/>` +
    `<polyline fill="none" stroke="#3a6ea5" stroke-width="1.6" points="${polyline(row, w, h, 4)}"/>` +
    `</svg>`
  );
}

/** The full-size curve editor for one table. */
export function drawCurveTable(view: CurveTableView, w = 460, h = 190): string {
  const rows = view.rows
    .map(
      (r, i) =>
        `<polyline fill="none" stroke="${["#3a6ea5", "#b8860b", "#2f6f3e", "#8a3f8a"][i % 4]}" ` +
        `stroke-width="1.8" points="${polyline(r, w, h, 26)}"/>`,
    )
    .join("");
  const legend = view.rows
    .map(
      (r, i) =>
        `<text x="${34 + i * 108}" y="16" font-size="11" ` +
        `fill="${["#3a6ea5", "#b8860b", "#2f6f3e", "#8a3f8a"][i % 4]}">${esc(r.name)}</text>`,
    )
    .join("");
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" ` +
    `font-family="ui-sans-serif, system-ui, sans-serif">` +
    `<rect width="${w}" height="${h}" rx="6" fill="#fcfcfc" stroke="#e3e3e3"/>` +
    rows +
    legend +
    // ⚠ Both axes named. A curve is read *against an input the project chose*, and a preview without
    // its domain is a shape with no units — which is the same picture for every curve in the project.
    `<text x="${w / 2}" y="${h - 6}" text-anchor="middle" font-size="11" fill="#777">${esc(view.domain)}</text>` +
    `<text x="12" y="${h / 2}" text-anchor="middle" font-size="11" fill="#777" ` +
    `transform="rotate(-90 12 ${h / 2})">${esc(view.yLabel)}</text>` +
    `</svg>`
  );
}

// ---------------------------------------------------------------------------------------------
// P03a — the Unlock table
// ---------------------------------------------------------------------------------------------

/** One row of a `.cvunlock`. */
export interface UnlockRow {
  id: string;
  name: string;
  doc: string;
  supersedes: string[];
}

/** What stops the table building, if anything. */
export interface UnlockFault {
  kind: "supersedes-cycle" | "dangling-supersedes" | "duplicate-id";
  rows: string[];
  message: string;
}

/** The table, and its fault. */
export interface UnlockTableView {
  rows: UnlockRow[];
  fault: UnlockFault | null;
}

/**
 * The Unlock table.
 *
 * ⚠ **`id` is a read-only *column*, not a per-row rule.** It is generated once and never edited,
 * because a rename that moved a key would break every reference without a migration — so the whole
 * column is marked, rather than the view deciding row by row and someone wondering why.
 *
 * ⚠ **A `supersedes` cycle is shown *in the table*, on the rows that form it** — not deferred to a
 * build error. An error message beside a table leaves the developer to find the cycle by eye, which is
 * the work the view exists to remove.
 */
export function drawUnlockTable(view: UnlockTableView): string {
  // ⚠ The fault names the cycle as a path — `a → b → a` — so the same id appears twice. Dedupe, or a
  // row gets marked once for arriving and once for closing the loop.
  const faulted = new Set(view.fault?.rows ?? []);

  const head =
    `<tr>` +
    `<th style="${TH}">id <span style="font-weight:400;color:#999">read-only</span></th>` +
    `<th style="${TH}">name</th><th style="${TH}">supersedes</th><th style="${TH}">doc</th></tr>`;

  const body = view.rows
    .map((r) => {
      const bad = faulted.has(r.id);
      const bg = bad ? "background:#fdf0ee" : "";
      return (
        `<tr style="${bg}">` +
        `<td style="${TD};color:#777;font-family:ui-monospace,monospace">${esc(r.id)}</td>` +
        `<td style="${TD}">${esc(r.name)}</td>` +
        `<td style="${TD};color:${bad ? "#b4341f" : "#444"}">${esc(r.supersedes.join(", "))}</td>` +
        `<td style="${TD};color:#666">${esc(r.doc)}</td>` +
        `</tr>`
      );
    })
    .join("");

  const note = view.fault
    ? `<p style="margin:8px 0 0;font:12px ui-monospace,monospace;color:#b4341f">⛔ ${esc(
        view.fault.message,
      )}</p>`
    : "";

  return `<table style="border-collapse:collapse;width:100%;font:13px ui-sans-serif,system-ui">${head}${body}</table>${note}`;
}

const TH = "text-align:left;padding:5px 8px;border-bottom:1px solid #ddd;font-size:11px;color:#666";
const TD = "padding:5px 8px;border-bottom:1px solid #f0f0f0";

// ---------------------------------------------------------------------------------------------
// P04 — the Dials view
// ---------------------------------------------------------------------------------------------

/** One dial, as the binding reports it. */
export interface DialRowView {
  id: string;
  owner: string;
  kind: string;
  doc: string;
  default: string;
  effective: string;
  source: string;
  overridden: boolean;
  outOfBounds: boolean;
}

/**
 * The standalone Dials view.
 *
 * ⚠ **It is `project.dials` and holds no state of its own.** The panel calls the same `list`/`get`/`set`
 * a host calls — which is what *"the editor is not allowed a private channel"* means in practice. A
 * cached copy here would be a second source of truth about the same numbers.
 *
 * ⚠ **`default` and `effective` both, because neither is derivable from the other.** Only-effective
 * makes *reset* impossible; only-default makes the panel a lie about what the next generate uses.
 */
export function drawDials(dials: DialRowView[]): string {
  if (dials.length === 0) {
    return `<p style="font:13px ui-sans-serif,system-ui;color:#888;margin:0">This project declares no dials.</p>`;
  }
  const head =
    `<tr><th style="${TH}">dial</th><th style="${TH}">kind</th><th style="${TH}">default</th>` +
    `<th style="${TH}">effective</th><th style="${TH}">source</th></tr>`;
  const body = dials
    .map((d) => {
      // ⚠ Out of bounds is a **warning**, not an error: content may have authored a default outside a
      // range it later narrowed, and hiding the dial would hide the mistake.
      const warn = d.outOfBounds ? "color:#b8860b" : "";
      return (
        `<tr>` +
        `<td style="${TD};font-family:ui-monospace,monospace">${esc(d.id)}</td>` +
        `<td style="${TD};color:#666">${esc(d.kind)}</td>` +
        `<td style="${TD};color:#888">${esc(d.default)}</td>` +
        `<td style="${TD};${warn};font-weight:${d.overridden ? 600 : 400}">${esc(d.effective)}</td>` +
        `<td style="${TD};color:#666">${esc(d.source)}</td>` +
        `</tr>`
      );
    })
    .join("");
  return `<table style="border-collapse:collapse;width:100%;font:13px ui-sans-serif,system-ui">${head}${body}</table>`;
}

// ---------------------------------------------------------------------------------------------
// P05 — the floor slider
// ---------------------------------------------------------------------------------------------

/**
 * The floor slider's state.
 *
 * ⚠ **Shared from the start**, because M20 links it to the skeleton view. A slider that owned its
 * value would have to be lifted out the moment a second view needed it — and lifting shared state
 * later means finding every place that read the private copy.
 */
export class FloorSlider {
  #floor = 0;
  #listeners: ((floor: number) => void)[] = [];

  /** The current floor. */
  get floor(): number {
    return this.#floor;
  }

  /** Move it, telling everyone who is drawing against it. */
  set(floor: number): void {
    if (floor === this.#floor) return;
    this.#floor = floor;
    for (const l of this.#listeners) l(floor);
  }

  /** Watch it. */
  onChange(fn: (floor: number) => void): void {
    this.#listeners.push(fn);
  }

  /** How many views are drawing against it. */
  get watchers(): number {
    return this.#listeners.length;
  }
}

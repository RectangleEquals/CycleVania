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
  /** ⚠ **How it reads between keys** — `LINEAR` · `STEP` · `CUBIC`, per row, already in the format. */
  interpolation: string;
  /** ⚠ **The authored keys.** A sample is a *result*; a key is a thing a developer moves. */
  keys: [number, number][];
  /** The sampled shape, computed by the core. */
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
    `<rect width="${w}" height="${h}" rx="3" fill="var(--cv-raised)" stroke="var(--cv-line)"/>` +
    `<polyline fill="none" stroke="var(--cv-accent)" stroke-width="1.6" points="${polyline(row, w, h, 4)}"/>` +
    `</svg>`
  );
}

/** Where a key sits, in view coordinates. */
function place(row: CurveRow, x: number, y: number, w: number, h: number, pad: number): [number, number] {
  const ys = row.points.map(([, v]) => v);
  const yMin = Math.min(...ys);
  const ySpan = Math.max(...ys) - yMin || 1;
  const xSpan = row.to - row.from || 1;
  return [
    pad + ((x - row.from) / xSpan) * (w - pad * 2),
    h - pad - ((y - yMin) / ySpan) * (h - pad * 2),
  ];
}

/**
 * Tangent handles for a key, in view coordinates.
 *
 * ⚠ **Two sides, editable separately** — Unreal's curve editor lets a developer control the incoming
 * and outgoing direction independently, and a single handle would make every key symmetric.
 *
 * ▶ **Drawn only on `CUBIC` rows.** A `LINEAR` or `STEP` row has no tangent to edit, and drawing an
 * inert handle invites a developer to drag something that cannot move.
 */
export function tangents(
  row: CurveRow,
  index: number,
  w = 460,
  h = 190,
  pad = 26,
): { at: [number, number]; incoming: [number, number]; outgoing: [number, number] } | undefined {
  if (row.interpolation.toUpperCase() !== "CUBIC") return undefined;
  const key = row.keys[index];
  if (!key) return undefined;
  const prev = row.keys[index - 1] ?? key;
  const next = row.keys[index + 1] ?? key;
  const at = place(row, key[0], key[1], w, h, pad);
  const before = place(row, prev[0], prev[1], w, h, pad);
  const after = place(row, next[0], next[1], w, h, pad);
  // The handle points a third of the way toward its neighbour — the direction the curve leaves in.
  const lerp = (a: [number, number], b: [number, number]): [number, number] => [
    a[0] + (b[0] - a[0]) / 3,
    a[1] + (b[1] - a[1]) / 3,
  ];
  return { at, incoming: lerp(at, before), outgoing: lerp(at, after) };
}

/** The full-size curve editor for one table. */
export function drawCurveTable(
  view: CurveTableView,
  w = 460,
  h = 190,
  selected?: { row: string; key: number },
): string {
  const colour = (i: number) => ["#5aab55", "#e0a33a", "#3d8bfd", "#b34fa0"][i % 4]!;

  const shapes = view.rows
    .map(
      (r, i) =>
        `<polyline fill="none" stroke="${colour(i)}" stroke-width="1.8" ` +
        `points="${polyline(r, w, h, 26)}"/>`,
    )
    .join("");

  // ⚠ **Keys are objects, not samples.** They are what a developer selects and moves; drawing only
  // the sampled line makes the editor a picture of a curve rather than a way to author one.
  const keys = view.rows
    .map((r, i) =>
      r.keys
        .map((k, j) => {
          const [x, y] = place(r, k[0], k[1], w, h, 26);
          const on = selected?.row === r.name && selected.key === j;
          return (
            `<rect x="${(x - 3.5).toFixed(1)}" y="${(y - 3.5).toFixed(1)}" width="7" height="7" ` +
            `rx="1.5" fill="${on ? "#fff" : colour(i)}" stroke="${colour(i)}" stroke-width="1.4" ` +
            `data-row="${r.name}" data-key="${j}"/>`
          );
        })
        .join(""),
    )
    .join("");

  // ⚠ **Tangents appear on selection**, as in Unreal — every key showing handles is a thicket.
  let handles = "";
  if (selected) {
    const row = view.rows.find((r) => r.name === selected.row);
    const t = row && tangents(row, selected.key, w, h, 26);
    if (t) {
      const arm = (to: [number, number]) =>
        `<line x1="${t.at[0].toFixed(1)}" y1="${t.at[1].toFixed(1)}" x2="${to[0].toFixed(1)}" ` +
        `y2="${to[1].toFixed(1)}" stroke="#e8e8e8" stroke-width="1.1" opacity=".8"/>` +
        `<circle cx="${to[0].toFixed(1)}" cy="${to[1].toFixed(1)}" r="3" fill="#e8e8e8"/>`;
      handles = arm(t.incoming) + arm(t.outgoing);
    }
  }

  const legend = view.rows
    .map(
      (r, i) =>
        `<text x="${34 + i * 112}" y="15" font-size="11" fill="${colour(i)}">${esc(r.name)}` +
        `<tspan fill="var(--cv-muted)" font-size="9"> ${esc(r.interpolation)}</tspan></text>`,
    )
    .join("");

  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" ` +
    `font-family="ui-sans-serif, system-ui, sans-serif">` +
    `<rect width="${w}" height="${h}" rx="4" fill="var(--cv-raised)" ` +
    `stroke="var(--cv-line)"/>` +
    shapes +
    keys +
    handles +
    legend +
    `<text x="${w / 2}" y="${h - 6}" text-anchor="middle" font-size="11" ` +
    `fill="var(--cv-muted, var(--cv-muted))">${esc(view.domain)}</text>` +
    `<text x="12" y="${h / 2}" text-anchor="middle" font-size="11" ` +
    `fill="var(--cv-muted, var(--cv-muted))" transform="rotate(-90 12 ${h / 2})">${esc(view.yLabel)}</text>` +
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
    `<th style="${TH}">id <span style="font-weight:400;color:var(--cv-muted)">read-only</span></th>` +
    `<th style="${TH}">name</th><th style="${TH}">supersedes</th><th style="${TH}">doc</th></tr>`;

  const body = view.rows
    .map((r) => {
      const bad = faulted.has(r.id);
      const bg = bad ? 'class="is-faulted" style="background:color-mix(in srgb, var(--cv-err) 18%, transparent)"' : "";
      return (
        `<tr ${bg}>` +
        `<td style="${TD};color:var(--cv-muted);font-family:ui-monospace,monospace">${esc(r.id)}</td>` +
        `<td style="${TD}">${esc(r.name)}</td>` +
        `<td style="${TD};color:${bad ? "var(--cv-err)" : "var(--cv-text)"}">${esc(r.supersedes.join(", "))}</td>` +
        `<td style="${TD};color:var(--cv-muted)">${esc(r.doc)}</td>` +
        `</tr>`
      );
    })
    .join("");

  const note = view.fault
    ? `<p style="margin:8px 0 0;font:12px ui-monospace,monospace;color:var(--cv-err)">⛔ ${esc(
        view.fault.message,
      )}</p>`
    : "";

  return `<table style="border-collapse:collapse;width:100%;font:13px ui-sans-serif,system-ui">${head}${body}</table>${note}`;
}

const TH = "text-align:left;padding:5px 8px;border-bottom:1px solid var(--cv-line);font-size:11px;color:var(--cv-muted)";
const TD = "padding:5px 8px;border-bottom:1px solid var(--cv-line)";

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
    return `<p style="font:13px ui-sans-serif,system-ui;color:var(--cv-muted);margin:0">This project declares no dials.</p>`;
  }
  const head =
    `<tr><th style="${TH}">dial</th><th style="${TH}">kind</th><th style="${TH}">default</th>` +
    `<th style="${TH}">effective</th><th style="${TH}">source</th></tr>`;
  const body = dials
    .map((d) => {
      // ⚠ Out of bounds is a **warning**, not an error: content may have authored a default outside a
      // range it later narrowed, and hiding the dial would hide the mistake.
      const warn = d.outOfBounds ? "color:var(--cv-warn)" : "";
      return (
        `<tr>` +
        `<td style="${TD};font-family:ui-monospace,monospace">${esc(d.id)}</td>` +
        `<td style="${TD};color:var(--cv-muted)">${esc(d.kind)}</td>` +
        `<td style="${TD};color:var(--cv-muted)">${esc(d.default)}</td>` +
        `<td style="${TD};${warn};font-weight:${d.overridden ? 600 : 400}">${esc(d.effective)}</td>` +
        `<td style="${TD};color:var(--cv-muted)">${esc(d.source)}</td>` +
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

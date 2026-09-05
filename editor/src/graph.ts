/**
 * **The schematic editor's canvas** — Unreal's Blueprint graph, and the surface CycleVania's whole
 * premise rests on.
 *
 * ⚠ **M18 built this editor's *rules* and no graph.** The palette, the connection tiers, the node
 * shapes, `may_paste` and the dial get-node all exist and are tested; nothing drew them — so a design
 * whose premise is visual authoring shipped a UI with no visual authoring in it.
 *
 * # What a node is made of — `10-editor.md` §9a, §9b
 *
 * - **a two-line header**: the name, and a smaller *context* line — `Custom Event`, `Target is Hookshot`
 * - **`exec` pins white, data pins coloured by type**, and ⚠ **shape carries the container**: a value is
 *   a circle, a list is a small grid, so `Ref<Actor>` and `Ref<Actor>[]` are the same colour and
 *   unmistakably different
 * - **inline widgets on unconnected inputs** — an unconnected input is an editable value, not an empty
 *   socket
 * - ⚠ **a corner badge for a latent node**, one that finishes later; a wire cannot say that
 * - **an expander for advanced pins** — §9d's rule, applied to a node
 */

import { pinColour, EXEC } from "./pins.ts";

/** A pin on a node. */
export interface Pin {
  id: string;
  label: string;
  /** `exec` is the white one; anything else is a type name. */
  type: string;
  /** ⚠ A list draws as a grid, not a circle. Colour is the type; shape is the container. */
  list?: boolean;
  /** What an unconnected input shows inline — a default, a picker's current value. */
  inline?: string;
  connected?: boolean;
}

export type NodeKind = "event" | "call" | "pure" | "flow";

export interface GNode {
  id: string;
  title: string;
  /** ⚠ **The second header line** — `Custom Event`, or `Target is Hookshot`. */
  context?: string;
  kind: NodeKind;
  x: number;
  y: number;
  inputs: Pin[];
  outputs: Pin[];
  /** ⚠ **Latent**: it finishes later. Drawn as a corner badge, because a wire cannot say it. */
  latent?: boolean;
  /** ▶ **Pre-placed and inert** — M17 P04's `OVERRIDES` rule drawn. Wiring it turns it live. */
  disabled?: boolean;
  /** Collapsed advanced pins. */
  advanced?: number;
  selected?: boolean;
}

export interface Wire {
  from: { node: string; pin: string };
  to: { node: string; pin: string };
}

export interface GraphView {
  /** `Hookshot › OnPickup` — the canvas breadcrumb. */
  path: string[];
  nodes: GNode[];
  wires: Wire[];
  /** ⚠ `(READ-ONLY)` in the breadcrumb, a watermark, and a dimmed canvas. Three signals for one fact. */
  readOnly?: boolean;
}

const HEADER = 26;
const ROW = 18;
const PAD = 9;
const NODE_W = 168;

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

/** ▶ **The header carries the node's colour** — category at a glance, which is Unreal's coding. */
export function headerColour(kind: NodeKind): string {
  return {
    event: "var(--cv-err)",
    call: "var(--cv-accent)",
    pure: "var(--cv-ok)",
    flow: "var(--cv-muted)",
  }[kind];
}

/** How tall a node draws. ⚠ Rows are max(inputs, outputs) — pins pair up across the body. */
export function nodeHeight(n: GNode): number {
  const rows = Math.max(n.inputs.length, n.outputs.length);
  return HEADER + (n.context ? 11 : 0) + rows * ROW + PAD * 2 + (n.advanced ? 14 : 0);
}

/** Where a pin sits, in canvas coordinates. */
export function pinAt(n: GNode, pin: string): { x: number; y: number } | undefined {
  const i = n.inputs.findIndex((p) => p.id === pin);
  const o = n.outputs.findIndex((p) => p.id === pin);
  const top = n.y + HEADER + (n.context ? 11 : 0) + PAD;
  if (i >= 0) return { x: n.x, y: top + i * ROW + ROW / 2 };
  if (o >= 0) return { x: n.x + NODE_W, y: top + o * ROW + ROW / 2 };
  return undefined;
}

/**
 * A wire.
 *
 * ▶ **Splines, not straight lines** — §9a. A straight line between distant pins reads as a wall; a
 * spline reads as a route.
 */
export function wirePath(a: { x: number; y: number }, b: { x: number; y: number }): string {
  const dx = Math.max(28, Math.abs(b.x - a.x) * 0.45);
  return `M${a.x} ${a.y} C${a.x + dx} ${a.y} ${b.x - dx} ${b.y} ${b.x} ${b.y}`;
}

/** One pin's mark. ⚠ **Circle for a value, grid for a list; `exec` is a white triangle.** */
function pinMark(p: Pin, cx: number, cy: number): string {
  const colour = p.type === "exec" ? EXEC : pinColour(p.type);
  if (p.type === "exec") {
    return (
      `<path d="M${cx - 4} ${cy - 5} L${cx + 5} ${cy} L${cx - 4} ${cy + 5} Z" ` +
      `fill="${p.connected ? colour : "none"}" stroke="${colour}" stroke-width="1.4"/>`
    );
  }
  if (p.list) {
    return (
      `<g fill="${colour}">` +
      [0, 1].flatMap((r) => [0, 1].map((c) =>
        `<rect x="${cx - 4 + c * 4.6}" y="${cy - 4 + r * 4.6}" width="3.4" height="3.4" rx=".6"/>`,
      )).join("") +
      `</g>`
    );
  }
  return (
    `<circle cx="${cx}" cy="${cy}" r="4" fill="${p.connected ? colour : "none"}" ` +
    `stroke="${colour}" stroke-width="1.5"/>`
  );
}

/** One node. */
export function drawNode(n: GNode): string {
  const h = nodeHeight(n);
  const dim = n.disabled ? ` opacity=".55"` : "";
  const top = HEADER + (n.context ? 11 : 0) + PAD;

  const rows = (pins: Pin[], side: "in" | "out") =>
    pins
      .map((p, i) => {
        const y = top + i * ROW + ROW / 2;
        const cx = side === "in" ? 0 : NODE_W;
        const tx = side === "in" ? 12 : NODE_W - 12;
        const anchor = side === "in" ? "start" : "end";
        // ⚠ **An unconnected input is an editable value**, not an empty socket.
        const inline =
          side === "in" && p.inline !== undefined && !p.connected
            ? `<rect x="${tx + p.label.length * 6 + 6}" y="${y - 7}" width="46" height="14" rx="2.5" ` +
              `fill="var(--cv-bg)" stroke="var(--cv-line)"/>` +
              `<text x="${tx + p.label.length * 6 + 29}" y="${y + 3.6}" text-anchor="middle" ` +
              `font-size="9.5" fill="var(--cv-text)">${esc(p.inline)}</text>`
            : "";
        return (
          pinMark(p, cx, y) +
          `<text x="${tx}" y="${y + 3.4}" text-anchor="${anchor}" font-size="10.5" ` +
          `fill="var(--cv-muted)">${esc(p.label)}</text>` +
          inline
        );
      })
      .join("");

  return (
    `<g class="cv-node${n.selected ? " is-selected" : ""}" data-node="${esc(n.id)}" ` +
    `transform="translate(${n.x},${n.y})"${dim}>` +
    `<rect width="${NODE_W}" height="${h}" rx="5" fill="var(--cv-panel)" ` +
    `stroke="${n.selected ? "var(--cv-accent)" : "var(--cv-line)"}" ` +
    `stroke-width="${n.selected ? 2 : 1.2}"/>` +
    // ▶ the header carries the node's colour
    `<path d="M0 5a5 5 0 0 1 5-5h${NODE_W - 10}a5 5 0 0 1 5 5v${HEADER - 5}H0Z" ` +
    `fill="${headerColour(n.kind)}" opacity=".85"/>` +
    `<text x="9" y="17" font-size="11.5" font-weight="600" fill="var(--cv-bg)">${esc(n.title)}</text>` +
    (n.context
      ? `<text x="9" y="${HEADER + 8}" font-size="9" font-style="italic" ` +
        `fill="var(--cv-muted)">${esc(n.context)}</text>`
      : "") +
    // ⚠ a corner badge marks a latent node — one that finishes later
    (n.latent
      ? `<circle cx="${NODE_W - 12}" cy="13" r="6.5" fill="var(--cv-bg)" opacity=".8"/>` +
        `<path d="M${NODE_W - 12} 9v4.2l2.6 1.6" stroke="var(--cv-text)" stroke-width="1.3" ` +
        `fill="none" stroke-linecap="round"/>`
      : "") +
    rows(n.inputs, "in") +
    rows(n.outputs, "out") +
    // ⚠ §9d applied to a node: advanced pins fold
    (n.advanced
      ? `<text x="${NODE_W / 2}" y="${h - 6}" text-anchor="middle" font-size="9" ` +
        `fill="var(--cv-muted)">▾ ${n.advanced} advanced</text>`
      : "") +
    `</g>`
  );
}

/**
 * The whole canvas.
 *
 * ⚠ **A disabled node explains itself.** Unreal's reads *"This node is disabled and will not be called.
 * Drag off pins to build functionality."* — ▶ **which is M17 P04's `OVERRIDES` rule drawn**: every hook
 * in the ancestry, pre-placed, each saying what happens if you leave it alone.
 */
export function drawGraph(g: GraphView, w = 0, h = 0): string {
  // ⚠ **Sized to its content, anchored top-left.** A fixed viewBox stretched to the container's width
  // scaled the whole graph down and floated it in the middle — ▶ **a canvas pans and scrolls; it does
  // not rescale itself to fit a panel.**
  const right = Math.max(...g.nodes.map((n) => n.x + NODE_W), 320) + 60;
  const bottom = Math.max(...g.nodes.map((n) => n.y + nodeHeight(n)), 240) + 60;
  w = w || right;
  h = h || bottom;
  const node = (id: string) => g.nodes.find((n) => n.id === id);
  const wires = g.wires
    .map((wire) => {
      const a = node(wire.from.node);
      const b = node(wire.to.node);
      if (!a || !b) return "";
      const p = pinAt(a, wire.from.pin);
      const q = pinAt(b, wire.to.pin);
      if (!p || !q) return "";
      const pin =
        a.outputs.find((o) => o.id === wire.from.pin) ?? b.inputs.find((i) => i.id === wire.to.pin);
      const colour = pin?.type === "exec" ? EXEC : pinColour(pin?.type ?? "exec");
      return (
        `<path d="${wirePath(p, q)}" fill="none" stroke="${colour}" ` +
        `stroke-width="${pin?.type === "exec" ? 2 : 1.6}" opacity=".9"/>`
      );
    })
    .join("");

  const notes = g.nodes
    .filter((n) => n.disabled)
    .map(
      (n) =>
        `<g transform="translate(${n.x},${n.y - 30})" opacity=".7">` +
        `<rect width="${NODE_W}" height="25" rx="3" fill="var(--cv-raised)" stroke="var(--cv-line)"/>` +
        `<text x="7" y="10.5" font-size="8.5" fill="var(--cv-muted)">This node is disabled and will</text>` +
        `<text x="7" y="19.5" font-size="8.5" fill="var(--cv-muted)">not be called. Drag off pins.</text>` +
        `</g>`,
    )
    .join("");

  return (
    `<svg class="cv-graph" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" ` +
    `preserveAspectRatio="xMinYMin meet" ` +
    `font-family="ui-sans-serif, system-ui, sans-serif">` +
    `<defs><pattern id="cvgrid" width="22" height="22" patternUnits="userSpaceOnUse">` +
    `<path d="M22 0H0V22" fill="none" stroke="var(--cv-line)" stroke-width=".6" opacity=".5"/>` +
    `</pattern></defs>` +
    `<rect width="${w}" height="${h}" fill="var(--cv-bg)"/>` +
    `<rect width="${w}" height="${h}" fill="url(#cvgrid)"/>` +
    (g.readOnly
      ? `<text x="${w / 2}" y="${h / 2}" text-anchor="middle" font-size="46" font-weight="700" ` +
        `fill="var(--cv-line)" opacity=".55">READ-ONLY</text>`
      : "") +
    notes +
    wires +
    g.nodes.map(drawNode).join("") +
    `</svg>`
  );
}

/** The canvas breadcrumb. ⚠ **`(READ-ONLY)` rides on the same line** — one of §9b's three signals. */
export function graphCrumbs(g: GraphView): string {
  return (
    `<div class="cv-gcrumbs">` +
    g.path.map((p, i) => (i ? `<span class="cv-csep">›</span>` : "") + `<span>${esc(p)}</span>`).join("") +
    (g.readOnly ? `<span class="cv-ro">(READ-ONLY)</span>` : "") +
    `<span class="cv-zoom">Zoom 1:1</span></div>`
  );
}

export function graphStyles(): string {
  const V = (n: string) => `var(--cv-${n})`;
  return `
.cv-graphwrap { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.cv-gcrumbs { display: flex; align-items: center; gap: 5px; padding: 5px 10px; font-size: 11px;
  color: ${V("muted")}; border-bottom: 1px solid ${V("line")}; background: ${V("panel")}; }
.cv-ro { color: ${V("warn")}; letter-spacing: .06em; }
.cv-zoom { margin-left: auto; font-family: ui-monospace, monospace; }
.cv-graphscroll { flex: 1 1 auto; min-height: 0; overflow: auto; background: ${V("bg")}; }
.cv-graph { display: block; }
.cv-node { cursor: pointer; }
.cv-node:hover rect:first-of-type { stroke: ${V("accent")}; }
/* WARN **A refusal is printed where the attempt happened** — SS9b. A rule enforced silently is
   indistinguishable from a bug. */
.cv-osec { display: flex; align-items: center; gap: 6px; padding: 7px 8px 3px;
  color: ${V("muted")}; font-size: 10px; text-transform: uppercase; letter-spacing: .09em; }
.cv-ocount { opacity: .75; }
.cv-oadd { margin-left: auto; background: none; border: 0; color: ${V("muted")}; cursor: pointer;
  font: inherit; line-height: 1; padding: 0 4px; }
.cv-oadd:hover { color: ${V("accent")}; }
.cv-orow { display: flex; align-items: center; gap: 6px; padding: 3px 8px; border-radius: ${V("radius")};
  cursor: pointer; font-size: 12px; }
.cv-orow:hover { background: ${V("raised")}; }
.cv-orow.is-selected { background: ${V("selected")}; box-shadow: inset 2px 0 0 ${V("accent")}; }
.cv-swatch { width: 9px; height: 9px; border-radius: 50%; flex: 0 0 auto; }
.cv-otype { margin-left: auto; color: ${V("muted")}; font-size: 10.5px; }
.cv-refusal { position: fixed; z-index: 70; pointer-events: none; background: ${V("panel")};
  border: 1px solid ${V("err")}; color: ${V("text")}; font-size: 11px; padding: 4px 8px;
  border-radius: ${V("radius")}; box-shadow: 0 4px 14px rgb(0 0 0 / .4); }
`;
}

/**
 * The schematic's Outline — Unreal's `My Blueprint`.
 *
 * WARN **A variable row carries its type's colour swatch and the type's name** — RARR the same colour
 * the pin will be, so the panel and the graph agree before a wire exists.
 * WARN **Section headers carry counts**, which is the cheapest possible answer to *"is there anything
 * in here"*. And the outline is **scoped to the active document tab**: a hook's locals belong to it.
 */
export function drawOutline(g: GraphView): string {
  const section = (title: string, count: number, rows: string) =>
    `<div class="cv-osec"><span>${esc(title)}</span>` +
    `<span class="cv-ocount">${count}</span><button class="cv-oadd" title="Add">+</button></div>` +
    (rows || `<div class="cv-empty">none</div>`);

  const typed = (name: string, type: string) =>
    `<div class="cv-orow"><span class="cv-swatch" style="background:${pinColour(type)}"></span>` +
    `<span>${esc(name)}</span><span class="cv-otype">${esc(type)}</span></div>`;

  const hooks = g.nodes.filter((n) => n.kind === "event");
  const dials = g.nodes.filter((n) => n.context?.startsWith("Dial"));

  return (
    section("Graphs", 1, `<div class="cv-orow is-selected"><span>${esc(g.path.at(-1) ?? "")}</span></div>`) +
    section(
      "Hooks",
      hooks.length,
      hooks
        .map(
          (n) =>
            `<div class="cv-orow"><span>${esc(n.title)}</span>` +
            (n.disabled ? `<span class="cv-otype">unused</span>` : "") +
            `</div>`,
        )
        .join(""),
    ) +
    section("Dials", dials.length, dials.map((n) => typed(n.title, "float")).join("")) +
    section("Variables", 0, "")
  );
}

/**
 * **The shell's frame** — menu bar, asset tabs, toolbar, document tabs, docks, status bar.
 *
 * ⚠ **This replaces M20a's topbar-navigator-stage-inspector arrangement, which was a different
 * architecture rather than a placeholder for one.** That shell grew a `Views` navigator listing
 * *Content*, *State graph* and *Curve editor* as things you pick — beside a second panel also called
 * Content. ▶ **Neither reference engine has a view picker**: a surface appears because of what you are
 * editing. `10-editor.md` §2.
 *
 * # What the frame is made of
 *
 * Three tab strips, and conflating them is how a UI becomes soup — `10-editor.md` §9b:
 *
 * - **the window's strip** — the open *assets*, plus the generated result, which cannot be closed
 * - **the stage's strip** — one asset's own *documents*
 * - **a dock's strip** — panels sharing one slot
 *
 * ⚠ **The toolbar and the menu bar both belong to the active asset tab**, not to the window. Unreal's
 * level tab reads `File Edit Window Tools Build Select Actor Help`; its Blueprint tab reads
 * `File Edit Asset View Debug Window Tools Help`. The whole chrome changes.
 */

/** Where a dock sits. A dock never floats — `10-editor.md` §9b keeps floating for asset editors. */
export type DockSide = "left" | "right" | "bottom";

import { icon, LAYER_ICON } from "./icons.ts";

/**
 * A toolbar group.
 *
 * ⚠ **`intent` is where it sits; `keep` is how long it survives a narrowing window.** They are separate
 * numbers because they disagree: `run` sits last and survives longest, `project` sits first and goes
 * early. ▶ A single ordering could not express that, and the design's phrase *"reverse order of intent"*
 * read as if it could.
 */
export interface ToolGroup {
  id: string;
  /** Left-to-right position: 1 project · 2 the surface's verb · 3 create · 4 docks · 5 run. */
  intent: 1 | 2 | 3 | 4 | 5;
  /** Higher survives longer. ⚠ `run` and the surface's verb are the last to go. */
  keep: number;
  items: ToolItem[];
}

export interface ToolItem {
  id: string;
  label: string;
  /** ⚠ A caret marks a button that opens a menu — Unreal's convention, and it predicts the click. */
  menu?: boolean;
  /** A toggle renders depressed while on. */
  toggle?: boolean;
  on?: boolean;
  /** ⚠ **Disabled, not hidden**, and the tooltip says what it needs. */
  enabled?: boolean;
  hint?: string;
  /** ⚠ **An icon never travels alone** — §9e. It makes the label findable, never replaces it. */
  icon?: string;
}

/** One asset open in the window's tab strip. */
export interface AssetTab {
  id: string;
  label: string;
  /** ⚠ The generated result cannot be closed — the stage never empties. */
  fixed?: boolean;
  dirty?: boolean;
  /** Right-aligned in the strip: the active asset's defining fact. Unreal's `Parent class: Actor`. */
  fact?: string;
}

/** A dock, and whether it has anything to say. */
export interface Dock {
  id: string;
  label: string;
  side: DockSide;
  /** ⚠ **§9d: a dock with nothing to say is not drawn.** Absent, not empty. */
  present: boolean;
  /** Collapsed to a labelled strip on the window edge, as Unreal does. */
  collapsed?: boolean;
  body?: string;
}

/** What the whole project is true of, regardless of which tab is open. */
export interface StatusFacts {
  /** ⚠ **The fingerprint has no other home** — in a panel it looks like a property of something. */
  fingerprint: string;
  seed: number | null;
  unsaved: number;
}

const V = (n: string) => `var(--cv-${n})`;
const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

// ---------------------------------------------------------------------------------------------
// P02b — the toolbar survives a narrow window
// ---------------------------------------------------------------------------------------------

/** Roughly how wide a group renders, in px. Enough to decide what fits; not a layout engine. */
export function groupWidth(g: ToolGroup): number {
  // padding + per-item label width + caret
  return 12 + g.items.reduce((w, i) => w + 16 + i.label.length * 7 + (i.menu ? 12 : 0), 0);
}

/**
 * Split groups into what is drawn and what goes to the `»` overflow.
 *
 * ⚠ **A toolbar that clips silently makes a missing button look like a missing feature.** Groups drop
 * whole — a half-group reads as a bug — lowest `keep` first, and the survivors stay in `intent` order.
 */
export function fitToolbar(groups: ToolGroup[], available: number): {
  shown: ToolGroup[];
  overflow: ToolGroup[];
} {
  const byIntent = [...groups].sort((a, b) => a.intent - b.intent);
  if (byIntent.reduce((w, g) => w + groupWidth(g), 0) <= available) {
    return { shown: byIntent, overflow: [] };
  }
  // ⚠ Reserve room for the `»` button itself, or the last drop leaves it clipped.
  const budget = available - 28;
  const dropOrder = [...byIntent].sort((a, b) => a.keep - b.keep);
  const dropped = new Set<string>();
  let width = byIntent.reduce((w, g) => w + groupWidth(g), 0);
  for (const g of dropOrder) {
    if (width <= budget) break;
    dropped.add(g.id);
    width -= groupWidth(g);
  }
  return {
    shown: byIntent.filter((g) => !dropped.has(g.id)),
    overflow: byIntent.filter((g) => dropped.has(g.id)),
  };
}

// ---------------------------------------------------------------------------------------------
// P03 — the layer switcher
// ---------------------------------------------------------------------------------------------

/** The pipeline's five layers, as the stage sees them. ⚠ Six layers L0-L5; L0 is content, not a view. */
export const LAYERS = [
  { id: "L1", label: "Mission", doc: "is this solvable? what gates what?" },
  { id: "L2", label: "Skeleton", doc: "what does it look like, and how do you move through it?" },
  { id: "L3", label: "Volume", doc: "what space actually got carved" },
  { id: "L4", label: "Geometry", doc: "what got built" },
  { id: "L5", label: "Final", doc: "what a player would see" },
] as const;

/**
 * The layer switcher as a toolbar group.
 *
 * ⚠ **A layer that has not been generated yet is disabled, not hidden.** *"You have not run this far"*
 * and *"this does not exist"* are different answers, and a missing button gives the wrong one.
 */
export function layerGroup(reached: number, active: string): ToolGroup {
  return {
    id: "layers",
    intent: 2,
    keep: 90,
    items: LAYERS.map((l, i) => ({
      id: l.id,
      // ⚠ **Named, never numbered.** `L3` means nothing to anyone who has not read the pipeline;
      // ▶ Godot's workspace switcher reads `2D 3D Script`, not `W1 W2 W3`, and needs no explaining.
      label: l.label,
      icon: LAYER_ICON[l.id],
      toggle: true,
      // ⚠ **A layer nobody has generated cannot be the live one.** Marking L1 active while it is
      // also disabled says two contradictory things at once, and a developer believes the highlight.
      on: l.id === active && i < reached,
      enabled: i < reached,
      hint: i < reached ? `${l.id} ${l.label} — ${l.doc}` : `${l.id} ${l.label} — not generated yet`,
    })),
  };
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

/** The menu bar. ⚠ Its contents belong to the active tab, like the toolbar's. */
export function menuBar(menus: string[]): string {
  return (
    `<div class="cv-menubar">` +
    menus.map((m) => `<button class="cv-menu" data-menu="${esc(m)}">${esc(m)}</button>`).join("") +
    `</div>`
  );
}

/** The window's asset tab strip. */
export function assetTabs(tabs: AssetTab[], active: string): string {
  const fact = tabs.find((t) => t.id === active)?.fact;
  return (
    `<div class="cv-assettabs">` +
    tabs
      .map(
        (t) =>
          `<button class="cv-atab${t.id === active ? " is-active" : ""}" data-asset="${esc(t.id)}" ` +
          `title="Asset: ${esc(t.label)}">${esc(t.label)}` +
          (t.dirty ? `<span class="cv-dirty">•</span>` : "") +
          (t.fixed ? "" : `<span class="cv-x" data-close="${esc(t.id)}">×</span>`) +
          `</button>`,
      )
      .join("") +
    (fact ? `<span class="cv-fact">${esc(fact)}</span>` : "") +
    `</div>`
  );
}

function toolItem(i: ToolItem): string {
  const cls =
    "cv-titem" + (i.toggle && i.on ? " is-on" : "") + (i.enabled === false ? " is-off" : "");
  return (
    `<button class="${cls}" data-tool="${esc(i.id)}"` +
    (i.enabled === false ? " disabled" : "") +
    (i.hint ? ` title="${esc(i.hint)}"` : "") +
    `>${i.icon ? icon(i.icon, 13) : ""}${esc(i.label)}` +
    `${i.menu ? `<span class="cv-caret">▾</span>` : ""}</button>`
  );
}

/**
 * The toolbar.
 *
 * ⚠ **A group is bounded, not merely spaced** — a raised rounded container per group. Unreal separates
 * with a hairline rule; ▶ on a dark ground at small sizes a hairline reads as noise and a container
 * reads as a category. `10-editor.md` §9b records the divergence.
 */
export function toolbar(groups: ToolGroup[], available = 1200): string {
  const { shown, overflow } = fitToolbar(groups, available);
  const draw = (g: ToolGroup) =>
    `<div class="cv-tgroup" data-group="${esc(g.id)}">${g.items.map(toolItem).join("")}</div>`;
  // ⚠ `run` is pushed right, as Unreal pushes Play — position is part of what says "this is the verb".
  const left = shown.filter((g) => g.intent < 5).map(draw).join("");
  const right = shown.filter((g) => g.intent === 5).map(draw).join("");
  const more = overflow.length
    ? `<button class="cv-more" data-overflow="${overflow.map((g) => g.id).join(",")}" ` +
      `title="${overflow.length} more group${overflow.length > 1 ? "s" : ""}">»</button>`
    : "";
  return `<div class="cv-toolbar">${left}<div class="cv-tspacer"></div>${more}${right}</div>`;
}

/** The stage's own document tabs. */
export function documentTabs(tabs: { id: string; label: string }[], active: string): string {
  if (tabs.length === 0) return "";
  return (
    `<div class="cv-doctabs">` +
    tabs
      .map(
        (t) =>
          `<button class="cv-dtab${t.id === active ? " is-active" : ""}" data-doc="${esc(t.id)}">` +
          `${esc(t.label)}</button>`,
      )
      .join("") +
    `</div>`
  );
}

/**
 * A dock.
 *
 * ⚠ **Absent when it has nothing to say** (§9d), collapsed to a labelled edge strip when the developer
 * folds it — ▶ so a closed dock is still a *place* rather than an absence.
 */
export function dock(d: Dock): string {
  if (!d.present) return "";
  if (d.collapsed) {
    return `<div class="cv-dock is-collapsed cv-${d.side}" data-dock="${esc(d.id)}">
      <button class="cv-dstrip" data-expand="${esc(d.id)}">${esc(d.label)}</button></div>`;
  }
  return `<div class="cv-dock cv-${d.side}" data-dock="${esc(d.id)}">
    <div class="cv-dhead"><span class="cv-dtitle">${icon(d.id, 12)}${esc(d.label)}</span>
      <button class="cv-dfold" data-collapse="${esc(d.id)}" title="Collapse">–</button></div>
    <div class="cv-dbody">${d.body ?? ""}</div></div>`;
}

/**
 * The status bar.
 *
 * ⚠ **The fingerprint belongs here and nowhere else.** It answers *"is this the same build"*, it is
 * true regardless of which tab is open, and putting it in a panel makes it look like a property of
 * something.
 */
export function statusBar(f: StatusFacts): string {
  const seed = f.seed === null ? "—" : String(f.seed);
  return (
    `<div class="cv-status">` +
    `<button class="cv-sbtn" data-drawer="content">${icon("content", 13)}Content Drawer</button>` +
    `<button class="cv-sbtn" data-drawer="output">Output Log</button>` +
    `<div class="cv-tspacer"></div>` +
    `<span class="cv-sfact" title="Core version, content digests and config — the build's identity">` +
    `fingerprint ${esc(f.fingerprint)}</span>` +
    `<span class="cv-sdot">·</span>` +
    `<span class="cv-sfact" title="Orthogonal to the fingerprint: a different seed is the same recipe">` +
    `seed ${esc(seed)}</span>` +
    `<span class="cv-sdot">·</span>` +
    `<span class="cv-sfact${f.unsaved ? " cv-warn" : ""}">` +
    `${f.unsaved ? `${f.unsaved} unsaved` : "all saved"}</span>` +
    `</div>`
  );
}

/** The frame's stylesheet. ⚠ Nothing here picks a hex. */
export function frameStyles(): string {
  return `
#app { display: grid; grid-template-rows: auto auto auto 1fr auto; height: 100vh; min-height: 0; }

.cv-menubar { display: flex; align-items: center; gap: 2px; padding: 3px 8px;
  background: ${V("panel")}; border-bottom: 1px solid ${V("line")}; }
.cv-menu { background: none; border: 0; color: ${V("text")}; font: inherit; cursor: pointer;
  padding: 3px 9px; border-radius: ${V("radius")}; }
.cv-menu:hover { background: ${V("raised")}; }
.cv-menu:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }

.cv-assettabs { display: flex; align-items: stretch; gap: 1px; background: ${V("bg")};
  border-bottom: 1px solid ${V("line")}; padding: 0 6px; }
.cv-atab { display: flex; align-items: center; gap: 5px; background: transparent; border: 0;
  border-bottom: 2px solid transparent; color: ${V("muted")}; font: inherit; cursor: pointer;
  padding: 6px 12px; }
.cv-atab:hover { color: ${V("text")}; background: ${V("panel")}; }
.cv-atab.is-active { color: ${V("text")}; background: ${V("panel")}; border-bottom-color: ${V("accent")}; }
.cv-atab:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }
.cv-dirty { color: ${V("accent")}; }
.cv-x { color: ${V("muted")}; font-size: 14px; line-height: 1; }
.cv-x:hover { color: ${V("err")}; }
.cv-fact { margin-left: auto; align-self: center; color: ${V("muted")}; font-size: 11px;
  padding-right: 8px; }

.cv-toolbar { display: flex; align-items: center; gap: 8px; padding: 5px 8px;
  background: ${V("panel")}; border-bottom: 1px solid ${V("line")}; }
/* ⚠ A group is bounded, not merely spaced — a deliberate divergence from Unreal's hairline rule. */
.cv-tgroup { display: flex; gap: 2px; padding: 2px; border: 1px solid ${V("line")};
  border-radius: ${V("radius")}; background: ${V("raised")}; }
.cv-titem { background: transparent; border: 0; color: ${V("text")}; font: inherit; cursor: pointer;
  padding: 3px 9px; border-radius: ${V("radius")}; display: inline-flex; align-items: center; gap: 5px; }
.cv-icon { flex: 0 0 auto; }
.cv-titem:hover:not(:disabled) { background: ${V("panel")}; }
.cv-titem:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }
/* ⚠ A toggle you cannot read the state of is a button that lies. */
.cv-titem.is-on { background: ${V("selected")}; box-shadow: inset 0 0 0 1px ${V("accent")}; }
.cv-titem:disabled { opacity: .4; cursor: default; }
.cv-caret { color: ${V("muted")}; font-size: 9px; }
.cv-tspacer { flex: 1 1 auto; }
.cv-more { background: ${V("raised")}; border: 1px solid ${V("line")}; color: ${V("text")};
  border-radius: ${V("radius")}; cursor: pointer; padding: 3px 8px; font: inherit; }
.cv-more:hover { border-color: ${V("accent")}; }

/* ⚠ **Flex, not grid, and the reason is a bug this had.** A grid with a fixed track template and a
   variable number of children silently reassigns tracks: with the Outline dock absent, the stage took
   the auto column and the Details dock took the 1fr, so the stage squeezed to its content and a
   300px panel stretched across the window. ▶ **Docks appear and disappear by design** (§9d), so the
   layout may not depend on how many there are. */
.cv-frame { display: flex; align-items: stretch; min-height: 0; }
.cv-centre { display: flex; flex-direction: column; flex: 1 1 auto; min-width: 0; min-height: 0; }
/* ⚠ **Not a child selector.** This was a direct-child rule and stopped matching the moment
   the stage gained a wrapper for the drawer, so the stage sized to its own text and left dead space
   under it. ▶ **The same defect as the grid tracks**: a rule that encodes a DOM shape breaks when
   the shape moves, and neither the type checker nor a unit test can see it. */
.cv-centre .cv-stage { flex: 1 1 auto; min-height: 0; }
.cv-dock.cv-left, .cv-dock.cv-right { flex: 0 0 auto; }
.cv-dock.cv-bottom { flex: 0 0 auto; }

.cv-doctabs { display: flex; gap: 1px; background: ${V("panel")};
  border-bottom: 1px solid ${V("line")}; padding: 0 6px; }
.cv-dtab { background: transparent; border: 0; border-bottom: 2px solid transparent;
  color: ${V("muted")}; font: inherit; cursor: pointer; padding: 5px 11px; font-size: 12px; }
.cv-dtab:hover { color: ${V("text")}; }
.cv-dtab.is-active { color: ${V("text")}; border-bottom-color: ${V("accent")}; }
.cv-dtab:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }

.cv-dock { background: ${V("panel")}; display: flex; flex-direction: column; min-height: 0; }
.cv-dock.cv-left { border-right: 1px solid ${V("line")}; width: 224px; }
.cv-dock.cv-right { border-left: 1px solid ${V("line")}; width: 300px; }
.cv-dock.cv-bottom { border-top: 1px solid ${V("line")}; height: 216px; }
.cv-dock.is-collapsed { width: auto; height: auto; }
.cv-dtitle { display: inline-flex; align-items: center; gap: 6px; }
.cv-dhead { display: flex; align-items: center; justify-content: space-between; padding: 5px 8px;
  border-bottom: 1px solid ${V("line")}; color: ${V("muted")}; font-size: 10px;
  text-transform: uppercase; letter-spacing: .09em; }
.cv-dfold { background: none; border: 0; color: ${V("muted")}; cursor: pointer; font: inherit;
  line-height: 1; padding: 0 4px; }
.cv-dfold:hover { color: ${V("accent")}; }
.cv-dbody { overflow: auto; padding: 6px; min-height: 0; flex: 1 1 auto; }
/* ⚠ A collapsed dock stays a place: the label reads vertically on the window edge. */
.cv-dstrip { writing-mode: vertical-rl; background: ${V("panel")}; border: 0;
  border-left: 1px solid ${V("line")}; color: ${V("muted")}; font: inherit; cursor: pointer;
  padding: 10px 5px; letter-spacing: .06em; }
.cv-dstrip:hover { color: ${V("accent")}; }
.cv-dock.cv-bottom.is-collapsed .cv-dstrip { writing-mode: horizontal-tb; border: 0;
  border-top: 1px solid ${V("line")}; width: 100%; text-align: left; padding: 5px 10px; }

.cv-status { display: flex; align-items: center; gap: 6px; padding: 4px 8px;
  background: ${V("panel")}; border-top: 1px solid ${V("line")}; font-size: 11px;
  color: ${V("muted")}; }
.cv-sbtn { display: inline-flex; align-items: center; gap: 5px; background: transparent;
  border: 1px solid transparent; color: ${V("text")};
  font: inherit; cursor: pointer; padding: 2px 8px; border-radius: ${V("radius")}; }
.cv-sbtn:hover { border-color: ${V("line")}; background: ${V("raised")}; }
.cv-sbtn:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -1px; }
.cv-sfact { font-family: ui-monospace, "Cascadia Code", Consolas, monospace; }
.cv-sdot { opacity: .5; }
`;
}

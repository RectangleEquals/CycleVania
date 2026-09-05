/**
 * The editor's browser half.
 *
 * ⚠ **M20b replaced M20a's arrangement rather than restyling it.** That shell had a `Views` navigator
 * listing *Content*, *State graph* and *Curve editor* as things a developer picks — beside a second
 * panel also called Content, and a curve widget inside a state-graph pane. ▶ **The defect was that
 * nothing answered *why a view is on screen***, so the shell invented a picker. `10-editor.md` §2 now
 * answers it: the stage holds one generated result seen at five depths, or an asset you opened;
 * everything else is a dock describing what the stage holds.
 *
 * ⚠ **Every colour comes from the four theme parameters.** Nothing here picks a hex.
 */

import { install } from "./theme.ts";
import {
  assetTabs,
  dock,
  documentTabs,
  frameStyles,
  menuBar,
  statusBar,
  toolbar,
  type AssetTab,
  type Dock,
} from "./frame.ts";
import { assetSurface, resultSurface, type Surface } from "./surfaces.ts";
import { shellStyles } from "./shell.ts";
import { browserStyles, createMenu, drawBrowser, type BrowserState } from "./browser.ts";
import { iconForPath } from "./icons.ts";
import { drawTags, tagStyles, type TagDef } from "./tag-editor.ts";
import { drawGraph, drawOutline, graphCrumbs, graphStyles, type GraphView } from "./graph.ts";
import {
  curveStyles,
  drawCanvas,
  drawChannels,
  drawCurveToolbar,
  drawGrid,
  type CurveAsset,
  type CurveState,
} from "./curve-editor.ts";
import {
  detailsStyles,
  drawDetails,
  type ClassDef,
  type DetailsState,
  type Subject,
} from "./details.ts";
import { checkLine, checkTone, drawStateGraph, type StateGraphView } from "./state-view.ts";
import {
  curveThumbnail,
  drawCurveTable,
  drawDials,
  drawUnlockTable,
  type CurveTableView,
  type DialRowView,
  type UnlockTableView,
} from "./table-views.ts";

/** `08-graph-resources.md` §9 — the water level. */
const WATER = `Begin StateGraph Version=1 Path=/Content/States/WaterLevel Id=stg_01
   Variable="water_level"
   Begin State Name="low" Id=stt_01 Pos=(-170,0)
      Initial=true
   End State
   Begin State Name="mid" Id=stt_02 Pos=(0,0)
   End State
   Begin State Name="high" Id=stt_03 Pos=(170,0)
   End State
   Begin Transition From="low" To="mid" Id=trn_01
   End Transition
   Begin Transition From="mid" To="low" Id=trn_02
   End Transition
   Begin Transition From="mid" To="high" Id=trn_03
   End Transition
   Begin Transition From="high" To="mid" Id=trn_04
      Gate=(Form=HoldsRule,unlock=Asset'/Content/Progression/unlocks.cvunlock'#"IronBoots")
   End Transition
End StateGraph
`;

const CURVES = `
{ "version": 1,
  "domain":  "depth",
  "y_label": "multiplier",
  "rows": {
    "complexity":     { "interpolation": "CUBIC",  "points": [[0.0,1.0],[0.5,3.0],[1.0,6.0]] },
    "hazard_density": { "interpolation": "LINEAR", "points": [[0.0,0.1],[1.0,0.8]] },
    "tier":           { "interpolation": "STEP",   "points": [[0.0,1.0],[0.5,2.0],[1.0,3.0]] }
  } }`;

/** ⚠ Deliberately cyclic, so the table shows what a `supersedes` cycle looks like. */
const UNLOCKS = `{
  "version": 1,
  "unlocks": [
    { "id": "u_grapple", "name": "Grapple", "doc": "reach a ledge one jump away" },
    { "id": "u_grapple_2", "name": "Long Grapple", "supersedes": ["u_grapple"], "doc": "twice the reach" },
    { "id": "u_boots", "name": "Iron Boots", "supersedes": ["u_weights"], "doc": "sink, and walk the floor" },
    { "id": "u_weights", "name": "Dive Weights", "supersedes": ["u_boots"], "doc": "an older way down" }
  ]
}`;

/**
 * A schematic's `grants` hook, drawn.
 *
 * ⚠ **A first build put `Event Tick` and `Event BeginPlay` on this canvas, and neither exists.**
 * They were copied from Unreal's screenshots without checking the manifest. ▶ **CycleVania has no
 * runtime and therefore no tick**: every hook is a question asked *during generation*, and the manifest
 * declares 59 of them — `pivot`, `footprint`, `grants`, `judge`, `on_placed`, `on_finalized` and the
 * rest. **The reference supplies the shape of a node, never the name of one.**
 *
 * ▶ `_configure` on `/Core/Object` is the construction script's equivalent, and it is the **Setup
 * Graph** rather than a hook you wire in the logic graph.
 */
const HOOK_GRAPH: GraphView = {
  path: ["Hookshot", "grants"],
  nodes: [
    {
      id: "ev", title: "grants", context: "Hook on Actor", kind: "event", x: 40, y: 130,
      inputs: [],
      outputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "ctx", label: "ctx", type: "Ref<Context>", connected: true },
      ],
    },
    {
      id: "dial", title: "rope_length", context: "Dial on Hookshot", kind: "pure", x: 40, y: 262,
      inputs: [],
      outputs: [{ id: "v", label: "Value", type: "float", connected: true }],
    },
    {
      id: "held", title: "Holds", context: "Target is Context", kind: "pure", x: 300, y: 128,
      inputs: [
        { id: "ctx", label: "ctx", type: "Ref<Context>", connected: true },
        { id: "unlock", label: "Unlock", type: "Unlock", inline: "Grapple" },
      ],
      outputs: [{ id: "out", label: "", type: "bool", connected: true }],
    },
    {
      id: "branch", title: "Branch", kind: "flow", x: 300, y: 236,
      inputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "cond", label: "Condition", type: "bool", connected: true },
      ],
      outputs: [
        { id: "true", label: "True", type: "exec", connected: true },
        { id: "false", label: "False", type: "exec" },
      ],
    },
    {
      id: "ret", title: "Return", context: "Array<Unlock>", kind: "call", x: 570, y: 232,
      inputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "value", label: "Unlocks", type: "Unlock", list: true, inline: "1 row" },
      ],
      outputs: [],
      selected: true,
    },
    {
      // ▶ **Pre-placed, disabled, and explaining itself** — M17 P04's `OVERRIDES` rule drawn.
      // ⚠ A *real* hook this time: `on_placed` is on `/Core/Actor`.
      id: "placed", title: "on_placed", context: "Hook on Actor", kind: "event", x: 570, y: 380,
      inputs: [],
      outputs: [
        { id: "exec", label: "", type: "exec" },
        { id: "ctx", label: "ctx", type: "Ref<Context>" },
      ],
      disabled: true,
    },
  ],
  wires: [
    { from: { node: "ev", pin: "exec" }, to: { node: "branch", pin: "exec" } },
    { from: { node: "ev", pin: "ctx" }, to: { node: "held", pin: "ctx" } },
    { from: { node: "held", pin: "out" }, to: { node: "branch", pin: "cond" } },
    { from: { node: "branch", pin: "true" }, to: { node: "ret", pin: "exec" } },
    { from: { node: "dial", pin: "v" }, to: { node: "ret", pin: "value" } },
  ],
};

/**
 * The schematic's components.
 *
 * ⚠ **This was missing entirely, and it is how an `Item` owns behaviour** — [`05-object-model.md`]
 * P11 makes the Actor the sole point of contact and the *component* the mechanic's identity, so a
 * schematic with no way to add one cannot express a mechanic. ▶ **Unreal stacks `Components` above
 * `My Blueprint` in the left dock**, and the same split applies: *what this object is made of*, then
 * *what it can be asked*.
 */
const COMPONENTS = [
  { name: "Hookshot", kind: "Item", path: "/Core/Item", root: true, collision: false },
  { name: "Reach", kind: "TraversalComponent", path: "/Core/TraversalComponent", collision: false },
  { name: "Hull", kind: "MountComponent", path: "/Core/MountComponent", collision: true },
];

/** Stand-in content, until a project is open./** Stand-in content, until a project is open. ⚠ Shaped like a real content root, not invented. */
/** ⚠ A tag set makes a `Tag` field a picker; without one it degrades to free text. */
const SAMPLE_TAGS: TagDef[] = [
  { name: "surface.stone", doc: "bare stone", uses: 6 },
  { name: "surface.stone.wet", doc: "slippery underfoot", uses: 0 },
  { name: "surface.metal.grate", doc: "see-through floor", uses: 2 },
  { name: "hazard.fire", doc: "burns on contact", uses: 3 },
  { name: "hazard.fall", uses: 1 },
];

const SAMPLE_CONTENT = [
  "schematics/Hookshot.cvs",
  "schematics/Plaque.cvs",
  "schematics/doors/IronDoor.cvs",
  "spines/reach.cvspine",
  "states/WaterLevel.cvstate",
  "curves/progression.cvcurve",
  "progression/unlocks.cvunlock",
  "tags/surfaces.cvtags",
];

async function post<T>(route: string, payload: unknown): Promise<T> {
  const res = await fetch(route, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  return (await res.json()) as T;
}

/** What the editor is currently showing. */
interface Ui {
  asset: string;
  tabs: AssetTab[];
  layer: string;
  /** How far the pipeline has run. ⚠ `Project::generate` is a stub, so nothing is reached yet. */
  reached: number;
  docks: Record<string, boolean>;
  collapsed: Record<string, boolean>;
  /** ⚠ **One browser.** `open` is the drawer showing; `docked` is whether it sits in the layout. */
  browser: BrowserState & { open: boolean };
  menuOpen: boolean;
  /** ⚠ **Anchored to the button that opened it.** Two buttons open this menu; a fixed offset is
   * right for at most one of them and floats over the stage for the other. */
  menuAt: [number, number];
  details: DetailsState;
  curve: CurveState;
  /** ⚠ Hidden channels, by name. Visibility is what makes a shared scale honest. */
  hidden: Record<string, boolean>;
  tag?: string;
  /** ⚠ What the Details panel is describing. Null is a real state, and it says so. */
  subject: Subject | null;
  /** Which of the asset's own documents is open. */
  doc: string;
}

let ui: Ui;
let views: {
  state: StateGraphView;
  curves: CurveTableView;
  unlocks: UnlockTableView;
  dials: DialRowView[];
  noProject: boolean;
  version: string;
  fingerprint: string;
  /** ▶ M17's generated artifact — 150 classes, and now the enum variants a dropdown needs. */
  classes: ClassDef[];
};

/** ⚠ The extension decides which editor opens — `10-editor.md` §2. There is no other model. */
const KIND_OF: Record<string, string> = {
  cvs: "Schematic",
  cvspine: "Spine",
  cvstate: "State graph",
  cvcurve: "Curve table",
  cvunlock: "Unlock table",
  cvtags: "Tag set",
};
const kindOf = (path: string) => KIND_OF[path.slice(path.lastIndexOf(".") + 1)] ?? "Asset";

/** ⚠ One mark per concept — the same icon in the tile, the tab and the Details subject. */
const ICON_OF: Record<string, string> = {
  cvs: "schematic", cvspine: "spine", cvstate: "state",
  cvcurve: "curve", cvunlock: "unlock", cvtags: "tags",
};
const iconOf = (path: string) => ICON_OF[path.slice(path.lastIndexOf(".") + 1)] ?? "tags";

/**
 * The curve editor.
 *
 * ⚠ **M20a drew every row onto one small canvas with a legend** — ▶ **which is a thumbnail**, and
 * a thumbnail belongs wherever a curve is *referenced*. `table-views.ts` keeps that drawing; this is the
 * editor: a channel list, one canvas, and a grid for typing exact numbers.
 */
function curveAsset(): CurveAsset {
  return {
    path: views.curves.path,
    domain: views.curves.domain,
    yLabel: views.curves.yLabel,
    channels: views.curves.rows.map((r) => ({
      name: r.name,
      interpolation: r.interpolation,
      keys: r.keys,
      points: r.points,
      visible: ui.hidden[r.name] !== true,
    })),
  };
}

function curveDocument(): string {
  const a = curveAsset();
  return (
    `<div class="cv-curvewrap">${drawCurveToolbar(a, ui.curve)}` +
    `<div class="cv-curvebody">${drawChannels(a, ui.curve)}` +
    (ui.curve.view === "grid"
      ? drawGrid(a, ui.curve)
      : `<div class="cv-canvas">${drawCanvas(a, ui.curve)}</div>`) +
    `</div></div>`
  );
}

/**
 * What the stage draws for the active tab.
 *
 * ⚠ **The M19 views are documents, not destinations**, and this is where that becomes true: they are
 * reached by opening a file, never by picking from a list.
 */
function stageBody(): string {
  if (ui.asset === "result") {
    // ⚠ **An empty state is a teaching surface** — §9d. It names the control that changes it.
    return (
      `<div class="cv-stage-empty">` +
      `<div class="cv-stage-title">Nothing generated yet</div>` +
      `<p>Press <b>Generate</b> to build a level from this project's content, ` +
      `then step through it at <b>L1</b>–<b>L5</b>.</p>` +
      `</div>`
    );
  }
  const path = ui.asset;
  if (path.endsWith(".cvstate")) {
    return (
      drawStateGraph(views.state) +
      `<pre class="cv-mono cv-${checkTone(views.state)}" ` +
      `style="white-space:pre-wrap;margin:10px 0 0">${checkLine(views.state)}</pre>`
    );
  }
  if (path.endsWith(".cvcurve")) return curveDocument();
  if (path.endsWith(".cvunlock")) return drawUnlockTable(views.unlocks);
  if (path.endsWith(".cvs")) return schematicDocument();
  if (path.endsWith(".cvtags")) return drawTags(SAMPLE_TAGS, ui.tag);
  // ⚠ **Says which milestone owes it**, rather than pretending the surface is broken.
  return (
    `<div class="cv-stage-empty"><div class="cv-stage-title">${kindOf(path)} editor</div>` +
    `<p>Not built yet. <b>${kindOf(path)}</b> gets its editor at ` +
    `${path.endsWith(".cvs") ? "M20e" : path.endsWith(".cvspine") ? "M20g" : "M20f"}.</p></div>`
  );
}

/**
 * What the schematic editor's active document tab shows.
 *
 * ⚠ **`Viewport` and `Setup Graph` were both missing**, and `Objects` was a tab that should never
 * have existed — the schematic's object tree is a **dock**, which is where Unreal puts `Components`.
 */
function schematicDocument(): string {
  if (ui.doc === "viewport") {
    // ⚠ **Honest about what exists.** M17 P03 computed *which components contribute collision*;
    // hulls and a real 3D preview arrive at M26/M27. ▶ The answer we have beats a fake camera.
    return (
      `<div class="cv-viewport"><div class="cv-vpstage">` +
      COMPONENTS.map(
        (c) =>
          `<div class="cv-vpbox${c.collision ? " has-collision" : ""}">${esc(c.name)}` +
          `<span class="cv-vptag">${c.collision ? "collision" : "no collision"}</span></div>`,
      ).join("") +
      `</div><p class="cv-empty">Components, and whether each contributes collision — answered from ` +
      `the palette, never from a class name. <b>The rendered preview arrives with hulls at M26.</b></p></div>`
    );
  }
  if (ui.doc === "setup") {
    // ▶ `_configure` on /Core/Object — the construction script's equivalent, and the reason there
    // is no `BeginPlay`: this runs before generation, not before play.
    const setup: GraphView = {
      path: ["Hookshot", "Setup Graph"],
      nodes: [
        {
          id: "cfg", title: "_configure", context: "Setup - runs before generation", kind: "event",
          x: 70, y: 160, inputs: [],
          outputs: [{ id: "exec", label: "", type: "exec" }],
        },
      ],
      wires: [],
    };
    return `<div class="cv-graphwrap">${graphCrumbs(setup)}<div class="cv-graphscroll">${drawGraph(setup)}</div></div>`;
  }
  return `<div class="cv-graphwrap">${graphCrumbs(HOOK_GRAPH)}<div class="cv-graphscroll">${drawGraph(HOOK_GRAPH)}</div></div>`;
}

const esc = (t: string) =>
  t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

/**
 * Which docks have anything to say right now.
 *
 * ⚠ **§9d: one that does not is not drawn.** ▶ **And there is no Content dock** — the Content
 * Browser is the drawer, and a second surface with the same name was the confusion M20c removed.
 */
function docks(): Dock[] {
  return [
    {
      // ⚠ **Unreal stacks `Components` above `My Blueprint`**, and the split is *what this object is
      // made of* then *what it can be asked*. A schematic with no way to add a component cannot
      // express a mechanic at all — P11 makes the component the mechanic's identity.
      id: "components",
      label: "Components",
      side: "left",
      present: ui.asset.endsWith(".cvs"),
      collapsed: ui.collapsed.components,
      body:
        `<div class="cv-osec"><span>Add</span><button class="cv-oadd" title="Add component">+</button></div>` +
        COMPONENTS.map(
          (c) =>
            `<div class="cv-orow${ui.subject?.classPath === c.path ? " is-selected" : ""}" ` +
            `data-comp="${c.path}" style="padding-left:${c.root ? 8 : 18}px">` +
            `<span>${c.name}</span><span class="cv-otype">${c.kind}</span></div>`,
        ).join(""),
    },
    {
      id: "outline",
      label: "Outline",
      side: "left",
      // ⚠ Nothing generated and no asset open means nothing to outline.
      // ⚠ **§9d, and it generalises.** An Outline is only a *second* place to see structure. A
      // schematic needs one because its graphs, hooks and dials are not on the canvas; a curve's
      // channels and a tag set's tree already are. ▶ **A second copy under a different name is the
      // confusion M20c removed elsewhere**, so the dock is present only where it adds something.
      present: ui.asset.endsWith(".cvs") && !ui.docks.outlineClosed,
      collapsed: ui.collapsed.outline,
      // ⚠ **Scoped to the active document tab** — a hook's locals belong to that hook.
      body: drawOutline(HOOK_GRAPH),
    },
    {
      id: "details",
      label: "Details",
      side: "right",
      present: !ui.docks.detailsClosed,
      collapsed: ui.collapsed.details,
      // ⚠ **Godot's shape, Unreal's grouping** — §9c. The same panel for every surface: it is given a
      // subject, and the subject's class bands and typed rows do the rest.
      body: drawDetails(views.classes, ui.subject, ui.details),
    },
  ];
}

function surface(): Surface {
  return ui.asset === "result"
    ? resultSurface(ui.reached, ui.layer)
    : assetSurface(kindOf(ui.asset), iconOf(ui.asset), ui.asset.endsWith(".cvs"));
}

function render(app: HTMLElement): void {
  const s = surface();
  const width = app.clientWidth || 1400;
  const all = docks();
  const left = all.filter((d) => d.side === "left");
  const right = all.filter((d) => d.side === "right");

  // ⚠ **One browser.** As a drawer it overlays the stage and auto-closes; docked, it sits under it.
  const browser = ui.browser.open
    ? `<div class="cv-drawer${ui.browser.docked ? " is-docked" : ""}">` +
      drawBrowser(SAMPLE_CONTENT, ui.browser, thumbs()) +
      `</div>`
    : "";

  app.innerHTML =
    menuBar(s.menus) +
    assetTabs(ui.tabs, ui.asset) +
    toolbar(s.groups, width - 40) +
    `<div class="cv-frame">` +
    // ⚠ **Left docks stack, they do not sit side by side.** Unreal puts `Components` above
    // `My Blueprint` in one column; two 224px columns is 450px of chrome before the stage starts.
    (left.length ? `<div class="cv-dockcol">${left.map(dock).join("")}</div>` : "") +
    `<div class="cv-centre">` +
    documentTabs(s.documents ?? [], ui.doc) +
    `<div class="cv-stagewrap"><div class="cv-stage">${stageBody()}</div>${browser}</div>` +
    `</div>` +
    right.map(dock).join("") +
    `</div>` +
    statusBar({
      fingerprint: views.fingerprint,
      seed: null,
      unsaved: ui.tabs.filter((t) => t.dirty).length,
    }) +
    (ui.menuOpen
      ? `<div class="cv-menu-anchor" style="left:${ui.menuAt[0]}px;top:${ui.menuAt[1]}px">` +
        `${createMenu()}</div>`
      : "");

  wire(app);
}

/** ▶ **Where the asset can be drawn, draw it** — §9e. The kind icon is the fallback, not the plan. */
function thumbs(): Record<string, string> {
  const out: Record<string, string> = {};
  const curve = views.curves.rows[0];
  if (curve) out["curves/progression.cvcurve"] = curveThumbnail(curve, 76, 44);
  return out;
}

function wire(app: HTMLElement): void {
  const redraw = () => render(app);
  const on = (sel: string, ev: string, f: (el: HTMLElement, e: Event) => void) =>
    app.querySelectorAll<HTMLElement>(sel).forEach((el) =>
      el.addEventListener(ev, (e) => f(el, e)),
    );

  on("[data-doc]", "click", (b) => {
    ui.doc = b.dataset.doc!;
    redraw();
  });
  on("[data-asset]", "click", (b) => {
    ui.asset = b.dataset.asset!;
    ui.subject = ui.asset === "result" ? null : subjectForAsset(ui.asset);
    redraw();
  });

  // ⚠ **The panel does not know which surface it is in** — it is given a subject, and the class bands
  // do the rest. A node and a component both resolve to one.
  on("[data-node]", "click", (n) => {
    const id = n.dataset.node!;
    const node = HOOK_GRAPH.nodes.find((x) => x.id === id);
    if (node) {
      ui.subject = {
        label: node.title,
        icon: "schematic",
        classPath: "/Core/Actor",
        values: {},
        from: { label: "Hookshot.cvs", path: ui.asset },
      };
    }
    redraw();
  });
  // ⚠ **The panel does not know which surface it is in.** A component row and a graph node both
  // resolve to a subject, and the class bands do the rest.
  on("[data-comp]", "click", (r) => {
    const path = r.dataset.comp!;
    const c = COMPONENTS.find((x) => x.path === path)!;
    ui.subject = {
      label: c.name,
      icon: c.root ? "schematic" : "components",
      classPath: path,
      values: {},
      from: { label: ui.asset.split("/").pop()!, path: ui.asset },
    };
    redraw();
  });
  // ⚠ **A key is a thing a developer selects and moves** — a polyline is only a preview.
  on(".cv-key", "click", (k) => {
    ui.curve = { ...ui.curve, selected: { channel: k.dataset.ch!, key: Number(k.dataset.key) } };
    redraw();
  });
  on("[data-channel]", "click", (r) => {
    ui.curve = { ...ui.curve, selected: { channel: r.dataset.channel!, key: 0 } };
    redraw();
  });
  on("[data-vis]", "click", (b, e) => {
    e.stopPropagation();
    const n = b.dataset.vis!;
    ui.hidden = { ...ui.hidden, [n]: !ui.hidden[n] };
    redraw();
  });
  on("[data-view]", "click", (b) => {
    ui.curve = { ...ui.curve, view: b.dataset.view as "curve" | "grid" };
    redraw();
  });
  on("[data-mode]", "click", (b) => {
    // ⚠ **The authored spelling is the one that crosses** — the file's word, never the core's enum.
    const m = b.dataset.mode!;
    const sel = ui.curve.selected;
    if (sel) {
      const row = views.curves.rows.find((r) => r.name === sel.channel);
      if (row) row.interpolation = m;
    }
    redraw();
  });
  on(".cv-chfilter", "input", (el) => {
    ui.curve = { ...ui.curve, filter: (el as HTMLInputElement).value };
    redrawKeepingFocus(app, ".cv-chfilter")();
  });

  on("[data-tag]", "click", (r) => {
    ui.tag = r.dataset.tag!;
    redraw();
  });
  on("[data-band]", "click", (b) => {
    const k = b.dataset.band!;
    ui.details = { ...ui.details, folded: { ...ui.details.folded, [k]: !ui.details.folded[k] } };
    redraw();
  });
  on("[data-adv]", "click", (b) => {
    const k = b.dataset.adv!;
    ui.details = {
      ...ui.details,
      advancedOpen: { ...ui.details.advancedOpen, [k]: !ui.details.advancedOpen[k] },
    };
    redraw();
  });
  on(".cv-dsearch", "input", (el) => {
    ui.details = { ...ui.details, search: (el as HTMLInputElement).value };
    const focus = redrawKeepingFocus(app, ".cv-dsearch");
    focus();
  });
  on("[data-close]", "click", (b, e) => {
    e.stopPropagation();
    const id = b.dataset.close!;
    ui.tabs = ui.tabs.filter((t) => t.id !== id);
    if (ui.asset === id) ui.asset = "result";
    redraw();
  });

  on("[data-tool]", "click", (b) => {
    const id = b.dataset.tool!;
    if (/^L[1-5]$/.test(id)) ui.layer = id;
    else if (id === "add") {
      ui.menuOpen = !ui.menuOpen;
      ui.menuAt = anchorUnder(b);
    }
    else if (id === "generate") {
      // ⚠ **Honest about the stub.** `Project::generate` does not run yet; pretending it did would
      // put a number on screen that nothing computed.
      ui.reached = 0;
    }
    redraw();
  });

  // ⚠ **A dock's own collapse is the only control for its state** — the toolbar toggles that used to
  // duplicate these were removed at M20c, because two controls for one thing disagree.
  on("[data-collapse]", "click", (b) => {
    ui.collapsed[b.dataset.collapse!] = true;
    redraw();
  });
  on("[data-expand]", "click", (b) => {
    ui.collapsed[b.dataset.expand!] = false;
    redraw();
  });

  // ⚠ **The drawer is the browser.** This button opens it; `Dock in Layout` decides where it lives.
  on("[data-drawer]", "click", (b) => {
    if (b.dataset.drawer === "content") ui.browser.open = !ui.browser.open;
    redraw();
  });
  on("[data-dock-browser]", "click", () => {
    ui.browser.docked = !ui.browser.docked;
    redraw();
  });

  // ⚠ **Double-click opens an asset. This is the architecture** — `10-editor.md` §2.
  on("[data-file]", "dblclick", (row) => {
    const path = row.dataset.file!;
    if (!ui.tabs.some((t) => t.id === path)) {
      ui.tabs.push({ id: path, label: path.split("/").pop()!, fact: kindOf(path) });
    }
    ui.asset = path;
    ui.subject = subjectForAsset(path);
    // ▶ A drawer auto-minimises when it loses focus — Unreal's, and the reason it costs nothing.
    if (!ui.browser.docked) ui.browser.open = false;
    redraw();
  });
  on("[data-folder]", "click", (row) => {
    ui.browser = { ...ui.browser, folder: row.dataset.folder! };
    redraw();
  });
  on(".cv-chip", "click", (chip) => {
    const kind = chip.dataset.kind!;
    const has = ui.browser.kinds.includes(kind);
    ui.browser = {
      ...ui.browser,
      kinds: has ? ui.browser.kinds.filter((k) => k !== kind) : [...ui.browser.kinds, kind],
    };
    redraw();
  });
  on(".cv-search", "input", (el) => {
    ui.browser = { ...ui.browser, search: (el as HTMLInputElement).value };
    const focus = redrawKeepingFocus(app);
    focus();
  });

  // ⚠ **One menu, two doors** — `⊕ Add` and the browser's right-click open the same thing.
  on("[data-open-create]", "click", (b) => {
    ui.menuOpen = !ui.menuOpen;
    ui.menuAt = anchorUnder(b);
    redraw();
  });
  on(".cv-tiles", "contextmenu", (_el, e) => {
    e.preventDefault();
    const m = e as MouseEvent;
    ui.menuOpen = true;
    ui.menuAt = [m.clientX, m.clientY];
    redraw();
  });
  on("[data-create]", "click", () => {
    // ▶ Creation makes an **empty** asset; M20c P05b's modal and the write path arrive with the
    // service work. ⚠ Recorded rather than faked: nothing here writes a file yet.
    ui.menuOpen = false;
    redraw();
  });
}

/** ⚠ A schematic describes an `Item`; the panel bands its ancestry from the manifest artifact. */
function subjectForAsset(path: string): Subject {
  const base: Subject = {
    label: path.split("/").pop()!,
    icon: iconOf(path),
    classPath: path.endsWith(".cvs") ? "/Core/Item" : `${kindOf(path)} asset`,
    values: {},
  };
  // ⚠ **A `.cvcurve` carries `domain` and `y_label`, and the editor had nowhere to put them.**
  // ▶ They are asset properties, and they belong in the same panel everything else uses.
  if (path.endsWith(".cvcurve")) {
    return {
      ...base,
      assetBand: {
        name: "Curve table",
        fields: [
          { name: "domain", type: "String", exposed: true, mutable: true, api: true,
            doc: "What the x axis measures." },
          { name: "y_label", type: "String", exposed: true, mutable: true, api: true,
            doc: "What the y axis measures." },
          { name: "curves", type: "int", exposed: true, mutable: false, api: true,
            doc: "How many curves this table holds." },
        ],
      },
      values: {
        domain: views.curves.domain,
        y_label: views.curves.yLabel,
        curves: String(views.curves.rows.length),
      },
    };
  }
  return base;
}

/** Where a popup opened from `el` should sit. ⚠ Kept on screen — a menu off the right edge is lost. */
function anchorUnder(el: HTMLElement): [number, number] {
  const r = el.getBoundingClientRect();
  return [Math.min(r.left, Math.max(8, innerWidth - 320)), r.bottom + 4];
}

/** Re-render without losing the caret in the search box. ⚠ A field that blurs on every keystroke is unusable. */
function redrawKeepingFocus(app: HTMLElement, sel = ".cv-search"): () => void {
  const box = app.querySelector<HTMLInputElement>(sel);
  const at = box?.selectionStart ?? null;
  render(app);
  return () => {
    const next = app.querySelector<HTMLInputElement>(sel);
    if (next) {
      next.focus();
      if (at !== null) next.setSelectionRange(at, at);
    }
  };
}

async function main(): Promise<void> {
  const app = document.querySelector<HTMLElement>("#app");
  if (!app) return;
  install();

  const style = document.createElement("style");
  style.textContent = shellStyles() + frameStyles() + browserStyles() + graphStyles() + detailsStyles() + curveStyles() + tagStyles() + extraStyles();
  document.head.appendChild(style);

  let version = "";
  try {
    version = (await (await fetch("/api/version")).json()).version;
  } catch {
    app.innerHTML = `<div class="cv-empty">the editor service is not running — \`npm run serve\`</div>`;
    return;
  }

  const state = await post<StateGraphView>("/api/state", { rel: "", text: WATER });
  const curves = await post<CurveTableView>("/api/curves", {
    path: "/Content/Curves/progression.cvcurve",
    text: CURVES,
  });
  const unlocks = await post<UnlockTableView>("/api/unlocks", { text: UNLOCKS });

  // ⚠ **"No project open" is not "no dials"** — the API distinguishes them with a 409 and the view
  // must not collapse that back.
  const dialsRes = await fetch("/api/dials");
  const noProject = dialsRes.status === 409;
  const dials: DialRowView[] = noProject ? [] : ((await dialsRes.json()).dials ?? []);

  // ⚠ **The panel is driven by the manifest artifact, not by a hand-written schema** — M17 P02.
  let classes: ClassDef[] = [];
  try {
    classes = (await (await fetch("/classes.json")).json()).classes ?? [];
  } catch {
    classes = [];
  }
  views = { state, curves, unlocks, dials, noProject, version, fingerprint: "—", classes };

  ui = {
    asset: "result",
    tabs: [{ id: "result", label: "Untitled", fixed: true, fact: version }],
    layer: "L1",
    reached: 0,
    // ⚠ **§9d depth 0**: the stage, the layer switcher, Generate, Content and Details. Nothing else.
    docks: {},
    collapsed: {},
    browser: { open: true, docked: true, kinds: [], folder: "", search: "" },
    menuOpen: false,
    menuAt: [8, 108],
    details: { search: "", folded: {}, advancedOpen: {} },
    curve: { filter: "", selected: null, view: "curve" },
    hidden: {},
    subject: null,
    doc: "grants",
  };

  render(app);
  addEventListener("resize", () => render(app));
}

/** Styles the frame does not own: the stage's empty state and the Details subject line. */
function extraStyles(): string {
  return `
.cv-stagewrap { position: relative; display: flex; flex-direction: column; flex: 1 1 auto;
  min-height: 0; }
/* ⚠ **A drawer overlays the stage; docked, it sits under it.** One surface, two placements. */
.cv-drawer { position: absolute; left: 0; right: 0; bottom: 0; height: 262px; z-index: 20;
  background: var(--cv-panel); border-top: 1px solid var(--cv-line);
  box-shadow: 0 -10px 26px rgb(0 0 0 / .38); }
.cv-drawer.is-docked { position: relative; height: 248px; box-shadow: none; flex: 0 0 auto; }
.cv-menu-anchor { position: fixed; z-index: 60; }
.cv-stage { display: block; overflow: auto; min-height: 0; padding: 18px; background: var(--cv-bg); }
.cv-stage-empty { max-width: 52ch; margin: 8vh auto 0; text-align: center; color: var(--cv-muted); }
.cv-stage-title { color: var(--cv-text); font-size: 15px; margin-bottom: 6px; }
.cv-stage-empty b { color: var(--cv-text); }
.cv-viewport { padding: 18px; }
.cv-vpstage { display: flex; gap: 14px; align-items: flex-end; justify-content: center;
  padding: 26px 18px; background: var(--cv-panel); border: 1px solid var(--cv-line);
  border-radius: var(--cv-radius); margin-bottom: 12px; }
.cv-vpbox { position: relative; min-width: 118px; padding: 26px 12px 10px; text-align: center;
  font-size: 11.5px; color: var(--cv-text); background: var(--cv-raised);
  border: 1px dashed var(--cv-line); border-radius: var(--cv-radius); }
.cv-vpbox.has-collision { border-style: solid; border-color: var(--cv-accent); }
.cv-vptag { display: block; margin-top: 5px; font-size: 9px; letter-spacing: .06em;
  text-transform: uppercase; color: var(--cv-muted); }
.cv-vpbox.has-collision .cv-vptag { color: var(--cv-accent); }
.cv-subject { display: flex; align-items: center; gap: 7px; color: var(--cv-text); font-size: 13px;
  padding: 4px 6px 8px; border-bottom: 1px solid var(--cv-line); margin-bottom: 6px; }
`;
}

void main();

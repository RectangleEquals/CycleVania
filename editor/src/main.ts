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
import { drawGraph, drawOutline, graphCrumbs, graphStyles, type GraphView } from "./graph.ts";
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
 * A schematic's `OnPickup` hook, drawn.
 *
 * ⚠ **Shaped by M18's rules, not invented.** The dial get-node is pure and carries the dial's real
 * type; `Kind<T>` shows a picker inline; the hook node is an event with an exec out and no return.
 * ▶ **`Event Tick` is pre-placed and disabled** — M17 P04's `OVERRIDES` rule drawn: every hook in
 * the ancestry, each saying what happens if you leave it alone.
 */
const HOOK_GRAPH: GraphView = {
  path: ["Hookshot", "OnPickup"],
  nodes: [
    {
      id: "ev", title: "On Pickup", context: "Hook", kind: "event", x: 40, y: 120,
      inputs: [],
      outputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "actor", label: "Actor", type: "Ref<Actor>", connected: true },
      ],
    },
    {
      id: "dial", title: "rope_length", context: "Dial on Hookshot", kind: "pure", x: 40, y: 250,
      inputs: [],
      outputs: [{ id: "v", label: "Value", type: "float", connected: true }],
    },
    {
      id: "grant", title: "Grant", context: "Target is Actor", kind: "call", x: 300, y: 116,
      inputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "target", label: "Target", type: "Ref<Actor>", connected: true },
        { id: "unlock", label: "Unlock", type: "Unlock", inline: "Grapple" },
      ],
      outputs: [{ id: "exec", label: "", type: "exec", connected: true }],
      selected: true,
    },
    {
      id: "reach", title: "Set Reach", context: "Target is Hookshot", kind: "call", x: 560, y: 116,
      inputs: [
        { id: "exec", label: "", type: "exec", connected: true },
        { id: "len", label: "Length", type: "float", connected: true },
        { id: "tags", label: "Tags", type: "String", list: true },
      ],
      outputs: [{ id: "exec", label: "", type: "exec" }],
      latent: true,
      advanced: 2,
    },
    {
      id: "tick", title: "Event Tick", context: "Hook", kind: "event", x: 300, y: 330,
      inputs: [],
      outputs: [
        { id: "exec", label: "", type: "exec" },
        { id: "dt", label: "Delta Seconds", type: "float" },
      ],
      disabled: true,
    },
  ],
  wires: [
    { from: { node: "ev", pin: "exec" }, to: { node: "grant", pin: "exec" } },
    { from: { node: "ev", pin: "actor" }, to: { node: "grant", pin: "target" } },
    { from: { node: "grant", pin: "exec" }, to: { node: "reach", pin: "exec" } },
    { from: { node: "dial", pin: "v" }, to: { node: "reach", pin: "len" } },
  ],
};

/** Stand-in content, until a project is open. ⚠ Shaped like a real content root, not invented. */
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
  if (path.endsWith(".cvcurve")) {
    return (
      drawCurveTable(views.curves, 560, 230, { row: "complexity", key: 1 }) +
      `<div style="display:flex;gap:14px;align-items:center;margin-top:10px">` +
      views.curves.rows
        .map(
          (r) =>
            `<span style="display:flex;align-items:center;gap:6px;font-size:11px" class="cv-dim">` +
            `${curveThumbnail(r)}${r.name}</span>`,
        )
        .join("") +
      `</div>`
    );
  }
  if (path.endsWith(".cvunlock")) return drawUnlockTable(views.unlocks);
  if (path.endsWith(".cvs")) {
    return (
      `<div class="cv-graphwrap">${graphCrumbs(HOOK_GRAPH)}${drawGraph(HOOK_GRAPH)}</div>`
    );
  }
  // ⚠ **Says which milestone owes it**, rather than pretending the surface is broken.
  return (
    `<div class="cv-stage-empty"><div class="cv-stage-title">${kindOf(path)} editor</div>` +
    `<p>Not built yet. <b>${kindOf(path)}</b> gets its editor at ` +
    `${path.endsWith(".cvs") ? "M20e" : path.endsWith(".cvspine") ? "M20g" : "M20f"}.</p></div>`
  );
}

/**
 * Which docks have anything to say right now.
 *
 * ⚠ **§9d: one that does not is not drawn.** ▶ **And there is no Content dock** — the Content
 * Browser is the drawer, and a second surface with the same name was the confusion M20c removed.
 */
function docks(): Dock[] {
  return [
    {
      id: "outline",
      label: "Outline",
      side: "left",
      // ⚠ Nothing generated and no asset open means nothing to outline.
      present: ui.asset !== "result" && !ui.docks.outlineClosed,
      collapsed: ui.collapsed.outline,
      // ⚠ **Scoped to the active document tab** — a hook's locals belong to that hook.
      body: ui.asset.endsWith(".cvs")
        ? drawOutline(HOOK_GRAPH)
        : `<div class="cv-empty">${kindOf(ui.asset)} — its outline arrives with its editor.</div>`,
    },
    {
      id: "details",
      label: "Details",
      side: "right",
      present: !ui.docks.detailsClosed,
      collapsed: ui.collapsed.details,
      // ⚠ **A panel that does not say what it describes cannot be trusted** — §9c.
      body:
        ui.asset === "result"
          ? `<div class="cv-empty">Select something to see its details.</div>`
          : `<div class="cv-subject">${iconForPath(ui.asset, 14)}` +
            `<span>${ui.asset.split("/").pop()}</span></div>` +
            `<div class="cv-empty">Typed rows arrive at M20d.</div>`,
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
    left.map(dock).join("") +
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

  on("[data-asset]", "click", (b) => {
    ui.asset = b.dataset.asset!;
    redraw();
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

/** Where a popup opened from `el` should sit. ⚠ Kept on screen — a menu off the right edge is lost. */
function anchorUnder(el: HTMLElement): [number, number] {
  const r = el.getBoundingClientRect();
  return [Math.min(r.left, Math.max(8, innerWidth - 320)), r.bottom + 4];
}

/** Re-render without losing the caret in the search box. ⚠ A field that blurs on every keystroke is unusable. */
function redrawKeepingFocus(app: HTMLElement): () => void {
  const box = app.querySelector<HTMLInputElement>(".cv-search");
  const at = box?.selectionStart ?? null;
  render(app);
  return () => {
    const next = app.querySelector<HTMLInputElement>(".cv-search");
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
  style.textContent = shellStyles() + frameStyles() + browserStyles() + graphStyles() + extraStyles();
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

  views = { state, curves, unlocks, dials, noProject, version, fingerprint: "—" };

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
    doc: "onpickup",
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
.cv-subject { display: flex; align-items: center; gap: 7px; color: var(--cv-text); font-size: 13px;
  padding: 4px 6px 8px; border-bottom: 1px solid var(--cv-line); margin-bottom: 6px; }
`;
}

void main();

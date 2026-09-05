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
import { drawContent, shellStyles, type ContentFilter } from "./shell.ts";
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
  filter: ContentFilter;
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
  // ⚠ **Says which milestone owes it**, rather than pretending the surface is broken.
  return (
    `<div class="cv-stage-empty"><div class="cv-stage-title">${kindOf(path)} editor</div>` +
    `<p>Not built yet. <b>${kindOf(path)}</b> gets its editor at ` +
    `${path.endsWith(".cvs") ? "M20e" : path.endsWith(".cvspine") ? "M20g" : "M20f"}.</p></div>`
  );
}

/** Which docks have anything to say right now. ⚠ §9d: one that does not is not drawn. */
function docks(): Dock[] {
  const dials = views.noProject
    ? `<p class="cv-empty">No project is open, so there are no dials to turn. Open one to see them.</p>`
    : drawDials(views.dials);
  return [
    {
      id: "outline",
      label: "Outline",
      side: "left",
      // ⚠ Nothing generated and no asset open means nothing to outline.
      present: !!ui.docks.outline && ui.asset !== "result",
      collapsed: ui.collapsed.outline,
      body: `<div class="cv-empty">${kindOf(ui.asset)} — its objects arrive with its editor.</div>`,
    },
    {
      id: "details",
      label: "Details",
      side: "right",
      present: !!ui.docks.details,
      collapsed: ui.collapsed.details,
      // ⚠ **A panel that does not say what it describes cannot be trusted** — §9c.
      body:
        ui.asset === "result"
          ? `<div class="cv-empty">Select something to see its details.</div>`
          : `<div class="cv-subject">${ui.asset.split("/").pop()}</div>` +
            `<div class="cv-empty">Typed rows arrive at M20d.</div>` +
            (views.noProject ? "" : `<div class="cv-h">Dials</div>${dials}`),
    },
    {
      id: "content",
      label: "Content",
      side: "bottom",
      present: !!ui.docks.content,
      collapsed: ui.collapsed.content,
      // ⚠ **Scaffolding.** M20c replaces this list with the drawer, sources tree and tiles.
      body: drawContent(SAMPLE_CONTENT, ui.filter),
    },
  ];
}

function surface(): Surface {
  return ui.asset === "result"
    ? resultSurface(ui.reached, ui.layer, ui.docks)
    : assetSurface(kindOf(ui.asset), ui.docks);
}

function render(app: HTMLElement): void {
  const s = surface();
  const width = app.clientWidth || 1400;
  const left = docks().filter((d) => d.side === "left");
  const right = docks().filter((d) => d.side === "right");
  const bottom = docks().filter((d) => d.side === "bottom");

  app.innerHTML =
    menuBar(s.menus) +
    assetTabs(ui.tabs, ui.asset) +
    toolbar(s.groups, width - 40) +
    `<div class="cv-frame">` +
    left.map(dock).join("") +
    `<div class="cv-centre">` +
    documentTabs(s.documents ?? [], "") +
    `<div class="cv-stage">${stageBody()}</div>` +
    bottom.map(dock).join("") +
    `</div>` +
    right.map(dock).join("") +
    `</div>` +
    statusBar({
      fingerprint: views.fingerprint,
      seed: null,
      unsaved: ui.tabs.filter((t) => t.dirty).length,
    });

  wire(app);
}

function wire(app: HTMLElement): void {
  const redraw = () => render(app);

  app.querySelectorAll<HTMLElement>("[data-asset]").forEach((b) =>
    b.addEventListener("click", () => {
      ui.asset = b.dataset.asset!;
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-close]").forEach((b) =>
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      const id = b.dataset.close!;
      ui.tabs = ui.tabs.filter((t) => t.id !== id);
      if (ui.asset === id) ui.asset = "result";
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-tool]").forEach((b) =>
    b.addEventListener("click", () => {
      const id = b.dataset.tool!;
      if (id.startsWith("dock:")) {
        const which = id.slice(5);
        ui.docks[which] = !ui.docks[which];
      } else if (/^L[1-5]$/.test(id)) {
        ui.layer = id;
      } else if (id === "generate") {
        // ⚠ **Honest about the stub.** `Project::generate` does not run yet; pretending it did would
        // put a number on screen that nothing computed.
        ui.reached = 0;
      }
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-collapse]").forEach((b) =>
    b.addEventListener("click", () => {
      ui.collapsed[b.dataset.collapse!] = true;
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-expand]").forEach((b) =>
    b.addEventListener("click", () => {
      ui.collapsed[b.dataset.expand!] = false;
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-drawer]").forEach((b) =>
    b.addEventListener("click", () => {
      if (b.dataset.drawer === "content") ui.docks.content = !ui.docks.content;
      redraw();
    }),
  );

  // ⚠ **Double-click opens an asset. This is the architecture** — §2 — and M20c gives it a real browser.
  app.querySelectorAll<HTMLElement>("[data-file]").forEach((row) =>
    row.addEventListener("dblclick", () => {
      const path = row.dataset.file!;
      if (!ui.tabs.some((t) => t.id === path)) {
        ui.tabs.push({ id: path, label: path.split("/").pop()!, fact: kindOf(path) });
      }
      ui.asset = path;
      ui.docks.outline = true;
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>("[data-folder]").forEach((row) =>
    row.addEventListener("click", () => {
      ui.filter = { ...ui.filter, folder: row.dataset.folder! };
      redraw();
    }),
  );
  app.querySelectorAll<HTMLElement>(".cv-chip").forEach((chip) =>
    chip.addEventListener("click", () => {
      const kind = chip.dataset.kind!;
      const on = ui.filter.kinds.includes(kind);
      ui.filter = {
        ...ui.filter,
        kinds: on ? ui.filter.kinds.filter((k) => k !== kind) : [...ui.filter.kinds, kind],
      };
      redraw();
    }),
  );
}

async function main(): Promise<void> {
  const app = document.querySelector<HTMLElement>("#app");
  if (!app) return;
  install();

  const style = document.createElement("style");
  style.textContent = shellStyles() + frameStyles() + extraStyles();
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
    docks: { content: true, details: true, outline: false },
    collapsed: {},
    filter: { kinds: [], folder: "", search: "" },
  };

  render(app);
  addEventListener("resize", () => render(app));
}

/** Styles the frame does not own: the stage's empty state and the Details subject line. */
function extraStyles(): string {
  return `
.cv-stage { display: block; overflow: auto; min-height: 0; padding: 18px; background: var(--cv-bg); }
.cv-stage-empty { max-width: 52ch; margin: 8vh auto 0; text-align: center; color: var(--cv-muted); }
.cv-stage-title { color: var(--cv-text); font-size: 15px; margin-bottom: 6px; }
.cv-stage-empty b { color: var(--cv-text); }
.cv-subject { color: var(--cv-text); font-size: 13px; padding: 4px 6px 8px;
  border-bottom: 1px solid var(--cv-line); margin-bottom: 6px; }
`;
}

void main();

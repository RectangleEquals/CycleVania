/**
 * The editor's browser half.
 *
 * ⚠ **A shell, not a docking system.** `10-editor.md` §10 still owes panel arrangement and how nine
 * views share one window; those want mockups. ▶ A fixed topbar-navigator-stage-inspector arrangement
 * that looks like an editor beats a white page while mockups are pending, and commits to nothing a
 * docking system would have to undo.
 *
 * ⚠ **Every colour comes from the four theme parameters.** Nothing here picks a hex.
 */

import { install } from "./theme.ts";
import { drawContent, navigator, shellStyles, topbar, type ContentFilter, type NavGroup } from "./shell.ts";
import { checkLine, drawStateGraph, type StateGraphView } from "./state-view.ts";
import {
  FloorSlider,
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

function card(title: string, note: string, body: string): string {
  return `<div class="cv-card"><h3>${title}</h3><p class="cv-note">${note}</p>${body}</div>`;
}

async function main(): Promise<void> {
  const app = document.querySelector<HTMLElement>("#app");
  if (!app) return;
  install();

  const style = document.createElement("style");
  style.textContent = shellStyles();
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
  const dials: DialRowView[] = dialsRes.status === 409 ? [] : ((await dialsRes.json()).dials ?? []);
  const noProject = dialsRes.status === 409;

  const groups: NavGroup[] = [
    {
      title: "Views",
      items: [
        { id: "content", label: "Content", hint: `${SAMPLE_CONTENT.length}` },
        { id: "state", label: "State graph", hint: state.variable },
        { id: "curves", label: "Curve editor", hint: `${curves.rows.length}` },
        { id: "unlocks", label: "Unlock table", hint: `${unlocks.rows.length}` },
        { id: "dials", label: "Dials", hint: noProject ? "—" : `${dials.length}` },
      ],
    },
    {
      title: "Not yet built",
      items: [
        { id: "schematic", label: "Schematic editor", hint: "M16" },
        { id: "mission", label: "Mission graph", hint: "M24" },
        { id: "skeleton", label: "Skeleton", hint: "M26" },
        { id: "trace", label: "Trace", hint: "M21" },
      ],
    },
  ];

  const filter: ContentFilter = { kinds: [], folder: "", search: "" };
  const floor = new FloorSlider();
  floor.set(2);

  app.innerHTML =
    topbar(version, ["Editor", "Play"], "Editor") +
    `<div class="cv-body">` +
    `<div class="cv-nav">${navigator(groups, "state")}</div>` +
    `<div class="cv-stage">` +
    card(
      "State graph",
      "Nodes are settings of a variable; edges are transitions with what they cost. The check is the core's.",
      drawStateGraph(state) +
        `<pre class="cv-mono ${state.satisfiesP15 ? "cv-ok" : "cv-err"}" ` +
        `style="white-space:pre-wrap;margin:10px 0 0">${checkLine(state)}</pre>`,
    ) +
    card(
      "Curve editor",
      "Keys are objects, not samples — select one to see its tangent handles. Per-row interpolation is in the format.",
      drawCurveTable(curves, 460, 190, { row: "complexity", key: 1 }) +
        `<div style="display:flex;gap:14px;align-items:center;margin-top:10px">` +
        curves.rows
          .map(
            (r) =>
              `<span style="display:flex;align-items:center;gap:6px;font-size:11px" class="cv-dim">` +
              `${curveThumbnail(r)}${r.name}</span>`,
          )
          .join("") +
        `</div>`,
    ) +
    card(
      "Unlock table",
      "id is a read-only column. A supersedes cycle is shown on the rows that form it, not deferred to a build error.",
      drawUnlockTable(unlocks),
    ) +
    card(
      "Dials",
      "project.dials, with no state of its own — the same list/get/set a host calls.",
      noProject
        ? `<p class="cv-empty">No project is open, so there are no dials to turn. Open one to see them.</p>`
        : drawDials(dials),
    ) +
    `</div>` +
    `<div class="cv-inspector">` +
    `<div class="cv-h">Content</div>` +
    drawContent(SAMPLE_CONTENT, filter) +
    `<div class="cv-h" style="margin-top:14px">Floor</div>` +
    `<input id="cv-floor" class="cv-search" type="range" min="0" max="6" value="${floor.floor}" style="padding:0"/>` +
    `<div id="cv-floor-read" class="cv-dim" style="font:11px ui-monospace,monospace;margin-top:4px">band ${floor.floor}</div>` +
    `</div></div>`;

  // ⚠ Interaction, so the states are real rather than drawn: the navigator selects, filters stack.
  app.querySelectorAll<HTMLElement>(".cv-nav .cv-row").forEach((row) =>
    row.addEventListener("click", () => {
      app.querySelectorAll(".cv-nav .cv-row").forEach((r) => r.classList.remove("is-selected"));
      row.classList.add("is-selected");
    }),
  );
  app.querySelectorAll<HTMLElement>(".cv-chip").forEach((chip) =>
    chip.addEventListener("click", () => chip.classList.toggle("is-on")),
  );

  // ⚠ **The readout was rendered once and never moved.** The slider sat at 2 while the label said
  // 0 — a control that disagrees with its own value is worse than no readout, because it is believed.
  const slider = app.querySelector<HTMLInputElement>("#cv-floor");
  const readout = app.querySelector<HTMLElement>("#cv-floor-read");
  floor.onChange((f) => {
    if (readout) readout.textContent = `band ${f}`;
  });
  slider?.addEventListener("input", () => floor.set(Number(slider.value)));
}

void main();

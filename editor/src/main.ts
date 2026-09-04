/**
 * The editor's browser half.
 *
 * ⚠ **Views, not a layout.** Each panel below draws what `10-editor.md` §2 says must be visible. Where
 * they sit — docking, navigation between the nine — is §10's mockups, which have not happened, so this
 * stacks them and says so rather than inventing an arrangement nobody reviewed.
 */

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

async function post<T>(route: string, payload: unknown): Promise<T> {
  const res = await fetch(route, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  return (await res.json()) as T;
}

function panel(title: string, note: string): HTMLElement {
  const el = document.createElement("section");
  el.style.cssText = "margin:0 0 30px";
  el.innerHTML =
    `<h2 style="font:600 12px ui-sans-serif,system-ui;letter-spacing:.06em;text-transform:uppercase;` +
    `color:#555;margin:0 0 2px">${title}</h2>` +
    `<p style="font:12px ui-sans-serif,system-ui;color:#8a8a8a;margin:0 0 10px;max-width:62ch">${note}</p>`;
  return el;
}

function boxed(html: string): HTMLElement {
  const d = document.createElement("div");
  d.style.cssText = "border:1px solid #e6e6e6;border-radius:8px;padding:10px;background:#fff";
  d.innerHTML = html;
  return d;
}

async function main(): Promise<void> {
  const app = document.querySelector("#app");
  if (!app) return;
  app.textContent = "";
  Object.assign((app as HTMLElement).style, {
    font: "14px ui-sans-serif, system-ui, sans-serif",
    color: "#222",
    padding: "24px 28px",
    maxWidth: "820px",
  });

  const version = document.createElement("p");
  version.style.cssText = "font:12px ui-monospace,monospace;color:#999;margin:0 0 4px";
  app.appendChild(version);

  const caveat = document.createElement("p");
  caveat.style.cssText = "font:12px ui-sans-serif,system-ui;color:#b8860b;margin:0 0 24px";
  caveat.textContent =
    "Views, stacked. Panel arrangement and navigation are still waiting on mockups (10-editor §10).";
  app.appendChild(caveat);

  try {
    version.textContent = (await (await fetch("/api/version")).json()).version;
  } catch {
    version.textContent = "the editor service is not running — `npm run serve`";
    return;
  }

  // --- State graph ------------------------------------------------------------------------
  const state = panel(
    "State graph",
    "Nodes are settings of a variable; edges are transitions with what they cost. The check is the core's.",
  );
  const view = await post<StateGraphView>("/api/state", { rel: "", text: WATER });
  state.appendChild(boxed(drawStateGraph(view)));
  const line = document.createElement("pre");
  line.style.cssText = `font:12px ui-monospace,monospace;white-space:pre-wrap;margin:10px 0 0;color:${
    view.satisfiesP15 ? "#2f6f3e" : "#b4341f"
  }`;
  line.textContent = checkLine(view);
  state.appendChild(line);
  app.appendChild(state);

  // --- Curve editor -----------------------------------------------------------------------
  const curves = panel(
    "Curve editor",
    "Rows are sampled by the core, each scaled to its own extent — a shared scale would flatten a small row beside a large one, and flat is what a broken curve looks like.",
  );
  const table = await post<CurveTableView>("/api/curves", {
    path: "/Content/Curves/progression.cvcurve",
    text: CURVES,
  });
  curves.appendChild(boxed(drawCurveTable(table)));
  const thumbs = document.createElement("div");
  thumbs.style.cssText = "display:flex;gap:14px;align-items:center;margin-top:10px";
  thumbs.innerHTML = table.rows
    .map(
      (r) =>
        `<span style="display:flex;align-items:center;gap:6px;font:11px ui-sans-serif,system-ui;color:#666">` +
        `${curveThumbnail(r)}${r.name}</span>`,
    )
    .join("");
  curves.appendChild(thumbs);
  app.appendChild(curves);

  // --- Unlock table -----------------------------------------------------------------------
  const unlocks = panel(
    "Unlock table",
    "id is a read-only column, not a per-row rule. A supersedes cycle is shown here, on the rows that form it — not deferred to a build error.",
  );
  unlocks.appendChild(boxed(drawUnlockTable(await post<UnlockTableView>("/api/unlocks", { text: UNLOCKS }))));
  app.appendChild(unlocks);

  // --- Dials ------------------------------------------------------------------------------
  const dials = panel(
    "Dials",
    "project.dials, with no state of its own — the same list/get/set a host calls. default and effective both, because neither is derivable from the other.",
  );
  // ⚠ **"No project open" and "an open project with no dials" are different states**, and the API
  // says so with a 409. A first draft caught that and returned `[]`, which put *"this project declares
  // no dials"* on screen when no project was open — flattening the distinction the binding took care
  // to make, and logging a console error on every load for good measure.
  const res = await fetch("/api/dials");
  if (res.status === 409) {
    dials.appendChild(
      boxed(
        `<p style="font:13px ui-sans-serif,system-ui;color:#8a8a8a;margin:0">` +
          `No project is open, so there are no dials to turn. Open one to see them.</p>`,
      ),
    );
  } else {
    const shown: DialRowView[] = (await res.json()).dials ?? [];
    dials.appendChild(boxed(drawDials(shown)));
  }
  app.appendChild(dials);

  // --- Floor slider -----------------------------------------------------------------------
  const floor = new FloorSlider();
  const fp = panel(
    "Floor",
    "Shared state from the start, because M20 links it to the skeleton. Lifting it later would mean finding every place that read a private copy.",
  );
  const readout = document.createElement("span");
  readout.style.cssText = "font:12px ui-monospace,monospace;color:#555;margin-left:10px";
  floor.onChange((f) => (readout.textContent = `floor ${f}`));
  const input = document.createElement("input");
  input.type = "range";
  input.min = "0";
  input.max = "6";
  input.value = "0";
  input.addEventListener("input", () => floor.set(Number(input.value)));
  floor.set(2);
  input.value = "2";
  const row = document.createElement("div");
  row.style.cssText = "display:flex;align-items:center";
  row.append(input, readout);
  fp.appendChild(boxed(row.outerHTML));
  app.appendChild(fp);
}

void main();

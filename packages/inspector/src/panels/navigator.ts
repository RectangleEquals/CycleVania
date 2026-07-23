/** Left navigator — Reaches + the active Reach's Regions, plus the Sphere slider. */

import { store, isRegionSelected } from "../state.js";
import { activeReach } from "../views/mission.js";
import { el, clear } from "../dom.js";

export function renderNavigator(host: HTMLElement): void {
  clear(host);
  const { reaches, view, selection, sphereStep } = store.state;

  host.append(el("h3", {}, "Reaches"));
  if (reaches.length === 0) host.append(el("div", { class: "hint" }, "generating…"));
  for (const r of reaches) {
    const sel = selection.kind === "reach" && selection.reachIndex === r.meta.reachIndex;
    const row = el("div", { class: `row${sel ? " sel" : ""}` }, `Reach ${r.meta.reachIndex} `, el("span", { class: "dim" }, `(${r.graph.regions.length}r)`));
    row.addEventListener("click", () => store.select({ kind: "reach", reachIndex: r.meta.reachIndex }));
    host.append(row);
  }

  const reach = activeReach();
  if (!reach) return;

  if (view === "mission") {
    host.append(el("h3", {}, "Sphere ladder"));
    const wrap = el("div", { style: "display:flex; align-items:center; gap:6px; margin-bottom:8px" });
    const input = el("input", { type: "range", min: "-1", max: String(reach.meta.spheres.length - 1), value: String(sphereStep), style: "flex:1" }) as HTMLInputElement;
    input.addEventListener("input", () => store.set({ sphereStep: Number(input.value) }));
    wrap.append(input);
    wrap.append(el("span", { class: "dim" }, sphereStep < 0 ? "all" : `≤ ${sphereStep}`));
    host.append(wrap);
  }

  host.append(el("h3", {}, "Regions"));
  for (const rg of reach.graph.regions) {
    const sel = isRegionSelected(reach.meta.reachIndex, rg.id);
    const row = el("div", { class: `row${sel ? " sel" : ""}` }, rg.id, " ", el("span", { class: "dim" }, rg.role));
    row.addEventListener("click", () => store.select({ kind: "region", reachIndex: reach.meta.reachIndex, regionId: rg.id }));
    host.append(row);
  }
}

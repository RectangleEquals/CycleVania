/** Schedule view — placed gadgets/puzzles per Reach (+ virtual plan when bounded). */

import { store } from "../state.js";
import { el, clear } from "../dom.js";

function track(title: string, perReach: { reachIndex: number; entries: string[] }[], plan: Map<string, number>): HTMLElement {
  const wrap = el("div", { style: "margin-bottom:18px" });
  wrap.append(el("h3", {}, title));
  const row = el("div", { style: "display:flex; gap:12px; align-items:flex-start" });
  for (const col of perReach) {
    const c = el("div", { style: "min-width:130px; border:1px solid var(--line); border-radius:6px; padding:6px; background:var(--panel)" });
    c.append(el("div", { style: "color:var(--muted); font-size:11px; margin-bottom:4px" }, `Reach ${col.reachIndex}`));
    if (col.entries.length === 0) c.append(el("div", { class: "hint" }, "—"));
    for (const id of col.entries) {
      const planned = plan.get(id);
      const chip = el(
        "div",
        { style: "background:var(--panel-2); border:1px solid var(--line); border-radius:4px; padding:2px 6px; margin:2px 0; font-size:12px", title: planned !== undefined ? `virtual plan: reach ${planned}` : "no virtual plan (unbounded World)" },
        id,
      );
      if (planned !== undefined && planned !== col.reachIndex) chip.style.borderColor = "var(--warn)";
      c.append(chip);
    }
    row.append(c);
  }
  wrap.append(row);
  return wrap;
}

export function renderSchedule(host: HTMLElement): void {
  clear(host);
  const { reaches, world } = store.state;
  const scroll = el("div", { style: "position:absolute; inset:0; overflow:auto; padding:16px" });

  if (reaches.length === 0) {
    scroll.append(el("div", { class: "hint" }, "No reaches generated."));
    host.append(scroll);
    return;
  }

  const gadgetPlan = world ? world.virtualSchedule("gadgets") : new Map<string, number>();
  const puzzlePlan = world ? world.virtualSchedule("puzzles") : new Map<string, number>();

  const gadgetCols = reaches.map((r) => ({ reachIndex: r.meta.reachIndex, entries: r.items.filter((i) => i.class === "progression").map((i) => i.id) }));
  const puzzleCols = reaches.map((r) => ({ reachIndex: r.meta.reachIndex, entries: r.puzzleInstances.map((p) => p.defId) }));

  scroll.append(track("Gadgets", gadgetCols, gadgetPlan));
  scroll.append(track("Puzzles", puzzleCols, puzzlePlan));
  if (gadgetPlan.size === 0) scroll.append(el("div", { class: "hint", style: "margin-top:8px" }, "Virtual plan shown only for length-bounded Worlds; this preset is unbounded."));
  host.append(scroll);
}

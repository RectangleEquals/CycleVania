/** Right detail panel — context-sensitive: World meta / Reach meta / Region rules. */

import { ruleSummary, missingCaps, CapSet } from "@cyclevania/core";
import { store } from "../state.js";
import { activeReach } from "../views/mission.js";
import { el, clear } from "../dom.js";

function kv(pairs: [string, string][]): HTMLElement {
  const g = el("div", { class: "kv" });
  for (const [k, v] of pairs) {
    g.append(el("div", { class: "k" }, k));
    g.append(el("div", {}, v));
  }
  return g;
}

export function renderDetail(host: HTMLElement): void {
  clear(host);
  const { selection, descriptor } = store.state;
  const reach = activeReach();

  if (selection.kind === "region" && reach) {
    const rg = reach.graph.regions.find((r) => r.id === selection.regionId);
    host.append(el("h3", {}, `Region · ${selection.regionId}`));
    host.append(kv([["role", rg?.role ?? "?"]]));

    const empty = new CapSet();
    const incident = reach.graph.edges.filter((e) => e.from === selection.regionId || e.to === selection.regionId);
    host.append(el("h3", {}, "Edges"));
    if (incident.length === 0) host.append(el("div", { class: "hint" }, "none"));
    for (const e of incident) {
      const dir = e.from === selection.regionId ? "→" : "←";
      const other = e.from === selection.regionId ? e.to : e.from;
      const summary = ruleSummary(e.rule) || "open";
      const miss = missingCaps(e.rule, empty);
      const line = el("div", { style: "margin:3px 0" }, `${dir} ${other}: `, el("span", { style: e.rule.k === "always" ? "" : "color:var(--warn)" }, summary));
      if (miss.length) line.append(el("span", { class: "dim" }, `  (needs ${miss.join(", ")})`));
      host.append(line);
    }

    const locs = reach.graph.locations.filter((l) => l.region === selection.regionId);
    if (locs.length) {
      host.append(el("h3", {}, "Locations"));
      for (const l of locs) {
        const item = reach.placement.get(l.id);
        host.append(el("div", { style: "margin:2px 0" }, `${l.id}: `, el("span", { class: "dim" }, item ?? "(empty)"), l.bonus ? el("span", { class: "dim" }, " · bonus") : ""));
      }
    }
    return;
  }

  if (selection.kind === "reach" && reach) {
    host.append(el("h3", {}, `Reach ${reach.meta.reachIndex}`));
    host.append(
      kv([
        ["ceiling", String(Math.round(reach.meta.finalCeiling))],
        ["areas", String(reach.meta.areaCount)],
        ["regions", String(reach.graph.regions.length)],
        ["spheres", String(reach.meta.spheres.length)],
        ["modifiers", reach.meta.chosenModifiers.join(", ") || "none"],
        ["relaxations", String(reach.meta.relaxations.length)],
      ]),
    );
    if (reach.meta.relaxations.length) {
      host.append(el("h3", {}, "Relaxations"));
      host.append(el("pre", {}, reach.meta.relaxations.join("\n")));
    }
    return;
  }

  host.append(el("h3", {}, "World"));
  if (descriptor) {
    host.append(
      kv([
        ["seed", descriptor.meta.worldSeed],
        ["version", descriptor.meta.generationVersion],
        ["fingerprint", descriptor.meta.registryFingerprint],
        ["reaches", String(descriptor.reaches.length)],
        ["length", descriptor.meta.drawnLength !== undefined ? String(descriptor.meta.drawnLength) : "unbounded"],
      ]),
    );
    host.append(el("h3", {}, "Tips"));
    host.append(el("div", { class: "hint" }, "Click a node to select; double-click a Reach to open its Mission graph."));
  } else {
    host.append(el("div", { class: "hint" }, "Generating…"));
  }
}

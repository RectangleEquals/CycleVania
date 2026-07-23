/**
 * Diagnostics panel — a filterable list of every DiagEvent from the last generation
 * (fed by a MemorySink). Click-to-navigate selects the named Reach/Region when the
 * event's `path` carries one. Warning/error counts badge the top bar (see main.ts).
 */

import type { DiagLevel } from "@cyclevania/core";
import { store } from "../state.js";
import { el, clear } from "../dom.js";

const ORDER: Record<DiagLevel, number> = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };

function navigate(path: string | undefined): void {
  if (!path) return;
  const m = /reach\s*(\d+)/i.exec(path);
  if (m) store.select({ kind: "reach", reachIndex: Number(m[1]) });
}

export function renderDiagnostics(host: HTMLElement): void {
  const { diagnostics, diagLevel, diagCollapsed } = store.state;
  host.className = `diag${diagCollapsed ? " collapsed" : ""}`;
  clear(host);

  const shown = diagnostics.filter((e) => ORDER[e.level] <= ORDER[diagLevel]);
  const warn = diagnostics.filter((e) => e.level === "warn").length;
  const err = diagnostics.filter((e) => e.level === "error").length;

  const hdr = el("div", { class: "hdr" }, `${diagCollapsed ? "▸" : "▾"} Diagnostics — ${shown.length} shown · ${warn} warn · ${err} error`);
  hdr.addEventListener("click", () => store.set({ diagCollapsed: !diagCollapsed }));
  host.append(hdr);
  if (diagCollapsed) return;

  if (shown.length === 0) {
    host.append(el("div", { class: "evt" }, el("span", {}, ""), el("span", {}, ""), el("span", { class: "hint" }, "no events at this level")));
    return;
  }
  for (const e of shown) {
    const row = el("div", { class: "evt" });
    row.append(el("span", { class: `lvl ${e.level}` }, e.level));
    row.append(el("span", { class: "code" }, e.code));
    row.append(el("span", {}, `${e.message}${e.path ? `  (${e.path})` : ""}`));
    row.addEventListener("click", () => navigate(e.path));
    host.append(row);
  }
}

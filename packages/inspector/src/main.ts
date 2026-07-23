/**
 * Inspector boot + shell. Builds the persistent top bar / three-pane layout once,
 * then re-renders the navigator, detail, active view, progress overlay, and
 * diagnostics panel on every store change. Phase 12.1: World / Mission / Schedule
 * views + progress overlay + diagnostics. Later phases add Skeleton/Volume/Geometry,
 * Play, and the editor/dials/seed-lab/bundle panels.
 */

import "./styles.css";
import { stableStringify, type DiagLevel } from "@cyclevania/core";
import { store, PHASE_1_VIEWS, type ViewId } from "./state.js";
import { regenerate } from "./generate.js";
import { renderWorld } from "./views/world.js";
import { renderMission } from "./views/mission.js";
import { renderSchedule } from "./views/schedule.js";
import { renderNavigator } from "./panels/navigator.js";
import { renderDetail } from "./panels/detail.js";
import { renderProgress } from "./panels/progress.js";
import { renderDiagnostics } from "./panels/diagnostics.js";
import { ROLE_COLORS } from "./graph-view.js";
import { el } from "./dom.js";

const PRESETS = ["classic", "crawler", "prime"];
const LEVELS: DiagLevel[] = ["error", "warn", "info", "debug", "trace"];

const app = document.getElementById("app");
if (!app) throw new Error("no #app");

// --- top bar ---
const topbar = el("div", { class: "topbar" });
const brand = el("span", { class: "brand" }, "CycleVania Inspector");

const presetSel = el("select") as HTMLSelectElement;
for (const p of PRESETS) presetSel.append(el("option", { value: p }, p));
presetSel.value = store.state.settings.presetName;

const seedInput = el("input", { type: "text", value: store.state.settings.seed }) as HTMLInputElement;
const reachInput = el("input", { type: "number", min: "1", max: "5", value: String(store.state.settings.reaches) }) as HTMLInputElement;
const geoChk = el("input", { type: "checkbox" }) as HTMLInputElement;
geoChk.checked = store.state.settings.geometry;
const regenBtn = el("button", { class: "primary" }, "Regenerate");
const exportBtn = el("button", {}, "Export JSON");
const levelSel = el("select") as HTMLSelectElement;
for (const l of LEVELS) levelSel.append(el("option", { value: l }, l));
levelSel.value = store.state.diagLevel;

const warnBadge = el("span", { class: "badge warn" });
const errBadge = el("span", { class: "badge err" });

const tabs = el("div", { class: "tabs" });
const tabButtons = new Map<ViewId, HTMLButtonElement>();
for (const v of PHASE_1_VIEWS) {
  const b = el("button", {}, v) as HTMLButtonElement;
  b.addEventListener("click", () => store.set({ view: v }));
  tabButtons.set(v, b);
  tabs.append(b);
}

topbar.append(
  brand,
  el("label", {}, "preset", presetSel),
  el("label", {}, "seed", seedInput),
  el("label", {}, "reaches", reachInput),
  el("label", {}, geoChk, "geometry"),
  regenBtn,
  exportBtn,
  el("label", {}, "log", levelSel),
  warnBadge,
  errBadge,
  tabs,
);

// --- body ---
const body = el("div", { class: "body" });
const nav = el("div", { class: "nav" });
const stage = el("div", { class: "stage" });
const detail = el("div", { class: "detail" });
const viewHost = el("div", { style: "position:absolute; inset:0" });
const legend = el("div", { class: "legend" });
const overlay = el("div", { class: "overlay" });
const diag = el("div", { class: "diag" });
stage.append(viewHost, legend, overlay, diag);
body.append(nav, stage, detail);
app.append(topbar, body);

// static role legend
for (const [role, color] of Object.entries(ROLE_COLORS)) {
  if (role === "default" || role === "portal") continue;
  legend.append(el("div", {}, el("span", { class: "sw", style: `background:${color}` }), role));
}

// --- control events ---
const go = (): void => {
  regenerate().catch((e) => console.error("[inspector] generation failed:", e));
};
presetSel.addEventListener("change", () => {
  store.state.settings.presetName = presetSel.value;
  go();
});
geoChk.addEventListener("change", () => {
  store.state.settings.geometry = geoChk.checked;
  go();
});
reachInput.addEventListener("change", () => {
  store.state.settings.reaches = Math.max(1, Math.min(5, Number(reachInput.value) || 1));
  reachInput.value = String(store.state.settings.reaches);
  go();
});
seedInput.addEventListener("change", () => {
  store.state.settings.seed = seedInput.value.trim() || "cyclevania-demo";
  go();
});
regenBtn.addEventListener("click", () => {
  store.state.settings.seed = seedInput.value.trim() || "cyclevania-demo";
  go();
});
levelSel.addEventListener("change", () => store.set({ diagLevel: levelSel.value as DiagLevel }));
exportBtn.addEventListener("click", () => {
  const d = store.state.descriptor;
  if (!d) return;
  const blob = new Blob([stableStringify(d)], { type: "application/json" });
  const a = el("a", { href: URL.createObjectURL(blob), download: `${d.meta.worldSeed}.world.json` });
  a.click();
  URL.revokeObjectURL(a.href);
});

// --- render loop ---
function renderView(): void {
  if (store.state.view === "mission") renderMission(viewHost);
  else if (store.state.view === "schedule") renderSchedule(viewHost);
  else renderWorld(viewHost);
}

function render(): void {
  for (const [v, b] of tabButtons) b.className = store.state.view === v ? "active" : "";
  legend.style.display = store.state.view === "schedule" ? "none" : "block";

  const warn = store.state.diagnostics.filter((e) => e.level === "warn").length;
  const err = store.state.diagnostics.filter((e) => e.level === "error").length;
  warnBadge.textContent = `${warn} warn`;
  errBadge.textContent = `${err} err`;
  warnBadge.style.display = warn > 0 ? "inline-block" : "none";
  errBadge.style.display = err > 0 ? "inline-block" : "none";

  renderView();
  renderNavigator(nav);
  renderDetail(detail);
  renderProgress(overlay);
  renderDiagnostics(diag);
}

// initial state from URL params (deep-linking + screenshot verification)
const params = new URLSearchParams(location.search);
const pView = params.get("view");
if (pView && (PHASE_1_VIEWS as string[]).includes(pView)) store.state.view = pView as ViewId;
const pPreset = params.get("preset");
if (pPreset && PRESETS.includes(pPreset)) {
  store.state.settings.presetName = pPreset;
  presetSel.value = pPreset;
}
const pSeed = params.get("seed");
if (pSeed) {
  store.state.settings.seed = pSeed;
  seedInput.value = pSeed;
}
const pGeo = params.get("geometry");
if (pGeo !== null) {
  store.state.settings.geometry = pGeo !== "0" && pGeo !== "false";
  geoChk.checked = store.state.settings.geometry;
}

store.subscribe(render);
render();
go();

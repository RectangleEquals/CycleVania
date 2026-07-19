/**
 * Inspector boot + UI. Two modes:
 *  · INSPECT — scope-isolated navigation World→Reach→Area→Room→Cell. Click to
 *    select + descend + zoom; Back ascends; detail-panel links focus the camera.
 *  · PLAY — drop inside the reach's first room; walk room-to-room by double-
 *    clicking exits (gate-checked), pick up gadgets, with diegetic on-screen
 *    messages and a live item/sphere readout.
 */

import {
  CapSet,
  buildSimWorld,
  complexityFor,
  composeWorld,
  defineRegistry,
  evalRule,
  initSim,
  missingCaps,
  parseCommand,
  ruleCaps,
  step,
  type AreaDescriptor,
  type CellDescriptor,
  type ReachResult,
  type Registry,
  type Rule,
  type RoomDescriptor,
  type SimState,
  type SimWorld,
} from "@cyclevania/core";
import { demoGeometryKit, demoStyles, demoTemplate, gadgetCatalog, lockCatalog, lootCatalog } from "@cyclevania/examples";
import { CONTENT_COLORS, InspectorScene, ROLE_COLORS, type PickResult, type Scope } from "./scene.js";
import { RoomGraph, roomKey, tagReachIndex, type RoomExit, type RoomRef } from "./roomgraph.js";

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;
const esc = (s: string): string => s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c] as string);

interface Settings { seed: string; reachCount: number; depth: number; roomCell: number; snap: "ps2" | "free"; }

const scene = new InspectorScene($("app"));

let settings: Settings = { seed: "cyclevania-demo", reachCount: 1, depth: 26, roomCell: 2, snap: "ps2" };
let registry: Registry;
let reaches: ReachResult[] = [];
let scope: Scope = { level: "world" };
let selection: PickResult | null = null;
let activeReach = 0;
let simWorld: SimWorld;
let simState: SimState;
let xrayOn = true;

// play mode
let play = false;
let graph: RoomGraph;
let at: RoomRef | null = null;
let held = new CapSet();
let collected = new Set<string>();
let inventory = new Set<string>();

const linkTargets = new Map<string, () => void>();

const HELP = [
  "commands:  /help /clear /goto <id> /take /use <item> /give <cap> /why <id> /reset /solve",
].join("\n");

// ---------- compose ----------
function buildRegistry(): void {
  registry = defineRegistry({
    grid: { areaCellSize: 16, roomCellSize: settings.roomCell, snap: settings.snap },
    geometryKit: demoGeometryKit,
    items: { catalog: [...gadgetCatalog, ...lootCatalog], startCaps: [] },
    locks: lockCatalog,
    styles: demoStyles,
  });
}

function compose(): void {
  if (play) exitPlay();
  buildRegistry();
  const world = composeWorld({ registry, seed: settings.seed }, { reachCount: settings.reachCount, template: demoTemplate, carryCaps: true, depthFor: (i) => settings.depth + i * 6 });
  reaches = world.reaches;
  reaches.forEach((r, i) => tagReachIndex(r, i));
  graph = new RoomGraph(reaches, registry);
  scene.setData(reaches);
  setActiveReach(0);
  goScope({ level: "world" }, null);
  log(`⟳ world "${settings.seed}" · ${reaches.length} reach(es) · ${reaches.reduce((n, r) => n + r.descriptor.areas.length, 0)} areas`);
  maybeDeepLink();
}

// dev-only: #room / #play deep-links so a scope can be screenshotted headlessly
function maybeDeepLink(): void {
  const r0 = reaches[0];
  if (!r0) return;
  if (location.hash === "#play") {
    setActiveReach(0);
    enterPlay();
  } else if (location.hash.startsWith("#area")) {
    const a = r0.descriptor.areas.find((x) => x.rooms.length >= 2) ?? r0.descriptor.areas[0];
    if (a) goScope({ level: "area", ri: 0, areaId: a.areaId }, { kind: "area", ri: 0, areaId: a.areaId, box: a.bounds });
  } else if (location.hash.startsWith("#room")) {
    const a = r0.descriptor.areas.find((x) => x.rooms.length > 0) ?? r0.descriptor.areas[0];
    const rm = a?.rooms[0];
    if (a && rm) goScope({ level: "room", ri: 0, areaId: a.areaId, nodeId: rm.nodeId }, { kind: "room", ri: 0, areaId: a.areaId, nodeId: rm.nodeId, box: rm.bounds });
  }
}

function setActiveReach(i: number): void {
  activeReach = i;
  const r = reaches[i];
  if (!r) return;
  simWorld = buildSimWorld(r, registry);
  simState = initSim(simWorld);
}

// ---------- inspect: scope + selection ----------
function goScope(s: Scope, sel: PickResult | null): void {
  scope = s;
  scene.setScope(s);
  selection = sel;
  scene.setSelected(sel);
  if ((s.level === "world" || s.level === "reach") && simWorld) scene.highlightSim(activeReach, simWorld, simState);
  $("back").style.display = s.level === "world" || play ? "none" : "block";
  render();
}

function selectOnly(pick: PickResult): void {
  selection = pick;
  scene.setSelected(pick);
  scene.focus(pick);
  render();
}

function inspect(pick: PickResult): void {
  if (pick.ri !== activeReach && scope.level === "world") setActiveReach(pick.ri);
  if (pick.kind === "area") goScope({ level: "area", ri: pick.ri, areaId: pick.areaId! }, pick);
  else if (pick.kind === "room") goScope({ level: "room", ri: pick.ri, areaId: pick.areaId!, nodeId: pick.nodeId! }, pick);
  else selectOnly(pick);
}

function back(): void {
  if (scope.level === "reach") goScope({ level: "world" }, null);
  else if (scope.level === "area") goScope({ level: "reach", ri: scope.ri }, null);
  else if (scope.level === "room") goScope({ level: "area", ri: scope.ri, areaId: scope.areaId }, null);
}

// ---------- play mode ----------
function enterPlay(): void {
  const start = reaches[activeReach] ? graph.startRoom(reaches[activeReach]!) : null;
  if (!start) return;
  play = true;
  at = start;
  held = new CapSet();
  for (const c of registry.items.startCaps) held.add(c);
  collected = new Set();
  inventory = new Set();
  $("playToggle").textContent = "■ Exit Play";
  $("play-info").style.display = "block";
  $("xray-panel").style.display = "none";
  $("right").classList.add("collapsed");
  $("right-reopen").style.display = "block";
  scene.setXray({ on: false });
  enterRoom(start, `You stand at the threshold of area ${start.areaId}.`);
}

function exitPlay(): void {
  play = false;
  at = null;
  $("playToggle").textContent = "▶ Play";
  $("play-info").style.display = "none";
  $("xray-panel").style.display = "block";
  scene.setPlayCamera(false);
  scene.setXray({ on: xrayOn });
  goScope({ level: "world" }, null);
}

function enterRoom(ref: RoomRef, message: string): void {
  at = ref;
  const node = graph.nodes.get(roomKey(ref));
  scope = { level: "room", ri: ref.ri, areaId: ref.areaId, nodeId: ref.nodeId };
  scene.setScope(scope);
  if (node) scene.setPlayCamera(true, node.bounds);
  selection = null;
  scene.setSelected(null);
  $("back").style.display = "none";
  toast(message);
  renderPlayInfo();
}

function itemName(id: string): string {
  return registry.items.defs.get(id)?.name ?? id;
}
function capOf(id: string): string | undefined {
  return registry.items.defs.get(id)?.grants;
}

function interact(pick: PickResult): void {
  if (!at) return;
  const node = graph.nodes.get(roomKey(at));
  if (!node) return;
  if (pick.kind === "gadget" && pick.itemId) {
    const g = node.gadgets.find((x) => x.itemId === pick.itemId || x.locationId === pick.itemId);
    if (!g || collected.has(g.locationId)) return toast("Nothing to recover here.");
    collected.add(g.locationId);
    inventory.add(g.itemId);
    if (g.cap) held.add(g.cap);
    toast(`Recovered ${itemName(g.itemId)}${g.cap ? ` — now wielding ${g.cap} access` : ""}.`);
    renderPlayInfo();
    return;
  }
  if (pick.kind === "connection" && pick.socketId) {
    const exit = node.exits.find((e) => e.socketId === pick.socketId) ?? node.exits.find((e) => e.pos[0] === pick.pos?.[0]);
    if (!exit) return toast("There is no way through here.");
    if (exit.gate && !evalRule(exit.gate, held)) {
      const miss = [...missingCaps(exit.gate, held)];
      return toast(`The way is sealed — you need ${miss.join(", ") || "something"}.`);
    }
    enterRoom(exit.to, travelMessage(exit));
    return;
  }
  selectOnly(pick);
}

function travelMessage(exit: RoomExit): string {
  if (exit.kind === "portal") {
    if (exit.gate) {
      const cap = [...ruleCaps(exit.gate)][0];
      const holder = [...registry.items.defs.values()].find((d) => d.grants === cap);
      return `Used ${holder?.name ?? cap} → opened the ${esc(exit.label)}.`;
    }
    return `You pass through the ${esc(exit.label)}.`;
  }
  return `You slip through the ${esc(exit.label)}.`;
}

// ---------- rendering DOM ----------
function render(): void {
  renderCrumbs();
  renderDetails();
  if (play) renderPlayInfo();
}

function renderCrumbs(): void {
  linkTargets.clear();
  const s = scope;
  const parts: string[] = [linkEl("World", () => goScope({ level: "world" }, null))];
  if (s.level !== "world") parts.push(linkEl(`Reach ${s.ri}`, () => goScope({ level: "reach", ri: s.ri }, null)));
  if (s.level === "area" || s.level === "room") parts.push(linkEl(`Area ${s.areaId}`, () => goScope({ level: "area", ri: s.ri, areaId: s.areaId }, area(s.ri, s.areaId) ? { kind: "area", ri: s.ri, areaId: s.areaId, box: area(s.ri, s.areaId)!.bounds } : null)));
  if (s.level === "room") parts.push(`<span>Room ${esc(s.nodeId)}</span>`);
  $("crumbs").innerHTML = parts.join(`<span class="sep">›</span>`);
  bindLinks("crumbs");
}

let linkSeq = 0;
function linkEl(text: string, act: () => void): string {
  const id = `lt${linkSeq++}`;
  linkTargets.set(id, act);
  return `<a class="link" data-lt="${id}">${esc(text)}</a>`;
}
function pillLink(text: string, cls: string, act: () => void): string {
  const id = `lt${linkSeq++}`;
  linkTargets.set(id, act);
  return `<span class="pill link ${cls}" data-lt="${id}">${esc(text)}</span>`;
}
function bindLinks(containerId: string): void {
  for (const el of document.querySelectorAll<HTMLElement>(`#${containerId} [data-lt]`)) {
    el.addEventListener("click", () => linkTargets.get(el.dataset["lt"] as string)?.());
  }
}

const area = (ri: number, areaId: number): AreaDescriptor | undefined => reaches[ri]?.descriptor.areas.find((a) => a.areaId === areaId);
const room = (ri: number, areaId: number, nodeId: string): RoomDescriptor | undefined => area(ri, areaId)?.rooms.find((r) => r.nodeId === nodeId);
const tableRows = (rows: Array<[string, string]>): string => `<table>${rows.map(([k, v]) => `<tr><td>${esc(k)}</td><td>${v}</td></tr>`).join("")}</table>`;

function renderDetails(): void {
  const el = $("details");
  linkSeq = 1000;
  const sel = selection;
  if (sel?.kind === "cell" && sel.cell) el.innerHTML = cellDetail(sel.cell);
  else if (scope.level === "room") el.innerHTML = roomDetail(scope.ri, scope.areaId, scope.nodeId);
  else if (scope.level === "area") el.innerHTML = areaDetail(scope.ri, scope.areaId);
  else if (scope.level === "reach") el.innerHTML = reachDetail(scope.ri);
  else el.innerHTML = worldDetail();
  bindLinks("details");
}

function worldDetail(): string {
  const areas = reaches.reduce((n, r) => n + r.descriptor.areas.length, 0);
  const gadgets = reaches.reduce((n, r) => n + r.descriptor.areas.reduce((m, a) => m + a.gadgets.length, 0), 0);
  const list = reaches.map((_, i) => pillLink(`Reach ${i}`, "", () => { setActiveReach(i); goScope({ level: "reach", ri: i }, null); })).join("");
  return `<h2>World</h2>${tableRows([["seed", esc(settings.seed)], ["reaches", String(reaches.length)], ["areas", String(areas)], ["gadgets", String(gadgets)]])}<h2>reaches</h2>${list}<div class="hint">Click an area/room to inspect. Toggle ▶ Play to walk it.</div>`;
}

function reachDetail(ri: number): string {
  const r = reaches[ri];
  if (!r) return "";
  const gated = r.descriptor.links.filter((l) => (l.requiredCaps?.length ?? 0) > 0).length;
  const conns = r.descriptor.areas.reduce((n, a) => n + a.connectors.length, 0);
  const b = complexityFor(settings.depth + ri * 6, registry.complexity);
  const areas = r.descriptor.areas.map((a) => pillLink(`${a.areaId}:${a.role} (${a.rooms.length}r)`, "", () => goScope({ level: "area", ri, areaId: a.areaId }, { kind: "area", ri, areaId: a.areaId, box: a.bounds }))).join("");
  return (
    `<h2>Reach ${ri}</h2>${tableRows([["areas", String(r.descriptor.areas.length)], ["links", `${r.descriptor.links.length} (${gated} gated)`], ["connectors", String(conns)], ["items", String(r.reach.items.length)]])}` +
    `<h2>budget (depth ${settings.depth + ri * 6})</h2>${tableRows([["c", b.c.toFixed(2)], ["room count", String(b.roomCount)], ["room size max", b.roomSizeMax.toFixed(0)], ["loop chance", b.loopChance.toFixed(2)], ["extra cycles", b.extraCycles.toFixed(1)], ["z-spread", b.zSpread.toFixed(2)]])}` +
    `<h2>areas</h2>${areas}`
  );
}

function areaDetail(ri: number, areaId: number): string {
  const a = area(ri, areaId);
  if (!a) return "";
  const rooms = a.rooms.map((r) => pillLink(`${r.nodeId} · ${r.kind}`, "", () => goScope({ level: "room", ri, areaId, nodeId: r.nodeId }, { kind: "room", ri, areaId, nodeId: r.nodeId, box: r.bounds }))).join("");
  const portals = a.portals.map((p) => pillLink(`${p.key}${p.requires ? " 🔒" : ""}`, p.requires ? "need" : "have", () => selectOnly({ kind: "connection", ri, areaId, socketId: `portal:${p.key}`, gated: !!p.requires, pos: p.spawn }))).join("") || "<span class='hint'>none</span>";
  const gadgets = a.gadgets.map((g) => pillLink(esc(itemName(g.itemId)), "", () => selectOnly({ kind: "gadget", ri, areaId, itemId: g.itemId, pos: g.pos }))).join("") || "<span class='hint'>none</span>";
  return `<h2>Area ${areaId}</h2>${tableRows([["role", esc(a.role)], ["region", esc(a.regionId)], ["rooms", String(a.rooms.length)], ["connectors", String(a.connectors.length)]])}<h2>rooms</h2>${rooms}<h2>portals</h2>${portals}<h2>gadgets</h2>${gadgets}`;
}

function roomDetail(ri: number, areaId: number, nodeId: string): string {
  const r = room(ri, areaId, nodeId);
  if (!r) return "";
  const roleCount = new Map<string, number>();
  for (const c of r.cells) roleCount.set(c.role, (roleCount.get(c.role) ?? 0) + 1);
  const roles = [...roleCount].map(([role, n]) => `<span class="pill">${esc(role)} ×${n}</span>`).join("");
  const sockets = r.sockets.map((s) => pillLink(`${s.id} · ${s.face}${s.gate ? " 🔒" : ""}`, s.gate ? "need" : "have", () => selectOnly({ kind: "connection", ri, areaId, nodeId, socketId: s.id, gated: !!s.gate, pos: s.pos }))).join("") || "<span class='hint'>none</span>";
  return `<h2>Room ${esc(nodeId)}</h2>${tableRows([["kind", esc(r.kind)], ["role", esc(r.role)], ["footprint", r.footprint.join(" × ")], ["cells", String(r.cells.length)]])}<h2>cell roles</h2>${roles}<h2>exits</h2>${sockets}<div class="hint">Click a cell in 3D for its metadata.</div>`;
}

function cellDetail(c: CellDescriptor): string {
  const contents = c.contents.map((x) => `<span class="pill">${esc(x.kind)}${x.ref ? ":" + esc(x.ref) : ""}</span>`).join("") || "<span class='hint'>empty</span>";
  return `<h2>Cell ${c.coord.join(",")}</h2>${tableRows([["role", esc(c.role)], ["kit", c.kitId ? esc(c.kitId) : "<i>air</i>"], ["yaw", `${Math.round((c.yaw * 180) / Math.PI)}°`], ["traversal", c.traversal ? esc(c.traversal) : "-"]])}<h2>contents</h2>${contents}`;
}

function renderPlayInfo(): void {
  if (!at) return;
  linkSeq = 5000;
  const node = graph.nodes.get(roomKey(at));
  const a = area(at.ri, at.areaId);
  const roomItems = (node?.gadgets ?? []).map((g) => `<span class="pill ${collected.has(g.locationId) ? "have" : ""}">${esc(itemName(g.itemId))}${collected.has(g.locationId) ? " ✓" : ""}</span>`).join("") || "<span class='hint'>none here</span>";
  const inv = [...inventory].map((id) => `<span class="pill have">${esc(itemName(id))}</span>`).join("") || "<span class='hint'>empty</span>";
  const areaItems = (a?.gadgets ?? []).map((g) => `<span class="pill ${collected.has(g.locationId) ? "have" : ""}">${esc(itemName(g.itemId))}</span>`).join("") || "<span class='hint'>none</span>";
  const hints = (node?.exits ?? []).map((e) => {
    const open = !e.gate || evalRule(e.gate, held);
    const miss = e.gate && !open ? [...missingCaps(e.gate, held)].join(",") : "";
    return `<span class="pill ${open ? "have" : "need"}">${esc(e.label)}${open ? "" : " — need " + miss}</span>`;
  }).join("") || "<span class='hint'>no exits</span>";
  $("pi-room").innerHTML = roomItems;
  $("pi-collected").innerHTML = inv;
  $("pi-area").innerHTML = areaItems;
  $("pi-hints").innerHTML = hints;
}

// ---------- console + log + toasts ----------
function runCommand(input: string): void {
  const s = input.trim();
  if (!s) return;
  const bare = s.replace(/^\//, "").toLowerCase();
  if (bare === "help" || bare === "?") return log(HELP);
  if (bare === "clear") return clearLog();
  try {
    const res = step(simWorld, simState, parseCommand(s));
    simState = res.state;
    if (scope.level === "world" || scope.level === "reach") scene.highlightSim(activeReach, simWorld, simState);
    log(`${res.ok ? "›" : "✗"} ${s} — ${res.message}`);
  } catch (err) {
    log(`✗ ${(err as Error).message}`);
  }
}
function log(line: string): void {
  const el = $("log");
  el.textContent = `${line}\n${el.textContent ?? ""}`.split("\n").slice(0, 200).join("\n");
}
function clearLog(): void {
  $("log").textContent = "";
}
function toast(text: string): void {
  const box = $("messages");
  const el = document.createElement("div");
  el.className = "toast";
  el.textContent = text;
  box.appendChild(el);
  while (box.children.length > 4) box.removeChild(box.firstChild as Node);
  setTimeout(() => el.remove(), 4200);
}

// ---------- settings ----------
function readSettings(): void {
  settings = {
    seed: $<HTMLInputElement>("seed").value || "cyclevania-demo",
    reachCount: Math.max(1, Number($<HTMLInputElement>("reachCount").value) || 1),
    depth: Math.max(0, Number($<HTMLInputElement>("depth").value) || 0),
    roomCell: Math.max(1, Number($<HTMLInputElement>("roomCell").value) || 2),
    snap: ($<HTMLSelectElement>("snap").value as "ps2" | "free") || "ps2",
  };
}
function writeSettings(): void {
  $<HTMLInputElement>("seed").value = settings.seed;
  $<HTMLInputElement>("reachCount").value = String(settings.reachCount);
  $<HTMLInputElement>("depth").value = String(settings.depth);
  $<HTMLInputElement>("roomCell").value = String(settings.roomCell);
  $<HTMLSelectElement>("snap").value = settings.snap;
}

// ---------- legend ----------
function buildLegend(): void {
  const items: Array<[number, string]> = [
    ...Object.entries(ROLE_COLORS).map(([r, c]) => [c, r] as [number, string]),
    [CONTENT_COLORS.gadget as number, "gadget"],
    [CONTENT_COLORS.prop as number, "prop"],
    [CONTENT_COLORS.hazard as number, "hazard"],
    [CONTENT_COLORS.enemy as number, "enemy"],
    [CONTENT_COLORS.switch as number, "switch"],
    [0x5fd35f, "open exit/link"],
    [0xd85a5a, "gated exit/link"],
    [0xffe08a, "selected"],
    [0xffffff, "you (sim)"],
  ];
  const html = items.map(([c, l]) => `<div class="legend"><span class="swatch" style="background:#${c.toString(16).padStart(6, "0")}"></span>${esc(l)}</div>`).join("");
  $("legend").innerHTML = html;
  $("floatLegend").innerHTML = html;
}

// ---------- x-ray controls ----------
function buildXrayMask(): void {
  const roles = ["ceiling", "wall", "corner", "opening"];
  const def: Record<string, boolean> = { ceiling: true, wall: true, corner: true, opening: false };
  $("xmask").innerHTML = roles.map((r) => `<label style="min-width:auto"><input type="checkbox" data-xmask="${r}" ${def[r] ? "checked" : ""}/> ${r}</label>`).join(" ");
  for (const cb of document.querySelectorAll<HTMLInputElement>("[data-xmask]")) cb.addEventListener("change", () => scene.setXray({ mask: { [cb.dataset["xmask"] as string]: cb.checked } }));
}
function wireRange(rangeId: string, minId: string, maxId: string, valId: string, onChange: (v: number) => void): void {
  const range = $<HTMLInputElement>(rangeId);
  const min = $<HTMLInputElement>(minId);
  const max = $<HTMLInputElement>(maxId);
  const val = $(valId);
  const sync = (): void => {
    range.min = min.value;
    range.max = max.value;
    val.textContent = range.value;
    onChange(Number(range.value));
  };
  range.addEventListener("input", sync);
  min.addEventListener("change", sync);
  max.addEventListener("change", sync);
}

// ---------- wiring ----------
$("regen").addEventListener("click", () => { readSettings(); compose(); });
$("addReach").addEventListener("click", () => { readSettings(); settings.reachCount += 1; writeSettings(); compose(); });
$("save").addEventListener("click", () => {
  readSettings();
  const url = URL.createObjectURL(new Blob([JSON.stringify(settings, null, 2)], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = `cyclevania-${settings.seed}.json`;
  a.click();
  URL.revokeObjectURL(url);
});
$("load").addEventListener("click", () => $("loadFile").click());
$<HTMLInputElement>("loadFile").addEventListener("change", (e) => {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => { try { settings = { ...settings, ...(JSON.parse(String(reader.result)) as Partial<Settings>) }; writeSettings(); compose(); } catch (err) { log(`✗ load: ${(err as Error).message}`); } };
  reader.readAsText(file);
});
for (const b of document.querySelectorAll<HTMLButtonElement>("button[data-cmd]")) b.addEventListener("click", () => runCommand(b.dataset["cmd"] as string));
$("clearLog").addEventListener("click", clearLog);
$<HTMLInputElement>("repl").addEventListener("keydown", (e) => { if (e.key === "Enter") { runCommand($<HTMLInputElement>("repl").value); $<HTMLInputElement>("repl").value = ""; } });
$("back").addEventListener("click", back);
$("playToggle").addEventListener("click", () => (play ? exitPlay() : enterPlay()));

const collapseRight = (v: boolean): void => {
  $("right").classList.toggle("collapsed", v);
  $("right-reopen").style.display = v ? "block" : "none";
  $("floatLegend").style.display = v ? "block" : "none";
  $("right-toggle").textContent = v ? "‹" : "›";
};
$("right-toggle").addEventListener("click", () => collapseRight(!$("right").classList.contains("collapsed")));
$("right-reopen").addEventListener("click", () => collapseRight(false));

$<HTMLInputElement>("xrayOn").addEventListener("change", (e) => { xrayOn = (e.target as HTMLInputElement).checked; scene.setXray({ on: xrayOn && !play }); });
wireRange("xrayDist", "xrayDistMin", "xrayDistMax", "xrayDistVal", (v) => scene.setXray({ dist: v }));
wireRange("xrayCone", "xrayConeMin", "xrayConeMax", "xrayConeVal", (v) => scene.setXray({ coneDeg: v }));

// canvas tap / double-tap
const canvas = scene.renderer.domElement;
let lastTap = 0;
let clickTimer: number | undefined;
let downX = 0, downY = 0, downT = 0;
canvas.addEventListener("pointerdown", (e) => { downX = e.clientX; downY = e.clientY; downT = performance.now(); });
canvas.addEventListener("pointerup", (e) => {
  if (Math.hypot(e.clientX - downX, e.clientY - downY) > 6 || performance.now() - downT > 400) return;
  const hit = scene.pick(e.clientX, e.clientY);
  const now = performance.now();
  if (now - lastTap < 280) {
    if (clickTimer) { clearTimeout(clickTimer); clickTimer = undefined; }
    lastTap = 0;
    if (hit) (play ? interact(hit) : inspect(hit));
  } else {
    lastTap = now;
    clickTimer = window.setTimeout(() => {
      clickTimer = undefined;
      if (!hit) return;
      if (play) selectOnly(hit);
      else inspect(hit);
    }, 260);
  }
});

buildLegend();
buildXrayMask();
writeSettings();
compose();

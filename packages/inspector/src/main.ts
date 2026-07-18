/**
 * Inspector boot + UI wiring. Composes a demo Reach, renders it, and drives the
 * playtest simulator through the REPL / buttons / area-click.
 */

import {
  buildSimWorld,
  composeReach,
  initSim,
  parseCommand,
  step,
  type ReachResult,
  type SimState,
  type SimWorld,
} from "@cyclevania/core";
import { demoRegistry, demoTemplate } from "@cyclevania/examples";
import { InspectorScene, ROLE_COLORS } from "./scene.js";

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

const appEl = $("app");
const seedEl = $<HTMLInputElement>("seed");
const reachEl = $<HTMLInputElement>("reachIndex");
const statusEl = $("status");
const logEl = $("log");
const replEl = $<HTMLInputElement>("repl-input");

const scene = new InspectorScene(appEl);

const registry = demoRegistry();
let result: ReachResult;
let world: SimWorld;
let sim: SimState;

function log(line: string): void {
  logEl.textContent = `${line}\n${logEl.textContent ?? ""}`.split("\n").slice(0, 60).join("\n");
}

function refreshStatus(): void {
  const caps = [...world.items.values()].filter((i) => i.cap && sim.held.has(i.cap)).length;
  statusEl.textContent =
    `${result.descriptor.areas.length} areas · ${result.descriptor.links.length} links · ` +
    `at area ${sim.areaId} · ${sim.collected.size} collected · ${caps} caps`;
}

function regenerate(): void {
  const seed = seedEl.value || "cyclevania-demo";
  const reachIndex = Math.max(0, Number(reachEl.value) || 0);
  result = composeReach({ registry, seed }, { template: demoTemplate, reachIndex });
  world = buildSimWorld(result, registry);
  sim = initSim(world);
  scene.setReach(result);
  scene.updateSim(world, sim);
  refreshStatus();
  log(`⟳ generated "${seed}" reach ${reachIndex}: ${result.descriptor.areas.length} areas`);
}

function run(input: string): void {
  if (!input.trim()) return;
  try {
    const cmd = parseCommand(input);
    const res = step(world, sim, cmd);
    sim = res.state;
    scene.updateSim(world, sim);
    refreshStatus();
    log(`${res.ok ? "›" : "✗"} ${input.trim()} — ${res.message}`);
  } catch (err) {
    log(`✗ ${(err as Error).message}`);
  }
}

// legend
const legend = $("legend");
const swatch = (color: number, label: string): void => {
  const row = document.createElement("div");
  row.className = "legend";
  row.innerHTML = `<span class="swatch" style="background:#${color.toString(16).padStart(6, "0")}"></span>${label}`;
  legend.appendChild(row);
};
for (const [role, color] of Object.entries(ROLE_COLORS)) swatch(color, role);
swatch(0xffd24a, "gadget");
swatch(0x5fd35f, "open link/portal");
swatch(0xd85a5a, "gated link/portal");
swatch(0xffffff, "you (current area)");
swatch(0xcaa24a, "blocked area");

// wiring
$("regen").addEventListener("click", regenerate);
for (const b of document.querySelectorAll<HTMLButtonElement>("button[data-cmd]")) {
  b.addEventListener("click", () => run(b.dataset["cmd"] as string));
}
replEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    run(replEl.value);
    replEl.value = "";
  }
});
scene.onAreaClick((areaId) => run(`goto ${areaId}`));

regenerate();

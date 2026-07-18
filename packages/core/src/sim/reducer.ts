/**
 * Simulator reducer — `step(world, state, cmd)` applies one command and returns a
 * fresh state + an outcome message. Pure: the input state is never mutated. This
 * is the same reachability model the solver uses, made interactive.
 */

import { missingCaps } from "../logic/index.js";
import type { Command } from "./command.js";
import { cloneState, initSim, type SimState } from "./state.js";
import { neighbors, type SimWorld } from "./world.js";
import { autosolve } from "./autosolve.js";

export interface SimResult {
  state: SimState;
  ok: boolean;
  message: string;
}

function done(s: SimState, ok: boolean, message: string): SimResult {
  s.log.push((ok ? "· " : "✗ ") + message);
  return { state: s, ok, message };
}

export function step(world: SimWorld, state: SimState, cmd: Command): SimResult {
  if (cmd.k === "reset") return { state: initSim(world), ok: true, message: "reset" };
  if (cmd.k === "solve") {
    const solved = autosolve(world);
    return { state: solved, ok: true, message: `autosolved → area ${solved.areaId}` };
  }

  const s = cloneState(state);
  switch (cmd.k) {
    case "goto": {
      const opts = neighbors(world, s.areaId, s.held);
      const n = opts.find((x) => x.to === cmd.areaId);
      if (!n) return done(s, false, `area ${cmd.areaId} is not adjacent to area ${s.areaId}`);
      if (!n.ok) {
        const miss = [...missingCaps(n.link.requires, s.held)];
        return done(s, false, `blocked into area ${cmd.areaId} — need: ${miss.join(", ") || "?"}`);
      }
      s.areaId = cmd.areaId;
      s.visited.add(cmd.areaId);
      return done(s, true, `moved to area ${cmd.areaId}`);
    }

    case "take": {
      const area = world.areas.get(s.areaId);
      const took: string[] = [];
      for (const g of area?.gadgets ?? []) {
        if (!s.collected.has(g.locationId)) {
          s.collected.add(g.locationId);
          s.inventory.add(g.itemId);
          if (g.cap) s.held.add(g.cap);
          took.push(g.itemId);
        }
      }
      if (took.length === 0) return done(s, false, "nothing to take here");
      return done(s, true, `took ${took.join(", ")}`);
    }

    case "use": {
      if (!s.inventory.has(cmd.itemId)) return done(s, false, `you don't hold "${cmd.itemId}"`);
      const info = world.items.get(cmd.itemId);
      const u = info?.use;
      if (u?.grants) s.held.add(u.grants);
      if (u?.setsFlag) s.held.setFlag(u.setsFlag);
      if (u?.reveals) s.held.setFlag(`reveal:${u.reveals}`);
      return done(s, true, `used ${info?.name ?? cmd.itemId}`);
    }

    case "give": {
      s.held.add(cmd.cap);
      return done(s, true, `granted "${cmd.cap}"`);
    }

    case "why": {
      const opts = neighbors(world, s.areaId, s.held);
      const n = opts.find((x) => x.to === cmd.areaId);
      if (!n) return done(s, true, `area ${cmd.areaId} is not adjacent to area ${s.areaId}`);
      if (n.ok) return done(s, true, `area ${cmd.areaId} is open`);
      const miss = [...missingCaps(n.link.requires, s.held)];
      return done(s, true, `area ${cmd.areaId} needs: ${miss.join(", ") || "?"}`);
    }
  }
}
